use crate::models::{GeminiAnalysis, RawNewsItem};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// System prompt RÍGIDO + TRAVA ANTI-ALUCINAÇÃO. É parametrizado pelo ativo
/// prioritário escolhido pelo usuário (ES/NQ/etc.), mas o schema de saída é
/// sempre o mesmo. A regra central: analisar SÓ o que está no texto — nunca
/// inventar números, consenso ou eventos.
fn system_prompt(asset: &str) -> String {
    format!(
        r#"Você é um motor de análise quantitativa de notícias macroeconômicas focado no ATIVO PRIORITÁRIO desta sessão: {asset}.

REGRAS ABSOLUTAS — VIOLAÇÃO NÃO É PERMITIDA:
1. Responda ESTRITAMENTE com um único objeto JSON válido. Sem markdown, sem cercas de código, sem texto antes ou depois.
2. O JSON deve conter EXATAMENTE estes campos: source, event, impact_level, actual, forecast, previous, sentiment, sp500_direction_probability (objeto com "up" e "down" inteiros somando 100), projected_target_pts, rationale, alert_type.
3. IDIOMA: TODO texto de saída deve estar em PORTUGUÊS BRASILEIRO. O campo "event" deve ser a TRADUÇÃO fiel e concisa da manchete original para o português (traduza, não invente conteúdo). O "rationale" também em português.
4. O campo sp500_direction_probability representa a probabilidade direcional do ATIVO PRIORITÁRIO ({asset}) — mesmo que o nome do campo mencione sp500. sentiment e projected_target_pts também se referem a {asset}.
5. CORRELAÇÃO POR ATIVO: analise o impacto ESPECÍFICO para {asset}. Índices reagem diferente à mesma notícia: Nasdaq-100 (NQ/MNQ) tem forte peso em tecnologia e é MAIS sensível a juros; Russell 2000 (RTY/M2K) é small cap doméstico, sensível a crédito e ciclo interno dos EUA; Dow (YM/MYM) é industrial/valor; S&P 500 (ES/MES) é amplo; Nikkei (NKD) segue o Japão e o iene. Calibre sentiment, probabilidade e pontos conforme o ativo escolhido.
6. impact_level: um de "CRITICAL", "HIGH", "MEDIUM", "LOW". CPI, Core CPI, Nonfarm Payrolls, decisão de juros do FOMC e falas do presidente do Fed com surpresa vs. consenso são "CRITICAL". Surpresas moderadas em PPI, GDP, Retail Sales, Jobless Claims são "HIGH".
7. sentiment: um de "BULLISH", "BEARISH", "NEUTRAL" — da perspectiva de {asset}, não da economia. Ex.: CPI acima do esperado = pressão de juros = tipicamente BEARISH para índices acionários.
8. projected_target_pts: estimativa de movimento em PONTOS de {asset} no formato "+15 pts", "-25 pts" ou "0 pts", calibrada pela magnitude da surpresa (actual vs. forecast).
9. rationale: máximo de 2 frases, direto, em português.
10. alert_type: um de "HIGH_VOLATILITY", "TREND_CONFIRMATION", "REVERSAL_RISK", "INFO".

TRAVA ANTI-ALUCINAÇÃO (CRÍTICO — a precisão vale mais que a ousadia):
A. Baseie-se EXCLUSIVAMENTE no texto fornecido (manchete + actual/forecast/previous). É PROIBIDO inventar números, percentuais, valores de consenso, datas ou eventos que não estejam explicitamente na entrada.
B. Se actual/forecast/previous vierem "N/A", NÃO fabrique surpresa numérica; avalie apenas o teor qualitativo da manchete e reflita essa incerteza reduzindo a magnitude e a confiança.
C. Se a manchete for ambígua, genérica, ou não claramente ligada a {asset} ou à macroeconomia dos EUA, retorne impact_level "LOW", sentiment "NEUTRAL", probabilidade 50/50 e projected_target_pts "0 pts", explicando a incerteza no rationale.
D. NÃO afirme relação de causa que não possa ser inferida do próprio texto. Na dúvida, prefira "NEUTRAL" a especular.
E. O rationale deve citar SOMENTE informação presente na entrada. É proibido citar números que não foram fornecidos.
F. A probabilidade direcional deve ser proporcional à força REAL da evidência: sem dado numérico de surpresa, mantenha-se próximo de 50/50 (ex.: no máximo 60/40), reservando extremos (80/20+) para surpresas numéricas claras.
G. Nunca inclua campos extras, comentários ou explicações fora do JSON."#
    )
}

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

/// Cliente Gemini compartilhado (Arc no AppState). Guarda, com mutabilidade
/// interior, o pool de API keys (com rotação quando uma esgota) e o ativo
/// prioritário — ambos ajustáveis em runtime pela UI.
pub struct GeminiClient {
    http: reqwest::Client,
    models: Vec<String>,
    model_idx: Mutex<usize>,
    last_call: Mutex<Instant>,
    min_interval: Duration,
    keys: Mutex<Vec<String>>,
    active_key: Mutex<usize>,
    asset: Mutex<String>,
}

enum CallError {
    NotFound,
    RateLimited,
    Transient(anyhow::Error),
}

/// Modelos candidatos, em ordem de preferência (família flash). O startup
/// sonda e trava no primeiro que responder 200 para a chave ativa.
fn default_models() -> Vec<String> {
    [
        "gemini-flash-latest",
        "gemini-2.0-flash",
        "gemini-2.0-flash-001",
        "gemini-flash-lite-latest",
        "gemini-2.5-flash-lite",
        "gemini-2.5-flash",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl GeminiClient {
    pub fn from_env() -> Result<Self> {
        // GEMINI_API_KEY aceita UMA ou VÁRIAS chaves separadas por vírgula.
        // Ausência de chave NÃO é fatal: o app abre e você pode colar a chave
        // no campo do dashboard (o motor fica offline até haver uma chave).
        let keys: Vec<String> = std::env::var("GEMINI_API_KEY")
            .unwrap_or_default()
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        if keys.is_empty() {
            log::warn!(
                "GEMINI_API_KEY não definida — configure no .env ou cole a chave no dashboard. \
                 O motor ficará OFFLINE até haver uma chave."
            );
        }

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

        let asset = std::env::var("PRIORITY_ASSET")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "S&P 500 Futuro (ES)".to_string());

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .connect_timeout(Duration::from_secs(15))
            .build()?;

        Ok(Self {
            http,
            models,
            model_idx: Mutex::new(0),
            last_call: Mutex::new(
                Instant::now()
                    .checked_sub(Duration::from_secs(3600))
                    .unwrap_or_else(Instant::now),
            ),
            min_interval: Duration::from_millis(min_interval_ms),
            keys: Mutex::new(keys),
            active_key: Mutex::new(0),
            asset: Mutex::new(asset),
        })
    }

    // ---- Configuração em runtime (usada pelos commands da UI) ----------------

    /// Adiciona uma nova API key ao pool (para quando uma esgota a cota).
    /// Retorna o total de chaves. Ignora duplicadas e vazias.
    pub fn add_key(&self, key: &str) -> usize {
        let key = key.trim().to_string();
        let mut keys = self.keys.lock().expect("keys mutex");
        if !key.is_empty() && !keys.contains(&key) {
            keys.push(key);
        }
        keys.len()
    }

    pub fn keys_len(&self) -> usize {
        self.keys.lock().expect("keys mutex").len()
    }

    pub fn has_keys(&self) -> bool {
        !self.keys.lock().expect("keys mutex").is_empty()
    }

    pub fn active_key_index(&self) -> usize {
        *self.active_key.lock().expect("active_key mutex")
    }

    fn current_key(&self) -> String {
        let keys = self.keys.lock().expect("keys mutex");
        let idx = *self.active_key.lock().expect("active_key mutex");
        keys.get(idx).cloned().unwrap_or_default()
    }

    /// Avança para a próxima chave do pool (cíclico). Retorna true se de fato
    /// trocou para uma chave diferente (i.e., há mais de uma).
    pub fn rotate_key(&self) -> bool {
        let len = self.keys.lock().expect("keys mutex").len();
        if len <= 1 {
            return false;
        }
        let mut idx = self.active_key.lock().expect("active_key mutex");
        *idx = (*idx + 1) % len;
        true
    }

    pub fn set_asset(&self, asset: &str) {
        let a = asset.trim();
        if !a.is_empty() {
            *self.asset.lock().expect("asset mutex") = a.to_string();
        }
    }

    pub fn asset(&self) -> String {
        self.asset.lock().expect("asset mutex").clone()
    }

    // ---- Diagnóstico + calibração -------------------------------------------

    pub async fn log_available_models(&self) {
        let api_key = self.current_key();
        let url = "https://generativelanguage.googleapis.com/v1beta/models";
        let resp = match self.http.get(url).header("x-goog-api-key", &api_key).send().await {
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
                        m["name"].as_str().map(|s| s.trim_start_matches("models/").to_string())
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

    /// Sonda cada modelo candidato com uma chamada REAL (mesma generationConfig
    /// de produção) usando a chave ativa e FIXA o primeiro que responder 200.
    pub async fn calibrate(&self) -> Result<String> {
        let api_key = self.current_key();
        let probe_body = json!({
            "system_instruction": { "parts": [{ "text": system_prompt(&self.asset()) }] },
            "contents": [{
                "role": "user",
                "parts": [{ "text": "Teste de disponibilidade. FONTE: teste; EVENTO/MANCHETE: ping; ATUAL: N/A; PROJEÇÃO: N/A; ANTERIOR: N/A. Responda com o JSON exigido." }]
            }],
            "generationConfig": {
                "temperature": 0,
                "response_mime_type": "application/json",
                "response_schema": response_schema()
            }
        });

        let mut fallback_429: Option<usize> = None;
        let mut last_err = anyhow!("nenhum modelo candidato testado");

        for (idx, model) in self.models.iter().enumerate() {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            );
            self.throttle().await;
            match self.probe(&url, &probe_body, &api_key).await {
                Ok(()) => {
                    *self.model_idx.lock().expect("model_idx mutex") = idx;
                    log::info!("Gemini: modelo selecionado para esta sessão -> '{model}'");
                    return Ok(model.clone());
                }
                Err(CallError::NotFound) => {
                    log::info!("Gemini: '{model}' sem generateContent p/ esta chave (404), pulando");
                }
                Err(CallError::RateLimited) => {
                    log::warn!("Gemini: '{model}' respondeu 429 no probe (existe, mas limitado agora)");
                    fallback_429.get_or_insert(idx);
                }
                Err(CallError::Transient(e)) => {
                    log::warn!("Gemini: probe de '{model}' falhou: {e:#}");
                    last_err = e;
                }
            }
        }

        if let Some(idx) = fallback_429 {
            *self.model_idx.lock().expect("model_idx mutex") = idx;
            let model = self.models[idx].clone();
            log::warn!(
                "Gemini: nenhum modelo respondeu 200 agora; usando '{model}' (limitado por cota)"
            );
            return Ok(model);
        }
        Err(last_err)
    }

    async fn probe(&self, url: &str, body: &Value, api_key: &str) -> Result<(), CallError> {
        let resp = self
            .http
            .post(url)
            .header("x-goog-api-key", api_key)
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
        Ok(())
    }

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

    // ---- Análise ------------------------------------------------------------

    /// Envia o contexto da notícia e retorna a análise estruturada, com
    /// fallback de modelos e respeito ao rate limit. Usa a chave ativa e o
    /// ativo prioritário correntes.
    pub async fn analyze(&self, item: &RawNewsItem) -> Result<GeminiAnalysis> {
        let api_key = self.current_key();
        let asset = self.asset();

        let user_context = format!(
            "ATIVO PRIORITÁRIO: {asset}\nFONTE: {}\nEVENTO/MANCHETE: {}\nDADO ATUAL (actual): {}\nPROJEÇÃO (forecast): {}\nANTERIOR (previous): {}\nHORÁRIO (UTC): {}\n\nAnalise o impacto imediato em {asset} e responda com o JSON exigido, seguindo a trava anti-alucinação.",
            item.source,
            item.headline,
            item.actual.as_deref().unwrap_or("N/A"),
            item.forecast.as_deref().unwrap_or("N/A"),
            item.previous.as_deref().unwrap_or("N/A"),
            item.timestamp_utc,
        );

        let body = json!({
            "system_instruction": { "parts": [{ "text": system_prompt(&asset) }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_context }] }],
            "generationConfig": {
                "temperature": 0.0,
                "response_mime_type": "application/json",
                "response_schema": response_schema()
            }
        });

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
                match self.call_once(&url, &body, &api_key).await {
                    Ok(analysis) => {
                        if idx != start {
                            log::info!("Gemini: usando modelo '{model}' a partir de agora");
                        }
                        *self.model_idx.lock().expect("model_idx mutex") = idx;
                        return Ok(analysis);
                    }
                    Err(CallError::NotFound) => {
                        log::warn!("Gemini: modelo '{model}' indisponível (404). Tentando o próximo…");
                        break;
                    }
                    Err(CallError::RateLimited) => {
                        rate_retries += 1;
                        if rate_retries > 3 {
                            last_err =
                                anyhow!("rate limit (429) persistente no modelo '{model}'");
                            break;
                        }
                        let secs = 5 * rate_retries as u64;
                        log::warn!("Gemini: rate limit (429). Aguardando {secs}s antes de re-tentar…");
                        tokio::time::sleep(Duration::from_secs(secs)).await;
                    }
                    Err(CallError::Transient(e)) => {
                        log::warn!("Gemini: erro transitório no modelo '{model}': {e:#}");
                        last_err = e;
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        break;
                    }
                }
            }
        }
        Err(last_err)
    }

    async fn call_once(
        &self,
        url: &str,
        body: &Value,
        api_key: &str,
    ) -> Result<GeminiAnalysis, CallError> {
        let resp = self
            .http
            .post(url)
            .header("x-goog-api-key", api_key)
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

        let payload: Value = resp.json().await.map_err(|e| CallError::Transient(e.into()))?;
        let text = payload["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                CallError::Transient(anyhow!("resposta do Gemini sem candidates/parts: {payload}"))
            })?;

        serde_json::from_str(text.trim())
            .map_err(|e| CallError::Transient(anyhow!("JSON fora do schema ({e}): {text}")))
    }
}
