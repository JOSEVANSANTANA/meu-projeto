use crate::models::RawNewsItem;
use crate::scrapers::{browser_client, is_high_impact_headline, polite_delay};
use anyhow::Result;
use scraper::{Html, Selector};

/// Feed de manchetes rápidas (squawk).
///
/// Fontes cobertas neste conector:
///   1. FinancialJuice — manchetes macro em tempo quase real
///   2. Truth Social (posts do Trump) — via espelho RSS público, já que a
///      plataforma não expõe API pública. Configure TRUTH_SOCIAL_RSS_URL
///      no .env apontando para um espelho RSS de sua preferência.
///
/// Ambas passam pelo filtro de keywords de alto impacto ANTES de
/// qualquer chamada ao Gemini.
pub async fn fetch_headlines() -> Result<Vec<RawNewsItem>> {
    let mut items = Vec::new();

    match fetch_financial_juice().await {
        Ok(mut v) => items.append(&mut v),
        Err(e) => log::warn!("FinancialJuice indisponível neste ciclo: {e:#}"),
    }

    match fetch_truth_social_rss().await {
        Ok(mut v) => items.append(&mut v),
        Err(e) => log::warn!("Truth Social RSS indisponível neste ciclo: {e:#}"),
    }

    Ok(items)
}

async fn fetch_financial_juice() -> Result<Vec<RawNewsItem>> {
    polite_delay().await; // anti-bot: 2–5s aleatórios

    let client = browser_client()?;
    let html = client
        .get("https://www.financialjuice.com/home")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let doc = Html::parse_document(&html);
    // NOTA: seletor sujeito a mudanças no site — validar periodicamente.
    let headline_sel = Selector::parse(".headline-title, .news-item .title").unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let items = doc
        .select(&headline_sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|h| !h.is_empty() && is_high_impact_headline(h))
        .take(15)
        .map(|headline| RawNewsItem {
            source: "FinancialJuice".to_string(),
            headline,
            actual: None,
            forecast: None,
            previous: None,
            impact_hint: "headline".to_string(),
            timestamp_utc: now.clone(),
        })
        .collect::<Vec<_>>();

    log::info!("FinancialJuice: {} manchete(s) de alto impacto", items.len());
    Ok(items)
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
