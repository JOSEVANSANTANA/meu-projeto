use crate::models::{GeminiAnalysis, RawNewsItem};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// System prompt RÍGIDO: força o Gemini a se comportar como analista
/// quantitativo e a responder exclusivamente com o JSON do schema exigido.
const SYSTEM_PROMPT: &str = r#"Você é um motor de análise quantitativa de notícias macroeconômicas especializado no S&P 500 Futuro (ticker ES).

REGRAS ABSOLUTAS — VIOLAÇÃO NÃO É PERMITIDA:
1. Responda ESTRITAMENTE com um único objeto JSON válido. Sem markdown, sem cercas de código, sem texto antes ou depois.
2. O JSON deve conter EXATAMENTE estes campos: source, event, impact_level, actual, forecast, previous, sentiment, sp500_direction_probability (objeto com "up" e "down" inteiros somando 100), projected_target_pts, rationale, alert_type.
3. impact_level: um de "CRITICAL", "HIGH", "MEDIUM", "LOW". Dados como CPI, Core CPI, Nonfarm Payrolls, decisão de juros do FOMC e falas do presidente do Fed com surpresa vs. consenso são "CRITICAL". Surpresas moderadas em PPI, GDP, Retail Sales, Jobless Claims são "HIGH".
4. sentiment: um de "BULLISH", "BEARISH", "NEUTRAL" — sempre da perspectiva do S&P 500 Futuro (ES), não da economia. Ex.: CPI acima do esperado = pressão de juros = tipicamente BEARISH para o ES.
5. projected_target_pts: estimativa de movimento no ES no formato "+15 pts", "-25 pts" ou "0 pts", calibrada pela magnitude da surpresa (actual vs. forecast) e histórico do evento.
6. rationale: máximo de 2 frases, direto, em português, citando a surpresa numérica quando existir.
7. alert_type: um de "HIGH_VOLATILITY", "TREND_CONFIRMATION", "REVERSAL_RISK", "INFO".
8. Se algum dado de entrada estiver ausente, use "N/A" no campo correspondente — nunca invente números.
9. Nunca inclua campos extras, comentários ou explicações fora do JSON."#;

/// Schema declarativo enviado em generationConfig.responseSchema —
/// segunda camada de garantia (além do prompt) de saída estruturada.
fn response_schema() -> Value {
    json!({
        "type": "OBJECT",
        "required": [
            "source", "event", "impact_level", "actual", "forecast", "previous",
            "sentiment", "sp500_direction_probability", "projected_target_pts",
            "rationale", "alert_type"
        ],
        "properties": {
            "source":       { "type": "STRING" },
            "event":        { "type": "STRING" },
            "impact_level": { "type": "STRING", "enum": ["CRITICAL", "HIGH", "MEDIUM", "LOW"] },
            "actual":       { "type": "STRING" },
            "forecast":     { "type": "STRING" },
            "previous":     { "type": "STRING" },
            "sentiment":    { "type": "STRING", "enum": ["BULLISH", "BEARISH", "NEUTRAL"] },
            "sp500_direction_probability": {
                "type": "OBJECT",
                "required": ["up", "down"],
                "properties": {
                    "up":   { "type": "INTEGER" },
                    "down": { "type": "INTEGER" }
                }
            },
            "projected_target_pts": { "type": "STRING" },
            "rationale":            { "type": "STRING" },
            "alert_type": {
                "type": "STRING",
                "enum": ["HIGH_VOLATILITY", "TREND_CONFIRMATION", "REVERSAL_RISK", "INFO"]
            }
        }
    })
}

pub struct GeminiClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .context("GEMINI_API_KEY não definida — configure o arquivo .env na raiz do projeto")?;
        let model =
            std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http, api_key, model })
    }

    /// Envia o contexto da notícia (Atual vs. Projeção vs. Anterior) e
    /// retorna a análise estruturada. Faz até 2 re-tentativas se a resposta
    /// não desserializar no schema exigido.
    pub async fn analyze(&self, item: &RawNewsItem) -> Result<GeminiAnalysis> {
        let user_context = format!(
            "FONTE: {}\nEVENTO/MANCHETE: {}\nDADO ATUAL (actual): {}\nPROJEÇÃO (forecast): {}\nANTERIOR (previous): {}\nHORÁRIO (UTC): {}\n\nAnalise o impacto imediato no S&P 500 Futuro (ES) e responda com o JSON exigido.",
            item.source,
            item.headline,
            item.actual.as_deref().unwrap_or("N/A"),
            item.forecast.as_deref().unwrap_or("N/A"),
            item.previous.as_deref().unwrap_or("N/A"),
            item.timestamp_utc,
        );

        let body = json!({
            "system_instruction": {
                "parts": [{ "text": SYSTEM_PROMPT }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": user_context }]
            }],
            "generationConfig": {
                "temperature": 0.1,
                "response_mime_type": "application/json",
                "response_schema": response_schema()
            }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        let mut last_err = anyhow!("sem tentativas executadas");
        for attempt in 1..=3u8 {
            match self.call_once(&url, &body).await {
                Ok(analysis) => return Ok(analysis),
                Err(e) => {
                    log::warn!("Gemini tentativa {attempt}/3 falhou: {e:#}");
                    last_err = e;
                    tokio::time::sleep(Duration::from_millis(1500 * attempt as u64)).await;
                }
            }
        }
        Err(last_err)
    }

    async fn call_once(&self, url: &str, body: &Value) -> Result<GeminiAnalysis> {
        let resp = self
            .http
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .json(body)
            .send()
            .await?
            .error_for_status()?;

        let payload: Value = resp.json().await?;
        let text = payload["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow!("resposta do Gemini sem candidates/parts: {payload}"))?;

        // Validação estrita: se não bater com o schema, é erro (e re-tenta).
        let analysis: GeminiAnalysis = serde_json::from_str(text.trim())
            .with_context(|| format!("JSON fora do schema exigido: {text}"))?;
        Ok(analysis)
    }
}
