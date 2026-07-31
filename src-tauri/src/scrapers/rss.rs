use crate::models::RawNewsItem;
use crate::scrapers::{browser_client, is_high_impact_headline, polite_delay};
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Ingestão por RSS — a via ROBUSTA de coleta.
///
/// Feeds RSS são projetados para leitura por máquina: não retornam 403,
/// não exigem chave e não dependem de JavaScript (ao contrário de raspar
/// o HTML do Investing.com/FinancialJuice, que o Cloudflare bloqueia).
///
/// Fontes padrão (todas gratuitas, macro, de alta credibilidade):
///   - Federal Reserve       -> decisões do FOMC, juros, discursos (oficial)
///   - MarketWatch           -> manchetes de mercado em tempo real (inclui CPI/NFP)
///   - CNBC (Economia)       -> cobertura macro dos EUA
///   - Bloomberg             -> mercados e política (feed público direto)
///   - The Wall Street Journal -> mercados e mundo (feed público direto)
///   - Financial Times       -> homepage internacional (feed público direto)
///   - Reuters               -> a Reuters descontinuou o RSS público em 2020;
///     cobrimos via busca do Google News restrita a site:reuters.com, que
///     devolve as manchetes reais da Reuters com atribuição preservada.
///
/// (O feed do BLS foi removido dos padrões: bls.gov retorna 403 a este
/// cliente. CPI e Payroll ainda chegam pela cobertura de MarketWatch/CNBC/WSJ.)
///
/// Você pode acrescentar/trocar feeds via variável RSS_FEEDS no .env
/// (formato: "Nome|url,Nome|url").
fn default_feeds() -> Vec<(String, String)> {
    // Todos gratuitos, legíveis por máquina e de veículos confiáveis. Cada
    // manchete ainda passa pelo filtro de impacto (keywords macro + anti-ruído)
    // antes de ir ao Gemini, então feeds mais "largos" não poluem o painel.
    let cnbc = |id: &str| {
        format!("https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id={id}")
    };
    let google_news = |query: &str| {
        format!("https://news.google.com/rss/search?q={query}&hl=en-US&gl=US&ceid=US:en")
    };
    vec![
        (
            "Federal Reserve".into(),
            "https://www.federalreserve.gov/feeds/press_all.xml".into(),
        ),
        (
            "MarketWatch".into(),
            "https://feeds.marketwatch.com/marketwatch/realtimeheadlines/".into(),
        ),
        (
            "MarketWatch Top".into(),
            "https://feeds.marketwatch.com/marketwatch/topstories/".into(),
        ),
        ("CNBC Economy".into(), cnbc("20910258")),
        ("CNBC Finance".into(), cnbc("10000664")),
        ("CNBC Markets".into(), cnbc("15839069")),
        ("CNBC Top News".into(), cnbc("100003114")),
        (
            "Yahoo Finance".into(),
            "https://finance.yahoo.com/news/rssindex".into(),
        ),
        (
            "Bloomberg Markets".into(),
            "https://feeds.bloomberg.com/markets/news.rss".into(),
        ),
        (
            "Bloomberg Politics".into(),
            "https://feeds.bloomberg.com/politics/news.rss".into(),
        ),
        (
            "WSJ Markets".into(),
            "https://feeds.a.dj.com/rss/RSSMarketsMain.xml".into(),
        ),
        (
            "WSJ World News".into(),
            "https://feeds.a.dj.com/rss/RSSWorldNews.xml".into(),
        ),
        (
            "Financial Times".into(),
            "https://www.ft.com/rss/home".into(),
        ),
        (
            "Reuters".into(),
            google_news("site:reuters.com+(economy+OR+markets+OR+fed+OR+trump+OR+tariff)+when:2d"),
        ),
    ]
}

/// Nome amigável derivado da URL (host), ex.: "finance.yahoo.com".
fn source_name_from_url(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let host = after.split('/').next().unwrap_or(after);
    host.trim_start_matches("www.").to_string()
}

/// Junta os feeds padrão + os do .env (RSS_FEEDS) + os adicionados pelo
/// usuário na UI (`extra`).
fn feeds(extra: &[String]) -> Vec<(String, String)> {
    let mut list = default_feeds();
    if let Ok(raw) = std::env::var("RSS_FEEDS") {
        for entry in raw.split(',') {
            if let Some((name, url)) = entry.split_once('|') {
                let (name, url) = (name.trim(), url.trim());
                if !url.is_empty() && !list.iter().any(|(_, u)| u == url) {
                    list.push((name.to_string(), url.to_string()));
                }
            }
        }
    }
    for url in extra {
        let url = url.trim();
        if !url.is_empty() && !list.iter().any(|(_, u)| u == url) {
            list.push((source_name_from_url(url), url.to_string()));
        }
    }
    list
}

/// Busca todos os feeds em sequência (cada um com polite_delay anti-bot)
/// e devolve apenas manchetes que passam no filtro de alto/médio impacto.
/// `extra` são as URLs de feed adicionadas pelo usuário.
pub async fn fetch_all(extra: &[String]) -> Result<Vec<RawNewsItem>> {
    let mut items = Vec::new();
    for (source, url) in feeds(extra) {
        match fetch_one(&source, &url).await {
            Ok(mut v) => items.append(&mut v),
            Err(e) => log::warn!("RSS '{source}' indisponível neste ciclo: {e:#}"),
        }
    }
    log::info!("RSS: {} manchete(s) macro no total", items.len());
    Ok(items)
}

/// Caminhos de feed comuns entre os principais CMS/sites de notícia — testados
/// como fallback quando a página não expõe `<link rel="alternate">` (muitos
/// sites JS-pesados omitem essa tag mesmo tendo um feed no ar).
const COMMON_FEED_PATHS: &[&str] = &[
    "/feed", "/feed/", "/rss", "/rss/", "/rss.xml", "/feed.xml", "/atom.xml",
    "/index.xml", "/feeds/all.xml", "/rss/all.xml", "/feed/rss",
];

/// Valida uma URL adicionada pelo usuário: tenta parsear como RSS/Atom e, se
/// for uma página HTML, AUTODESCOBRE o link do feed (`<link rel="alternate"
/// type="application/rss+xml" href="...">`); se ainda assim não achar, tenta
/// caminhos de feed comuns (`/feed`, `/rss.xml`, ...) no mesmo host. Retorna
/// (url_final_do_feed, nº de itens crus encontrados). Erro se não achar feed.
pub(crate) async fn probe_feed(url: &str) -> Result<(String, usize)> {
    let client = browser_client()?;
    let body = client.get(url).send().await?.error_for_status()?.text().await?;

    let titles = extract_titles(&body);
    if !titles.is_empty() {
        return Ok((url.to_string(), titles.len()));
    }

    if let Some(feed_url) = discover_feed_link(&body, url) {
        if let Some(n) = try_feed_url(&client, &feed_url).await {
            return Ok((feed_url, n));
        }
    }

    // Fallback: sites JS-pesados (SPAs) muitas vezes não anunciam o feed no
    // HTML mesmo tendo um. Tenta os caminhos mais comuns no mesmo host.
    let base = base_origin(url);
    for path in COMMON_FEED_PATHS {
        let candidate = format!("{base}{path}");
        if let Some(n) = try_feed_url(&client, &candidate).await {
            return Ok((candidate, n));
        }
    }

    Err(anyhow::anyhow!(
        "nenhum feed RSS/Atom encontrado nessa URL, nem nos caminhos comuns \
         (/feed, /rss.xml, ...). Esse site provavelmente não expõe RSS público \
         — tente colar a URL direta do feed, se você a tiver."
    ))
}

/// Busca uma URL candidata e retorna Some(nº de itens) se for um feed válido.
async fn try_feed_url(client: &reqwest::Client, url: &str) -> Option<usize> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    let titles = extract_titles(&body);
    (!titles.is_empty()).then_some(titles.len())
}

/// Extrai "esquema://host" de uma URL (sem path).
fn base_origin(url: &str) -> String {
    let scheme = url.split("://").next().unwrap_or("https");
    let after = url.split("://").nth(1).unwrap_or(url);
    let host = after.split('/').next().unwrap_or(after);
    format!("{scheme}://{host}")
}

/// Procura no HTML o link de feed RSS/Atom declarado em <link rel="alternate">.
fn discover_feed_link(html: &str, base_url: &str) -> Option<String> {
    for chunk in html.split('<') {
        let low = chunk.to_lowercase();
        if !low.starts_with("link") {
            continue;
        }
        let tag = chunk.split('>').next().unwrap_or(chunk);
        let low_tag = tag.to_lowercase();
        if (low_tag.contains("application/rss+xml") || low_tag.contains("application/atom+xml"))
            && low_tag.contains("href")
        {
            if let Some(href) = extract_attr(tag, "href") {
                return Some(resolve_url(&href, base_url));
            }
        }
    }
    None
}

/// Extrai o valor de um atributo (aspas simples/duplas ou sem aspas).
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let low = tag.to_lowercase();
    let pos = low.find(&format!("{attr}="))?;
    let after = tag[pos + attr.len() + 1..].trim_start();
    let mut chars = after.chars();
    let first = chars.next()?;
    if first == '"' || first == '\'' {
        let rest = &after[1..];
        let end = rest.find(first)?;
        Some(rest[..end].to_string())
    } else {
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

/// Resolve uma URL possivelmente relativa contra a base.
fn resolve_url(href: &str, base: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let scheme = base.split("://").next().unwrap_or("https");
    let after = base.split("://").nth(1).unwrap_or(base);
    let host = after.split('/').next().unwrap_or(after);
    if let Some(rest) = href.strip_prefix('/') {
        format!("{scheme}://{host}/{rest}")
    } else {
        format!("{scheme}://{host}/{href}")
    }
}

/// Busca e parseia um feed RSS/Atom, devolvendo cada item com sua data REAL
/// de publicação (ver `extract_item_date`). Usado pelo pipeline de ingestão
/// para não carimbar "agora" numa notícia que na verdade é antiga.
pub(crate) async fn fetch_titles_with_dates(url: &str) -> Result<Vec<(String, Option<DateTime<Utc>>)>> {
    polite_delay().await;
    let client = browser_client()?;
    let xml = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(extract_titles_with_dates(&xml))
}

/// Como `fetch_best_text`, mas com a data real de publicação — ver
/// `fetch_titles_with_dates`.
pub(crate) async fn fetch_best_text_with_dates(url: &str) -> Result<Vec<(String, Option<DateTime<Utc>>)>> {
    polite_delay().await;
    let client = browser_client()?;
    let xml = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(extract_best_text_with_dates(&xml))
}

/// Idade máxima que uma notícia pode ter para ser tratada como "em tempo
/// real" pelo motor. Acima disso é descartada — sem isso, feeds que
/// resurfaceiam itens antigos (ex.: Google News com janela de "when:1d/2d",
/// ou um feed com cache defasado) fariam o app exibir notícias de dias atrás
/// como se fossem novas. Configurável via .env; padrão 24h.
fn max_news_age() -> chrono::Duration {
    let hours: i64 = std::env::var("MAX_NEWS_AGE_HOURS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    chrono::Duration::hours(hours.max(1))
}

/// True se a notícia deve ser aceita: sem data conhecida (não dá pra provar
/// que é velha, deixa passar) OU dentro da janela de "tempo real" configurada.
pub(crate) fn is_fresh_enough(published: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    match published {
        None => true,
        Some(dt) => now.signed_duration_since(dt) <= max_news_age(),
    }
}

async fn fetch_one(source: &str, url: &str) -> Result<Vec<RawNewsItem>> {
    let now = chrono::Utc::now();
    let mut stale = 0usize;
    let items = fetch_titles_with_dates(url)
        .await?
        .into_iter()
        .filter(|(h, _)| !h.is_empty() && is_high_impact_headline(h))
        .filter(|(_, published)| {
            let fresh = is_fresh_enough(*published, now);
            if !fresh {
                stale += 1;
            }
            fresh
        })
        .take(10)
        .map(|(headline, published)| RawNewsItem {
            source: source.to_string(),
            headline,
            actual: None,
            forecast: None,
            previous: None,
            impact_hint: "headline".to_string(),
            // Usa a data REAL de publicação do item; só recorre a "agora"
            // quando o feed não informa nenhuma data (ver is_fresh_enough).
            timestamp_utc: published.unwrap_or(now).to_rfc3339(),
        })
        .collect::<Vec<_>>();

    if stale > 0 {
        log::info!(
            "RSS '{source}': {stale} item(ns) descartado(s) por estar(em) fora \
             da janela de atualidade (> {}h)",
            std::env::var("MAX_NEWS_AGE_HOURS").unwrap_or_else(|_| "24".to_string())
        );
    }
    log::info!("RSS '{source}': {} manchete(s) de interesse", items.len());
    Ok(items)
}

/// Parser RSS/Atom minimalista (evita dependência extra de crate):
/// extrai o <title> de cada <item> (RSS) ou <entry> (Atom).
fn extract_titles(xml: &str) -> Vec<String> {
    extract_titles_with_dates(xml).into_iter().map(|(t, _)| t).collect()
}

/// Como `extract_titles`, mas também extrai a data REAL de publicação de cada
/// item (<pubDate> no RSS 2.0; <published>/<updated> no Atom), para que o
/// pipeline de ingestão saiba a idade verdadeira da notícia — nunca carimbe
/// "agora" para algo que na verdade é de dias atrás (ver MAX_NEWS_AGE_HOURS
/// em fetch_one/fetch_trump_mirror). None quando o item não traz nenhuma
/// dessas tags ou a data não pôde ser parseada.
fn extract_titles_with_dates(xml: &str) -> Vec<(String, Option<DateTime<Utc>>)> {
    let mut out = Vec::new();
    // RSS usa <item>…</item>; Atom usa <entry>…</entry>. Tratamos os dois.
    for (open, close) in [("<item", "</item>"), ("<entry", "</entry>")] {
        for chunk in xml.split(open).skip(1) {
            let block = chunk.split(close).next().unwrap_or(chunk);
            if let Some(title) = extract_tag(block, "title") {
                out.push((title, extract_item_date(block)));
            }
        }
    }
    out
}

/// Extrai e parseia a data de publicação de um item: tenta <pubDate> (RSS,
/// formato RFC 2822 — "Fri, 24 Jul 2026 15:08:26 GMT"), depois <published> e
/// <updated> (Atom, RFC 3339 — "2026-07-24T04:09:18+00:00"). Validado contra
/// amostras reais capturadas de Bloomberg/WSJ/FT/Reuters/Truth Social.
fn extract_item_date(block: &str) -> Option<DateTime<Utc>> {
    for tag in ["pubDate", "published", "updated"] {
        if let Some(raw) = extract_tag(block, tag) {
            let raw = raw.trim();
            if let Ok(dt) = DateTime::parse_from_rfc2822(raw) {
                return Some(dt.with_timezone(&Utc));
            }
            if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
                return Some(dt.with_timezone(&Utc));
            }
        }
    }
    None
}

/// Como `extract_titles`, mas quando o <title> de um item está vazio ou é o
/// placeholder padrão de agregadores para post sem legenda (começa com
/// "[No Title]"), usa o <description> (com tags HTML removidas) no lugar.
/// Ignora o item só se AMBOS title e description vierem vazios.
/// (Produção usa `extract_best_text_with_dates` diretamente; esta versão sem
/// data existe só para manter o teste abaixo simples de ler.)
#[cfg(test)]
fn extract_best_text(xml: &str) -> Vec<String> {
    extract_best_text_with_dates(xml).into_iter().map(|(t, _)| t).collect()
}

/// Como `extract_best_text`, mas também extrai a data real de publicação —
/// ver `extract_item_date`.
fn extract_best_text_with_dates(xml: &str) -> Vec<(String, Option<DateTime<Utc>>)> {
    let mut out = Vec::new();
    for (open, close) in [("<item", "</item>"), ("<entry", "</entry>")] {
        for chunk in xml.split(open).skip(1) {
            let block = chunk.split(close).next().unwrap_or(chunk);
            let title = extract_tag(block, "title").unwrap_or_default();
            let use_title = !title.is_empty() && !title.starts_with("[No Title]");
            let text = if use_title {
                title
            } else {
                extract_tag(block, "description")
                    .map(|d| strip_html_tags(&d))
                    .unwrap_or_default()
            };
            if !text.trim().is_empty() {
                out.push((text, extract_item_date(block)));
            }
        }
    }
    out
}

/// Remove marcações HTML (<p>, <span>, <br/>, ...) preservando o texto.
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    clean_text(&out)
}

/// Extrai o conteúdo textual da primeira ocorrência de <tag>…</tag>,
/// removendo CDATA e decodificando as entidades HTML mais comuns.
fn extract_tag(s: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = s.find(&open)?;
    let after = &s[start..];
    let content_start = after.find('>')? + 1;
    let close = format!("</{tag}>");
    let content_end = after.find(&close)?;
    if content_end < content_start {
        return None;
    }
    Some(clean_text(&after[content_start..content_end]))
}

fn clean_text(raw: &str) -> String {
    raw.replace("<![CDATA[", "")
        .replace("]]>", "")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#8217;", "'")
        .replace("&#8216;", "'")
        .replace("&#8220;", "\"")
        .replace("&#8221;", "\"")
        .replace("&#8211;", "-")
        .replace("&#8212;", "—")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_do_host() {
        assert_eq!(source_name_from_url("https://www.finance.yahoo.com/x"), "finance.yahoo.com");
        assert_eq!(source_name_from_url("https://feeds.marketwatch.com/mw"), "feeds.marketwatch.com");
    }

    #[test]
    fn autodescobre_link_do_feed() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" title="RSS" href="/news/rss.xml">
            </head><body>oi</body></html>"#;
        assert_eq!(
            discover_feed_link(html, "https://exemplo.com/pagina"),
            Some("https://exemplo.com/news/rss.xml".to_string())
        );
        // Sem link de feed -> None
        assert_eq!(discover_feed_link("<html><body>nada</body></html>", "https://x.com"), None);
    }

    #[test]
    fn resolve_urls() {
        assert_eq!(resolve_url("https://a.com/f.xml", "https://b.com"), "https://a.com/f.xml");
        assert_eq!(resolve_url("/rss", "https://b.com/pagina"), "https://b.com/rss");
        assert_eq!(resolve_url("rss.xml", "https://b.com"), "https://b.com/rss.xml");
    }

    #[test]
    fn extrai_atributo() {
        let tag = r#"link rel="alternate" type="application/rss+xml" href='/feed'"#;
        assert_eq!(extract_attr(tag, "href"), Some("/feed".to_string()));
    }

    #[test]
    fn strip_html_remove_tags_preserva_texto() {
        assert_eq!(
            strip_html_tags("<p>Ol\u{e1} <span>mundo</span> &amp; cia</p>"),
            "Olá mundo & cia"
        );
        assert_eq!(strip_html_tags("<br/>sem tags externas"), "sem tags externas");
    }

    #[test]
    fn extract_best_text_usa_description_quando_sem_titulo() {
        // Caso real do Trump mirror: repost sem legenda -> "[No Title]...".
        let xml = r#"<rss><channel>
            <item>
                <title><![CDATA[[No Title] - Post from July 24, 2026]]></title>
                <description><![CDATA[<p><span class="quote-inline">RT: algo importante sobre tarifas</span></p>]]></description>
            </item>
            <item>
                <title><![CDATA[Texto completo do post original sobre o Fed]]></title>
                <description><![CDATA[<p>Texto completo do post original sobre o Fed</p>]]></description>
            </item>
            <item>
                <title></title>
                <description></description>
            </item>
        </channel></rss>"#;
        let out = extract_best_text(xml);
        assert_eq!(out.len(), 2, "item totalmente vazio deve ser descartado");
        assert_eq!(out[0], "RT: algo importante sobre tarifas");
        assert_eq!(out[1], "Texto completo do post original sobre o Fed");
    }

    #[test]
    fn extrai_data_real_formatos_rss_e_atom() {
        // Amostras REAIS capturadas de Bloomberg/WSJ/FT/Reuters/Truth Social.
        let block_rss_gmt = "<pubDate>Thu, 23 Jul 2026 22:06:16 GMT</pubDate>";
        let block_rss_offset = "<pubDate>Fri, 24 Jul 2026 04:09:38 +0000</pubDate>";
        let block_atom = "<published>2026-07-24T04:09:18+00:00</published>";
        assert_eq!(
            extract_item_date(block_rss_gmt),
            Some("2026-07-23T22:06:16Z".parse().unwrap())
        );
        assert_eq!(
            extract_item_date(block_rss_offset),
            Some("2026-07-24T04:09:38Z".parse().unwrap())
        );
        assert_eq!(
            extract_item_date(block_atom),
            Some("2026-07-24T04:09:18Z".parse().unwrap())
        );
        assert_eq!(extract_item_date("<title>sem data</title>"), None);
    }

    #[test]
    fn is_fresh_enough_rejeita_noticia_velha_aceita_recente_e_sem_data() {
        std::env::set_var("MAX_NEWS_AGE_HOURS", "24");
        let now: DateTime<Utc> = "2026-07-31T10:00:00Z".parse().unwrap();

        // Caso real que motivou este teste: item do feed do WSJ datado de
        // janeiro de 2025 (mais de um ano antes de "agora") — sem o filtro,
        // isso seria exibido como notícia "de agora".
        let muito_velha: DateTime<Utc> = "2025-01-27T19:27:15Z".parse().unwrap();
        assert!(!is_fresh_enough(Some(muito_velha), now));

        // "Hoje é 31/07" — notícia de 29/07 (48h atrás) também deve ser
        // rejeitada com a janela padrão de 24h (era exatamente a reclamação
        // do usuário: notícia de 2 dias atrás sendo exibida como fresca).
        let dois_dias_atras = now - chrono::Duration::hours(48);
        assert!(!is_fresh_enough(Some(dois_dias_atras), now));

        // Notícia de 2h atrás: dentro da janela, deve passar.
        let duas_horas_atras = now - chrono::Duration::hours(2);
        assert!(is_fresh_enough(Some(duas_horas_atras), now));

        // Sem data conhecida: não dá pra provar que é velha, deixa passar.
        assert!(is_fresh_enough(None, now));

        std::env::remove_var("MAX_NEWS_AGE_HOURS");
    }

    #[test]
    fn extract_titles_with_dates_preserva_a_data_de_cada_item() {
        let xml = r#"<rss><channel>
            <item><title>CPI acima do esperado</title><pubDate>Fri, 24 Jul 2026 12:00:00 GMT</pubDate></item>
            <item><title>Nota antiga sem relevancia</title><pubDate>Mon, 27 Jan 2025 14:27:15 -0500</pubDate></item>
        </channel></rss>"#;
        let items = extract_titles_with_dates(xml);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, "CPI acima do esperado");
        assert_eq!(items[0].1, Some("2026-07-24T12:00:00Z".parse().unwrap()));
        // "Mon, 27 Jan 2025 14:27:15 -0500" == 19:27:15 UTC no mesmo dia.
        assert_eq!(items[1].1, Some("2025-01-27T19:27:15Z".parse().unwrap()));
    }

    #[test]
    fn base_origin_extrai_esquema_e_host() {
        assert_eq!(base_origin("https://exemplo.com/pagina?x=1"), "https://exemplo.com");
        assert_eq!(base_origin("http://a.b.com"), "http://a.b.com");
    }
}

#[cfg(test)]
mod network_verification {
    // Teste manual de rede (não faz parte da suíte padrão — depende de
    // internet). Confirma que a URL do Google News (com "(", "+", sem
    // encoding manual) funciona através do browser_client() REAL do projeto,
    // não só via curl. Rode com:
    //   cargo test --lib network_verification -- --ignored --nocapture
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn google_news_url_funciona_via_reqwest_real() {
        let url = "https://news.google.com/rss/search?q=site:reuters.com+(economy+OR+markets+OR+fed+OR+trump+OR+tariff)+when:2d&hl=en-US&gl=US&ceid=US:en";
        let items = fetch_titles_with_dates(url)
            .await
            .expect("fetch_titles_with_dates via reqwest real deve funcionar");
        println!("OK: {} titulos extraidos via reqwest real", items.len());
        for (t, d) in items.iter().take(3) {
            println!("  -> [{d:?}] {t}");
        }
        let titles: Vec<&String> = items.iter().map(|(t, _)| t).collect();
        assert!(titles.len() > 5, "esperava vários títulos, veio {}", titles.len());
    }
}
