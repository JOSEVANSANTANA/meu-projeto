use crate::models::RawNewsItem;
use crate::scrapers::{browser_client, is_high_impact_headline, polite_delay};
use anyhow::Result;

/// Ingestão por RSS — a via ROBUSTA de coleta.
///
/// Feeds RSS são projetados para leitura por máquina: não retornam 403,
/// não exigem chave e não dependem de JavaScript (ao contrário de raspar
/// o HTML do Investing.com/FinancialJuice, que o Cloudflare bloqueia).
///
/// Fontes padrão (todas gratuitas, macro, de alta credibilidade):
///   - Federal Reserve  -> decisões do FOMC, juros, discursos (oficial)
///   - MarketWatch      -> manchetes de mercado em tempo real (inclui CPI/NFP)
///   - CNBC (Economia)  -> cobertura macro dos EUA
///
/// (O feed do BLS foi removido dos padrões: bls.gov retorna 403 a este
/// cliente. CPI e Payroll ainda chegam pela cobertura de MarketWatch/CNBC.)
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

/// Valida uma URL adicionada pelo usuário: tenta parsear como RSS/Atom e, se
/// for uma página HTML, AUTODESCOBRE o link do feed (`<link rel="alternate"
/// type="application/rss+xml" href="...">`). Retorna (url_final_do_feed,
/// nº de itens crus encontrados). Erro se não achar um feed.
pub(crate) async fn probe_feed(url: &str) -> Result<(String, usize)> {
    let client = browser_client()?;
    let body = client.get(url).send().await?.error_for_status()?.text().await?;

    let titles = extract_titles(&body);
    if !titles.is_empty() {
        return Ok((url.to_string(), titles.len()));
    }

    if let Some(feed_url) = discover_feed_link(&body, url) {
        let body2 = client
            .get(&feed_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let titles2 = extract_titles(&body2);
        if !titles2.is_empty() {
            return Ok((feed_url, titles2.len()));
        }
    }
    Err(anyhow::anyhow!(
        "nenhum feed RSS/Atom encontrado nessa URL — tente colar a URL direta do feed"
    ))
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

/// Busca e parseia um feed RSS/Atom, devolvendo os títulos crus (sem filtro).
/// Reutilizado por outros conectores (ex.: Truth Social) que aplicam o próprio
/// critério de relevância.
pub(crate) async fn fetch_titles(url: &str) -> Result<Vec<String>> {
    polite_delay().await; // anti-bot: 2–5s aleatórios antes de cada request
    let client = browser_client()?;
    let xml = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(extract_titles(&xml))
}

async fn fetch_one(source: &str, url: &str) -> Result<Vec<RawNewsItem>> {
    let now = chrono::Utc::now().to_rfc3339();
    let items = fetch_titles(url)
        .await?
        .into_iter()
        .filter(|h| !h.is_empty() && is_high_impact_headline(h))
        .take(10)
        .map(|headline| RawNewsItem {
            source: source.to_string(),
            headline,
            actual: None,
            forecast: None,
            previous: None,
            impact_hint: "headline".to_string(),
            timestamp_utc: now.clone(),
        })
        .collect::<Vec<_>>();

    log::info!("RSS '{source}': {} manchete(s) de interesse", items.len());
    Ok(items)
}

/// Parser RSS/Atom minimalista (evita dependência extra de crate):
/// extrai o <title> de cada <item> (RSS) ou <entry> (Atom).
fn extract_titles(xml: &str) -> Vec<String> {
    let mut titles = Vec::new();
    // RSS usa <item>…</item>; Atom usa <entry>…</entry>. Tratamos os dois.
    for (open, close) in [("<item", "</item>"), ("<entry", "</entry>")] {
        for chunk in xml.split(open).skip(1) {
            let block = chunk.split(close).next().unwrap_or(chunk);
            if let Some(title) = extract_tag(block, "title") {
                titles.push(title);
            }
        }
    }
    titles
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
}
