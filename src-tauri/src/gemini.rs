use crate::models::{GeminiAnalysis, RawNewsItem};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    /// Lista de modelos a tentar, em ordem de preferência. O primeiro que
    /// responder (não-404) é memorizado em `model_idx` e usado nas próximas.
    models: Vec<String>,
    model_idx: Mutex<usize>,
    /// Controle de rate limit: instante da última chamada + intervalo mínimo.
    last_call: Mutex<Instant>,
    min_interval: Duration,
}

/// Resultado tipado de uma chamada, para o fallback decidir o que fazer:
/// 404 -> troca de modelo; 429 -> espera e re-tenta; demais -> transitório.
enum CallError {
    NotFound,
    RateLimited,
    Transient(anyhow::Error),
}

/// Modelos padrão tentados em cascata (nomes atuais da API Gemini).
fn default_models() -> Vec<String> {
    ["gemini-2.5-flash", "gemini-2.0-flash", "gemini-flash-latest", "gemini-2.5-flash-lite"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl GeminiClient {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .context("GEMINI_API_KEY não definida — configure o arquivo .env na raiz do projeto")?;

        // GEMINI_MODEL pode ser uma lista separada por vírgula. O que o
        // usuário definir é tentado primeiro; os padrões entram como fallback.
        let mut models: Vec<String> = std::env::var("GEMINI_MODEL")
            .unwrap_or_default()
            .split(',')
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .collect();
        for d in default_models() {
            if !models.contains(&d) {
                models.push(d);
            }
        }
        if models.is_empty() {
            models = default_models();
        }

        let min_interval_ms: u64 = std::env::var("GEMINI_MIN_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6000);

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            api_key,
            models,
            model_idx: Mutex::new(0),
            // Recuado no tempo p/ a 1ª chamada não esperar (checked_sub evita
            // underflow se a máquina tiver sido ligada há pouco tempo).
            last_call: Mutex::new(
                Instant::now()
                    .checked_sub(Duration::from_secs(3600))
                    .unwrap_or_else(Instant::now),
            ),
            min_interval: Duration::from_millis(min_interval_ms),
        })
    }

    /// Diagnóstico de startup: lista os modelos que ESTA chave pode usar com
    /// generateContent. Ajuda a descobrir o nome correto quando dá 404.
    pub async fn log_available_models(&self) {
        let url = "https://generativelanguage.googleapis.com/v1beta/models";
        let resp = match self
            .http
            .get(url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Gemini ListModels: request falhou: {e:#}");
                return;
            }
        };
        let status = resp.status();
        let payload: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Gemini ListModels (status {status}): falha ao parsear: {e:#}");
                return;
            }
        };
        match payload["models"].as_array() {
            Some(models) => {
                let names: Vec<String> = models
                    .iter()
                    .filter(|m| {
                        m["supportedGenerationMethods"]
                            .as_array()
                            .map(|arr| arr.iter().any(|s| s.as_str() == Some("generateContent")))
                            .unwrap_or(false)
                    })
                    .filter_map(|m| {
                        m["name"]
                            .as_str()
                            .map(|s| s.trim_start_matches("models/").to_string())
                    })
                    .collect();
                log::info!(
                    "Gemini: modelos disponíveis p/ generateContent nesta chave: [{}]",
                    names.join(", ")
                );
            }
            None => log::warn!(
                "Gemini ListModels (status {status}) sem lista de modelos — chave inválida? Resposta: {payload}"
            ),
        }
    }

    /// Rate limit: garante o intervalo mínimo entre chamadas ao Gemini,
    /// reservando o próximo "slot" para chamadas sequenciais não colidirem.
    async fn throttle(&self) {
        let wait = {
            let mut last = self.last_call.lock().expect("last_call mutex");
            let now = Instant::now();
            let elapsed = now.duration_since(*last);
            let w = self.min_interval.saturating_sub(elapsed);
            *last = now + w;
            w
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
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

        // Fallback em cascata: percorre a lista de modelos começando pelo
        // último que funcionou. 404 = modelo indisponível -> próximo. 429 =
        // rate limit -> espera e re-tenta o MESMO modelo. Ao ter sucesso,
        // memoriza o índice para as próximas chamadas irem direto.
        let n = self.models.len();
        let start = *self.model_idx.lock().expect("model_idx mutex");
        let mut last_err = anyhow!("nenhuma tentativa executada");

        for offset in 0..n {
            let idx = (start + offset) % n;
            let model = self.models[idx].clone();
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            );

            let mut rate_retries = 0u8;
            loop {
                self.throttle().await;
                match self.call_once(&url, &body).await {
                    Ok(analysis) => {
                        if idx != start {
                            log::info!("Gemini: usando modelo '{model}' a partir de agora");
                        }
                        *self.model_idx.lock().expect("model_idx mutex") = idx;
                        return Ok(analysis);
                    }
                    Err(CallError::NotFound) => {
                        log::warn!("Gemini: modelo '{model}' indisponível (404). Tentando o próximo…");
                        break; // passa para o próximo modelo
                    }
                    Err(CallError::RateLimited) => {
                        rate_retries += 1;
                        if rate_retries > 3 {
                            last_err = anyhow!("rate limit (429) persistente no modelo '{model}'");
                            break;
                        }
                        let secs = 5 * rate_retries as u64;
                        log::warn!("Gemini: rate limit (429). Aguardando {secs}s antes de re-tentar…");
                        tokio::time::sleep(Duration::from_secs(secs)).await;
                        // re-tenta o MESMO modelo
                    }
                    Err(CallError::Transient(e)) => {
                        log::warn!("Gemini: erro transitório no modelo '{model}': {e:#}");
                        last_err = e;
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        break; // tenta o próximo modelo
                    }
                }
            }
        }
        Err(last_err)
    }

    async fn call_once(&self, url: &str, body: &Value) -> Result<GeminiAnalysis, CallError> {
        let resp = self
            .http
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| CallError::Transient(e.into()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(CallError::NotFound);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(CallError::RateLimited);
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(CallError::Transient(anyhow!("HTTP {status}: {text}")));
        }

        let payload: Value = resp
            .json()
            .await
            .map_err(|e| CallError::Transient(e.into()))?;
        let text = payload["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                CallError::Transient(anyhow!("resposta do Gemini sem candidates/parts: {payload}"))
            })?;

        // Validação estrita: se não bater com o schema exigido, é transitório.
        serde_json::from_str(text.trim())
            .map_err(|e| CallError::Transient(anyhow!("JSON fora do schema ({e}): {text}")))
    }
}
