use crate::models::RawNewsItem;
use crate::scrapers::{browser_client, is_high_impact_headline, polite_delay};
use anyhow::Result;

/// Manchetes rápidas opcionais.
///
/// Truth Social (posts do Trump) via espelho RSS público — a plataforma
/// não expõe API pública. Configure TRUTH_SOCIAL_RSS_URL no .env apontando
/// para um espelho RSS de sua preferência; sem ela, o conector fica inerte.
///
/// (O scrape do FinancialJuice foi removido: o site renderiza as manchetes
/// via JavaScript/WebSocket, então o HTML estático vem vazio. As manchetes
/// macro agora vêm dos feeds RSS confiáveis em `scrapers::rss`.)
pub async fn fetch_headlines() -> Result<Vec<RawNewsItem>> {
    match fetch_truth_social_rss().await {
        Ok(v) => Ok(v),
        Err(e) => {
            log::warn!("Truth Social RSS indisponível neste ciclo: {e:#}");
            Ok(Vec::new())
        }
    }
}

async fn fetch_truth_social_rss() -> Result<Vec<RawNewsItem>> {
    let url = match std::env::var("TRUTH_SOCIAL_RSS_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return Ok(Vec::new()), // conector opcional — desligado sem URL
    };

    polite_delay().await; // anti-bot: 2–5s aleatórios

    let client = browser_client()?;
    let xml = client.get(&url).send().await?.error_for_status()?.text().await?;

    // Parse RSS minimalista (evita dependência extra): extrai <title> dos <item>
    let now = chrono::Utc::now().to_rfc3339();
    let items = xml
        .split("<item>")
        .skip(1)
        .filter_map(|chunk| {
            let title = chunk.split("<title>").nth(1)?.split("</title>").next()?;
            let clean = title
                .replace("<![CDATA[", "")
                .replace("]]>", "")
                .trim()
                .to_string();
            (!clean.is_empty()).then_some(clean)
        })
        .filter(|h| is_high_impact_headline(h))
        .take(10)
        .map(|headline| RawNewsItem {
            source: "Truth Social (Trump)".to_string(),
            headline,
            actual: None,
            forecast: None,
            previous: None,
            impact_hint: "headline".to_string(),
            timestamp_utc: now.clone(),
        })
        .collect::<Vec<_>>();

    log::info!("Truth Social: {} post(s) de alto impacto", items.len());
    Ok(items)
}
