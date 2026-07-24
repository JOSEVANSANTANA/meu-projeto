pub mod investing;
pub mod rss;
pub mod truth_social;

use rand::Rng;
use std::time::Duration;

/// REGRA CRÍTICA ANTI-BOT: delay aleatório entre 2 e 5 segundos
/// (configurável via .env) antes de CADA requisição HTTP de scraping.
/// Protege o IP residencial contra rate-limit e bloqueios.
pub async fn polite_delay() {
    let min: u64 = std::env::var("MIN_REQUEST_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let max: u64 = std::env::var("MAX_REQUEST_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);
    let ms = rand::thread_rng().gen_range(min..=max.max(min + 1));
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// User-Agent de navegador real + headers coerentes: requisições "nuas"
/// do reqwest são o primeiro gatilho de bloqueio anti-bot.
pub fn browser_client() -> reqwest::Result<reqwest::Client> {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE};
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

    reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        )
        .default_headers(headers)
        .cookie_store(true)
        .gzip(true)
        .timeout(Duration::from_secs(20))
        .build()
}

/// Filtro de alto impacto: só interessa o que move o ES.
/// Calendário: eventos 3 estrelas. Manchetes: keywords macro críticas.
pub fn is_high_impact_headline(headline: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        // Inflação
        "cpi", "core cpi", "consumer price", "inflation", "ppi",
        "producer price", "pce",
        // Emprego (o release oficial do NFP se chama "Employment Situation")
        "payroll", "nonfarm", "nfp", "employment situation", "unemployment",
        "jobless", "initial claims",
        // Fed / juros
        "fomc", "fed ", "federal reserve", "powell", "rate decision",
        "interest rate", "rate cut", "rate hike", "fed funds", "dot plot",
        "fomc minutes", "beige book", "discount rate",
        // Atividade / macro
        "gdp", "gross domestic product", "retail sales", "ism", "pmi",
        "consumer confidence", "recession",
        // Mercado / risco
        "treasury", "yield", "tariff", "sanction", "opec", "crude",
        // Político / geopolítico
        "trump", "white house", "geopolit", "war",
    ];
    let h = headline.to_lowercase();
    KEYWORDS.iter().any(|k| h.contains(k)) && !is_market_noise(&h)
}

/// Descarta manchetes regulatórias/administrativas que casam nas keywords
/// (ex.: "Federal Reserve") mas NÃO movem o mercado — economiza cota da API
/// do Gemini. Recebe a manchete já em minúsculas.
fn is_market_noise(headline_lower: &str) -> bool {
    const NOISE: &[&str] = &[
        "enforcement",           // "issues enforcement action with ..."
        "anti-money laundering",
        "bank secrecy",
        "passing of",            // notas de falecimento
        "personnel",
        "appoints",
        "elects",
        "nomination",
        "designation of",
    ];
    NOISE.iter().any(|k| headline_lower.contains(k))
}
