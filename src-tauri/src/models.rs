use serde::{Deserialize, Serialize};

/// Item bruto capturado pelos scrapers, antes da análise do Gemini.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawNewsItem {
    pub source: String,
    pub headline: String,
    /// Dado divulgado (calendário econômico) — None para manchetes puras
    pub actual: Option<String>,
    pub forecast: Option<String>,
    pub previous: Option<String>,
    /// "3-star" | "headline" — usado no filtro de alto impacto
    pub impact_hint: String,
    pub timestamp_utc: String,
}

impl RawNewsItem {
    /// Hash estável para deduplicação (mesma manchete/dado não é analisado 2x).
    pub fn dedup_key(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.source.as_bytes());
        hasher.update(self.headline.as_bytes());
        hasher.update(self.actual.as_deref().unwrap_or("").as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Probabilidade direcional do ES retornada pelo Gemini.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionProbability {
    pub up: u8,
    pub down: u8,
}

/// Schema JSON ESTRITO exigido do Gemini. Qualquer resposta que não
/// desserialize neste struct é descartada e re-tentada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiAnalysis {
    pub source: String,
    pub event: String,
    /// "CRITICAL" | "HIGH" | "MEDIUM" | "LOW"
    pub impact_level: String,
    pub actual: String,
    pub forecast: String,
    pub previous: String,
    /// "BULLISH" | "BEARISH" | "NEUTRAL"
    pub sentiment: String,
    pub sp500_direction_probability: DirectionProbability,
    pub projected_target_pts: String,
    pub rationale: String,
    /// "HIGH_VOLATILITY" | "TREND_CONFIRMATION" | "REVERSAL_RISK" | "INFO"
    pub alert_type: String,
}

impl GeminiAnalysis {
    pub fn requires_native_alert(&self) -> bool {
        matches!(self.impact_level.as_str(), "CRITICAL" | "HIGH")
    }
}

/// Evento completo persistido no SQLite e enviado ao frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsEvent {
    pub id: i64,
    pub dedup_key: String,
    pub received_at_utc: String,
    #[serde(flatten)]
    pub analysis: GeminiAnalysis,
}
