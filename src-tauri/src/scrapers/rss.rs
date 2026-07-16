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
///   - U.S. BLS         -> CPI e Employment Situation/Payroll (oficial)
///   - MarketWatch      -> manchetes de mercado em tempo real
///   - CNBC (Economia)  -> cobertura macro dos EUA
///
/// Você pode acrescentar/trocar feeds via variável RSS_FEEDS no .env
/// (formato: "Nome|url,Nome|url").
fn default_feeds() -> Vec<(String, String)> {
    vec![
        (
            "Federal Reserve".into(),
            "https://www.federalreserve.gov/feeds/press_all.xml".into(),
        ),
        (
            "U.S. BLS".into(),
            "https://www.bls.gov/feed/bls_latest.rss".into(),
        ),
        (
            "MarketWatch".into(),
            "https://feeds.marketwatch.com/marketwatch/realtimeheadlines/".into(),
        ),
        (
            "CNBC Economy".into(),
            "https://search.cnbc.com/rs/search/combinedcms/view.xml?partnerId=wrss01&id=20910258"
                .into(),
        ),
    ]
}

/// Lê feeds extras do .env (RSS_FEEDS) e os acrescenta aos padrões.
fn feeds() -> Vec<(String, String)> {
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
    list
}

/// Busca todos os feeds em sequência (cada um com polite_delay anti-bot)
/// e devolve apenas manchetes que passam no filtro de alto/médio impacto.
pub async fn fetch_all() -> Result<Vec<RawNewsItem>> {
    let mut items = Vec::new();
    for (source, url) in feeds() {
        match fetch_one(&source, &url).await {
            Ok(mut v) => items.append(&mut v),
            Err(e) => log::warn!("RSS '{source}' indisponível neste ciclo: {e:#}"),
        }
    }
    log::info!("RSS: {} manchete(s) macro no total", items.len());
    Ok(items)
}

async fn fetch_one(source: &str, url: &str) -> Result<Vec<RawNewsItem>> {
    polite_delay().await; // anti-bot: 2–5s aleatórios antes de cada request

    let client = browser_client()?;
    let xml = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let now = chrono::Utc::now().to_rfc3339();
    let items = extract_titles(&xml)
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
