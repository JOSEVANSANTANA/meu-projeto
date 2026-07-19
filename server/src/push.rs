//! Envio de push nativo para iPhone via APNs (token-based, chave .p8).
//!
//! Configuração via variáveis de ambiente:
//!   APNS_TEAM_ID  -> Team ID da sua conta Apple Developer (ex.: AB12CD34EF)
//!   APNS_KEY_ID   -> Key ID da chave .p8 criada no portal (ex.: XYZ987)
//!   APNS_P8_PATH  -> caminho do arquivo AuthKey_XYZ987.p8
//!   APNS_TOPIC    -> bundle id do app iOS (ex.: com.esf.newsmonitor)
//!   APNS_SANDBOX  -> "1" para builds de desenvolvimento/TestFlight interno
//!
//! Sem essas variáveis, o push roda em DRY-RUN (loga o que enviaria) — assim o
//! servidor funciona de ponta a ponta antes de você ter a conta Apple.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ApnsConfig {
    team_id: String,
    key_id: String,
    p8_pem: String,
    topic: String,
    host: &'static str,
}

pub struct Pusher {
    http: reqwest::Client,
    config: Option<ApnsConfig>,
    /// JWT do APNs é válido por até 1h; cache com renovação a cada 45 min.
    jwt_cache: Mutex<Option<(String, Instant)>>,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    iat: u64,
}

impl Pusher {
    pub fn from_env() -> Self {
        let config = Self::load_config();
        match &config {
            Some(c) => log::info!("APNs configurado (topic {}, host {})", c.topic, c.host),
            None => log::warn!(
                "APNs NÃO configurado (APNS_TEAM_ID/KEY_ID/P8_PATH/TOPIC ausentes) — push em DRY-RUN"
            ),
        }
        Self {
            http: reqwest::Client::builder()
                .http2_prior_knowledge() // APNs exige HTTP/2
                .timeout(Duration::from_secs(15))
                .build()
                .expect("client http2"),
            config,
            jwt_cache: Mutex::new(None),
        }
    }

    fn load_config() -> Option<ApnsConfig> {
        let team_id = std::env::var("APNS_TEAM_ID").ok()?.trim().to_string();
        let key_id = std::env::var("APNS_KEY_ID").ok()?.trim().to_string();
        let p8_path = std::env::var("APNS_P8_PATH").ok()?.trim().to_string();
        let topic = std::env::var("APNS_TOPIC").ok()?.trim().to_string();
        if team_id.is_empty() || key_id.is_empty() || topic.is_empty() {
            return None;
        }
        let p8_pem = std::fs::read_to_string(&p8_path)
            .map_err(|e| log::error!("APNs: falha ao ler {p8_path}: {e}"))
            .ok()?;
        let sandbox = std::env::var("APNS_SANDBOX")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Some(ApnsConfig {
            team_id,
            key_id,
            p8_pem,
            topic,
            host: if sandbox {
                "https://api.sandbox.push.apple.com"
            } else {
                "https://api.push.apple.com"
            },
        })
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    /// JWT ES256 exigido pelo APNs (renovado a cada 45 min).
    fn jwt(&self, c: &ApnsConfig) -> Result<String> {
        let mut cache = self.jwt_cache.lock().expect("jwt cache");
        if let Some((token, at)) = cache.as_ref() {
            if at.elapsed() < Duration::from_secs(45 * 60) {
                return Ok(token.clone());
            }
        }
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
        header.kid = Some(c.key_id.clone());
        let claims = Claims {
            iss: &c.team_id,
            iat: chrono::Utc::now().timestamp() as u64,
        };
        let key = jsonwebtoken::EncodingKey::from_ec_pem(c.p8_pem.as_bytes())
            .context("chave .p8 inválida (esperado PEM EC/ES256 da Apple)")?;
        let token = jsonwebtoken::encode(&header, &claims, &key)?;
        *cache = Some((token.clone(), Instant::now()));
        Ok(token)
    }

    /// Envia um push a um device token. Retorna Ok(true) enviado, Ok(false)
    /// token inválido/expirado (deve ser removido do registro).
    pub async fn send(&self, device_token: &str, title: &str, body: &str) -> Result<bool> {
        let Some(c) = &self.config else {
            log::info!("[DRY-RUN push] {device_token}: {title} — {body}");
            return Ok(true);
        };
        let jwt = self.jwt(c)?;
        let url = format!("{}/3/device/{}", c.host, device_token);
        let payload = serde_json::json!({
            "aps": {
                "alert": { "title": title, "body": body },
                "sound": "default",
                "interruption-level": "time-sensitive"
            }
        });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(jwt)
            .header("apns-topic", &c.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .json(&payload)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            return Ok(true);
        }
        let text = resp.text().await.unwrap_or_default();
        // 410 Gone / BadDeviceToken -> aparelho desinstalou ou token trocou.
        if status.as_u16() == 410 || text.contains("BadDeviceToken") {
            log::warn!("APNs: token inválido/expirado ({device_token}), removendo");
            return Ok(false);
        }
        Err(anyhow!("APNs {status}: {text}"))
    }
}
