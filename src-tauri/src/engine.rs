use crate::db::Database;
use crate::gemini::GeminiClient;
use crate::models::RawNewsItem;
use crate::scrapers;
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// Após esta quantidade de falhas seguidas, um item é abandonado na sessão
/// para não gerar tempestade de retry/consumo de cota a cada ciclo.
const MAX_ITEM_FAILURES: u32 = 3;

/// Falhas consecutivas de análise que disparam a rotação de API key.
const FAILURES_BEFORE_KEY_ROTATION: u32 = 3;

/// Estado global gerenciado pelo Tauri (acessível nos commands).
pub struct AppState {
    pub db: Mutex<Database>,
    pub gemini: Arc<GeminiClient>,
}

/// Loop principal do pregão:
///   scraping (com delays anti-bot) -> dedup -> Gemini -> SQLite ->
///   evento p/ UI -> notificação nativa se CRITICAL/HIGH.
///
/// Roda em background pelo tokio do próprio Tauri; a UI nunca bloqueia.
pub async fn run_loop(app: AppHandle, gemini: Arc<GeminiClient>) {
    let base_interval: u64 = std::env::var("SCRAPE_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    log::info!("Motor de ingestão iniciado (ciclo base: {base_interval}s)");

    // Diagnóstico + CALIBRAÇÃO. Primeiro lista os modelos da chave (informativo);
    // depois sonda e trava num modelo que comprovadamente responde 200. Isso
    // elimina os 404 em operação — o ponto central da robustez.
    gemini.log_available_models().await;
    match gemini.calibrate().await {
        Ok(model) => {
            log::info!("Motor ONLINE usando modelo Gemini '{model}'");
            let _ = app.emit("engine-status", "ONLINE");
        }
        Err(e) => {
            log::error!("Gemini sem modelo utilizável para esta chave: {e:#}");
            let _ = app.emit(
                "engine-error",
                format!("Nenhum modelo Gemini utilizável para esta chave: {e}"),
            );
        }
    }

    // Teto de análises por ciclo: suaviza rajadas (evita 429 no nível gratuito).
    let max_per_cycle: usize = std::env::var("MAX_ANALYSES_PER_CYCLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);

    // Contador de falhas por item (dedup_key -> nº de falhas) entre ciclos.
    let mut fail_counts: HashMap<String, u32> = HashMap::new();
    // Falhas consecutivas (qualquer item) para acionar rotação de chave.
    let mut consecutive_failures: u32 = 0;

    loop {
        let mut batch: Vec<RawNewsItem> = Vec::new();

        // Pilar 1 — conectores rodam em sequência, cada um com seu
        // polite_delay() interno (2–5s aleatórios por requisição).

        // Fonte principal: feeds RSS (robusta, sem bloqueio, sem chave).
        match scrapers::rss::fetch_all().await {
            Ok(mut v) => batch.append(&mut v),
            Err(e) => log::warn!("Feeds RSS falharam neste ciclo: {e:#}"),
        }

        // Manchetes rápidas opcionais (Truth Social via RSS, se configurado).
        match scrapers::financial_juice::fetch_headlines().await {
            Ok(mut v) => batch.append(&mut v),
            Err(e) => log::warn!("Feeds de manchetes rápidas falharam neste ciclo: {e:#}"),
        }

        // Investing.com fica atrás do Cloudflare e retorna 403 a scrapers
        // HTTP simples. Por isso vem DESLIGADO por padrão; ative apenas se
        // tiver um contorno (proxy/navegador) definindo ENABLE_INVESTING=1.
        let enable_investing = std::env::var("ENABLE_INVESTING")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if enable_investing {
            match scrapers::investing::fetch_high_impact_events().await {
                Ok(mut v) => batch.append(&mut v),
                Err(e) => log::warn!("Investing.com falhou neste ciclo: {e:#}"),
            }
        }

        let mut analyzed_this_cycle = 0usize;
        for item in batch {
            if analyzed_this_cycle >= max_per_cycle {
                break; // o restante é reprocessado no próximo ciclo (via dedup)
            }

            let key = item.dedup_key();

            // Dedup ANTES do Gemini: cada chamada de LLM custa dinheiro/latência.
            let is_new = {
                let state = app.state::<AppState>();
                let db = state.db.lock().expect("db mutex envenenado");
                db.is_new(&key)
            };
            match is_new {
                Ok(false) => continue,
                Err(e) => {
                    log::error!("Falha ao consultar dedup: {e:#}");
                    continue;
                }
                Ok(true) => {}
            }

            // Abandona itens que já falharam demais (evita retry infinito).
            if fail_counts.get(&key).copied().unwrap_or(0) >= MAX_ITEM_FAILURES {
                continue;
            }

            analyzed_this_cycle += 1;
            if analyze_and_store(&app, &gemini, &item, &key).await {
                fail_counts.remove(&key);
                consecutive_failures = 0;
            } else {
                *fail_counts.entry(key).or_insert(0) += 1;
                consecutive_failures += 1;

                // Falhas seguidas podem indicar cota esgotada -> tenta a próxima
                // API key do pool e recalibra (nova chave pode ter outros modelos).
                if consecutive_failures >= FAILURES_BEFORE_KEY_ROTATION && gemini.rotate_key() {
                    consecutive_failures = 0;
                    log::warn!(
                        "Gemini: falhas seguidas — rotacionando para a API key #{}",
                        gemini.active_key_index() + 1
                    );
                    match gemini.calibrate().await {
                        Ok(model) => {
                            log::info!("Gemini: recalibrado na nova chave -> '{model}'");
                            let _ = app.emit("engine-status", "ONLINE");
                        }
                        Err(e) => {
                            log::error!("Gemini: nova chave sem modelo utilizável: {e:#}")
                        }
                    }
                }
            }
        }

        // Jitter no intervalo do ciclo: padrão de tráfego não-robótico
        let jitter = rand::thread_rng().gen_range(0..=10);
        tokio::time::sleep(Duration::from_secs(base_interval + jitter)).await;
    }
}

/// Analisa um item novo no Gemini, persiste e transmite à UI.
/// Retorna `true` em sucesso, `false` em qualquer falha (para o contador).
async fn analyze_and_store(
    app: &AppHandle,
    gemini: &GeminiClient,
    item: &RawNewsItem,
    key: &str,
) -> bool {
    // Pilar 2 — análise estruturada no Gemini
    let analysis = match gemini.analyze(item).await {
        Ok(a) => a,
        Err(e) => {
            log::error!("Gemini falhou para '{}': {e:#}", item.headline);
            return false;
        }
    };

    // Persistência + broadcast para o dashboard
    let event = {
        let state = app.state::<AppState>();
        let db = state.db.lock().expect("db mutex envenenado");
        match db.insert(key, &analysis) {
            Ok(ev) => ev,
            Err(e) => {
                log::error!("Falha ao persistir evento: {e:#}");
                return false;
            }
        }
    };

    log::info!(
        "[{}] {} | {} | ES {}",
        event.analysis.impact_level,
        event.analysis.event,
        event.analysis.sentiment,
        event.analysis.projected_target_pts
    );
    let _ = app.emit("news-event", &event);

    // Pilar 3 — alerta nativo do Windows para CRITICAL / HIGH
    if event.analysis.requires_native_alert() {
        fire_native_alert(app, &event.analysis);
    }
    true
}

fn fire_native_alert(app: &AppHandle, a: &crate::models::GeminiAnalysis) {
    let title = format!("⚠ {} — {}", a.impact_level, a.event);
    let body = format!(
        "{} | ES: {} (↑{}% ↓{}%)\n{}",
        a.sentiment,
        a.projected_target_pts,
        a.sp500_direction_probability.up,
        a.sp500_direction_probability.down,
        a.rationale
    );

    // O som do alerta é disparado pelo frontend (Web Audio) ao receber o
    // evento "news-event" com impact_level CRITICAL/HIGH — ver useNewsStream.ts.
    if let Err(e) = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
    {
        log::error!("Falha ao exibir notificação nativa: {e:#}");
    }
}
