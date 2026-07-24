use crate::models::RawNewsItem;
use crate::scrapers::rss;
use anyhow::Result;
#[cfg(desktop)]
use anyhow::anyhow;
#[cfg(desktop)]
use serde::Deserialize;
#[cfg(desktop)]
use std::time::Duration;

/// Ingestão do Truth Social em DUAS camadas:
///   1. Espelho público do Trump (RSS aberto) — confiável, sem login, LIGADO.
///   2. Conector LOGADO opcional (home timeline de quem você segue) via
///      subprocesso Python — DESLIGADO até você configurar credenciais.
pub async fn fetch_all() -> Result<Vec<RawNewsItem>> {
    let mut items = Vec::new();

    match fetch_trump_mirror().await {
        Ok(mut v) => items.append(&mut v),
        Err(e) => log::warn!("Truth Social (espelho do Trump) indisponível: {e:#}"),
    }

    match fetch_following().await {
        Ok(mut v) => items.append(&mut v),
        Err(e) => log::warn!("Truth Social (conector logado) falhou: {e:#}"),
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// Camada 1 — espelho público do Trump (RSS)
// ---------------------------------------------------------------------------

fn mirror_url() -> String {
    std::env::var("TRUMP_MIRROR_RSS_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://trumpstruth.org/feed".to_string())
}

async fn fetch_trump_mirror() -> Result<Vec<RawNewsItem>> {
    let filter_on = std::env::var("TRUMP_FILTER")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true);

    let now = chrono::Utc::now().to_rfc3339();
    // fetch_best_text (não fetch_titles): ~40% dos posts do Trump são reposts
    // sem legenda própria, cujo <title> vem como "[No Title] - Post from ...".
    // Sem o fallback para <description>, esses posts eram todos descartados.
    let items = rss::fetch_best_text(&mirror_url())
        .await?
        .into_iter()
        .filter(|h| !h.is_empty() && (!filter_on || is_market_relevant_post(h)))
        .take(20)
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

    log::info!("Truth Social (Trump): {} post(s) de interesse", items.len());
    Ok(items)
}

/// Filtro LENIENTE para posts políticos: mais amplo que o macro dos feeds
/// econômicos, pois uma fala do Trump pode mover o mercado por vários ângulos.
fn is_market_relevant_post(text: &str) -> bool {
    const KW: &[&str] = &[
        "tariff", "tariffs", "trade", "china", "chinese", "tax", "taxes", "fed",
        "federal reserve", "powell", "rate", "rates", "interest", "inflation",
        "oil", "energy", "gas", "opec", "dollar", "economy", "economic", "market",
        "markets", "stock", "stocks", "jobs", "deal", "sanction", "sanctions",
        "russia", "ukraine", "iran", "israel", "war", "border", "immigration",
        "election", "crypto", "bitcoin", "nasdaq", "dow", "s&p", "recession",
        "tax cut", "spending", "debt", "treasury", "semiconductor", "chip",
    ];
    let t = text.to_lowercase();
    KW.iter().any(|k| t.contains(k))
}

// ---------------------------------------------------------------------------
// Camada 2 — conector logado (home timeline) via subprocesso Python
// ---------------------------------------------------------------------------

#[cfg(desktop)]
#[derive(Deserialize)]
struct PyPost {
    source: String,
    headline: String,
    #[serde(default)]
    created_at: Option<String>,
}

/// No iOS/Android não há como spawnar o subprocesso Python (sandbox). O
/// conector LOGADO fica indisponível no mobile; o espelho do Trump (RSS)
/// segue funcionando normalmente.
#[cfg(mobile)]
async fn fetch_following() -> Result<Vec<RawNewsItem>> {
    Ok(Vec::new())
}

/// Só roda se houver credenciais configuradas (TRUTHSOCIAL_TOKEN OU
/// TRUTHSOCIAL_USERNAME+PASSWORD). Caso contrário, fica inerte (Ok vazio).
#[cfg(desktop)]
async fn fetch_following() -> Result<Vec<RawNewsItem>> {
    let has_token = std::env::var("TRUTHSOCIAL_TOKEN")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let has_login = std::env::var("TRUTHSOCIAL_USERNAME")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        && std::env::var("TRUTHSOCIAL_PASSWORD")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    if !has_token && !has_login {
        return Ok(Vec::new()); // conector desligado
    }

    let python = std::env::var("TRUTH_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script =
        std::env::var("TRUTH_CONNECTOR_PATH").unwrap_or_else(|_| "truth_connector.py".to_string());

    // O subprocesso herda as variáveis de ambiente (credenciais) do processo.
    let run = tokio::process::Command::new(&python)
        .arg(&script)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output();

    let out = match tokio::time::timeout(Duration::from_secs(40), run).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(anyhow!("falha ao executar '{python} {script}': {e}")),
        Err(_) => return Err(anyhow!("timeout no conector Python (40s)")),
    };

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("conector Python retornou erro: {}", err.trim()));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let posts: Vec<PyPost> = serde_json::from_str(stdout.trim())
        .map_err(|e| anyhow!("JSON inválido do conector Python: {e} — saída: {stdout}"))?;

    let items = posts
        .into_iter()
        .filter(|p| !p.headline.trim().is_empty())
        .map(|p| RawNewsItem {
            source: if p.source.is_empty() {
                "Truth Social".to_string()
            } else {
                p.source
            },
            headline: p.headline,
            actual: None,
            forecast: None,
            previous: None,
            impact_hint: "headline".to_string(),
            timestamp_utc: p
                .created_at
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        })
        .collect::<Vec<_>>();

    log::info!("Truth Social (logado): {} post(s) de quem você segue", items.len());
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtro_mantem_posts_de_mercado_e_descarta_ruido() {
        assert!(is_market_relevant_post("New TARIFFS on China are coming!"));
        assert!(is_market_relevant_post("The Fed must CUT rates immediately"));
        assert!(is_market_relevant_post("Big deal on OIL with Saudi Arabia"));
        assert!(!is_market_relevant_post("Happy Birthday to a great American patriot"));
        assert!(!is_market_relevant_post("Rally tonight, see you all there!"));
    }

    #[test]
    fn parse_json_do_conector_python() {
        let json = r#"[
            {"source":"Truth Social (@realDonaldTrump)","headline":"Tariffs on China!","created_at":"2026-07-16T00:00:00Z"},
            {"source":"","headline":"Fed should cut"}
        ]"#;
        let posts: Vec<PyPost> = serde_json::from_str(json).unwrap();
        assert_eq!(posts.len(), 2);
        assert_eq!(posts[0].headline, "Tariffs on China!");
        assert!(posts[1].created_at.is_none());
    }

    #[tokio::test]
    async fn subprocesso_python_parseia_saida_mock() {
        // Script Python mock (não usa curl_cffi/rede): valida spawn + parse.
        let script = std::env::temp_dir().join("mock_truth_connector.py");
        std::fs::write(
            &script,
            "print('[{\"source\":\"Truth Social (@x)\",\"headline\":\"China tariffs incoming\",\"created_at\":\"2026-01-01T00:00:00Z\"}]')",
        )
        .unwrap();

        std::env::set_var("TRUTHSOCIAL_TOKEN", "dummy");
        std::env::set_var("TRUTH_CONNECTOR_PATH", script.to_str().unwrap());
        std::env::set_var("TRUTH_PYTHON", "python3");

        let items = fetch_following().await.expect("subprocesso deve funcionar");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "Truth Social (@x)");
        assert_eq!(items[0].headline, "China tariffs incoming");

        std::env::remove_var("TRUTHSOCIAL_TOKEN");
        std::env::remove_var("TRUTH_CONNECTOR_PATH");
        std::env::remove_var("TRUTH_PYTHON");
    }
}
