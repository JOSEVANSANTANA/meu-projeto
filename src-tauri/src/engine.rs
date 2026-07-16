use crate::db::Database;
use crate::gemini::GeminiClient;
use crate::models::RawNewsItem;
use crate::scrapers;
use rand::Rng;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// Estado global gerenciado pelo Tauri (acessível nos commands).
pub struct AppState {
    pub db: Mutex<Database>,
}

/// Loop principal do pregão:
///   scraping (com delays anti-bot) -> dedup -> Gemini -> SQLite ->
///   evento p/ UI -> notificação nativa se CRITICAL/HIGH.
///
/// Roda em background pelo tokio do próprio Tauri; a UI nunca bloqueia.
pub async fn run_loop(app: AppHandle) {
    let gemini = match GeminiClient::from_env() {
        Ok(g) => g,
        Err(e) => {
            log::error!("Motor desligado — {e:#}");
            let _ = app.emit("engine-error", format!("{e:#}"));
            return;
        }
    };

    let base_interval: u64 = std::env::var("SCRAPE_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    log::info!("Motor de ingestão iniciado (ciclo base: {base_interval}s)");

    // Diagnóstico: lista os modelos que a chave realmente pode usar. Se der
    // 404 nas análises, é aqui que se confere o nome correto do modelo.
    gemini.log_available_models().await;

    let _ = app.emit("engine-status", "ONLINE");

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

        for item in batch {
            process_item(&app, &gemini, item).await;
        }

        // Jitter no intervalo do ciclo: padrão de tráfego não-robótico
        let jitter = rand::thread_rng().gen_range(0..=10);
        tokio::time::sleep(Duration::from_secs(base_interval + jitter)).await;
    }
}

async fn process_item(app: &AppHandle, gemini: &GeminiClient, item: RawNewsItem) {
    let key = item.dedup_key();

    // Dedup ANTES do Gemini: cada chamada de LLM custa dinheiro e latência
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().expect("db mutex envenenado");
        match db.is_new(&key) {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                log::error!("Falha ao consultar dedup: {e:#}");
                return;
            }
        }
    } // lock liberado antes do await (Mutex std não atravessa await)

    // Pilar 2 — análise estruturada no Gemini
    let analysis = match gemini.analyze(&item).await {
        Ok(a) => a,
        Err(e) => {
            log::error!("Gemini falhou para '{}': {e:#}", item.headline);
            return;
        }
    };

    // Persistência + broadcast para o dashboard
    let event = {
        let state = app.state::<AppState>();
        let db = state.db.lock().expect("db mutex envenenado");
        match db.insert(&key, &analysis) {
            Ok(ev) => ev,
            Err(e) => {
                log::error!("Falha ao persistir evento: {e:#}");
                return;
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
