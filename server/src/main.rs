//! Servidor 24/7 do ESF News Monitor.
//!
//! Reaproveita o MESMO motor do app desktop (scrapers RSS/Truth Social,
//! análise Gemini com trava anti-alucinação, SQLite) e expõe:
//!   GET  /health            -> "ok" (monitoramento)
//!   GET  /events?limit=100  -> últimos eventos analisados (JSON)
//!   POST /devices {token}   -> registra um iPhone para receber push APNs
//!   GET  /devices/count     -> nº de aparelhos registrados
//!
//! Push: eventos CRITICAL/HIGH disparam APNs para todos os aparelhos
//! registrados (DRY-RUN até configurar as credenciais da Apple — ver push.rs).

// ---- Módulos COMPARTILHADOS com o app (mesmos arquivos, zero duplicação) ----
// allow(dead_code): o servidor não usa TODAS as funções do código compartilhado
// (ex.: helpers exclusivos da UI desktop) — e isso é esperado.
#[path = "../../src-tauri/src/models.rs"]
#[allow(dead_code)]
mod models;
#[path = "../../src-tauri/src/db.rs"]
#[allow(dead_code)]
mod db;
#[path = "../../src-tauri/src/gemini.rs"]
#[allow(dead_code)]
mod gemini;
#[path = "../../src-tauri/src/instruments.rs"]
#[allow(dead_code)]
mod instruments;
#[path = "../../src-tauri/src/scrapers/mod.rs"]
#[allow(dead_code)]
mod scrapers;

mod push;

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use db::Database;
use gemini::GeminiClient;
use models::RawNewsItem;
use push::Pusher;
use rand::Rng;
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_ITEM_FAILURES: u32 = 3;

struct Ctx {
    db: Mutex<Database>,
    devices: Mutex<Connection>,
    gemini: Arc<GeminiClient>,
    pusher: Pusher,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
    std::fs::create_dir_all(&data_dir)?;

    let db = Database::open(std::path::Path::new(&format!("{data_dir}/esf_news.db")))?;
    let devices = open_devices_store(&format!("{data_dir}/devices.db"))?;
    let gemini = Arc::new(GeminiClient::from_env()?);

    // Normaliza o ativo do servidor (PRIORITY_ASSET) para o nome canônico.
    let resolved = instruments::resolve(&gemini.asset());
    gemini.set_asset(&resolved.name);
    log::info!("Servidor analisando para: {}", resolved.name);

    let ctx = Arc::new(Ctx {
        db: Mutex::new(db),
        devices: Mutex::new(devices),
        gemini: gemini.clone(),
        pusher: Pusher::from_env(),
    });

    // Loop de ingestão em background.
    tokio::spawn(run_loop(ctx.clone()));

    // API HTTP.
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/events", get(get_events))
        .route("/devices", post(register_device))
        .route("/devices/count", get(devices_count))
        .with_state(ctx);

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("API ouvindo em http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Registro de aparelhos (device tokens do APNs)
// ---------------------------------------------------------------------------

fn open_devices_store(path: &str) -> anyhow::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS devices (
            token      TEXT PRIMARY KEY,
            created_at TEXT NOT NULL
        );",
    )?;
    Ok(conn)
}

#[derive(Deserialize)]
struct RegisterBody {
    token: String,
}

async fn register_device(
    State(ctx): State<Arc<Ctx>>,
    Json(body): Json<RegisterBody>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let token = body.token.trim().to_string();
    // Device tokens APNs são hex (64+ chars). Validação leve anti-lixo.
    if token.len() < 32 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    let count = {
        let conn = ctx.devices.lock().expect("devices lock");
        conn.execute(
            "INSERT OR IGNORE INTO devices (token, created_at) VALUES (?1, ?2)",
            rusqlite::params![token, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        device_count(&conn)
    };
    log::info!("Aparelho registrado ({count} no total)");
    Ok(Json(serde_json::json!({ "registered": true, "devices": count })))
}

fn device_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0)).unwrap_or(0)
}

fn all_devices(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT token FROM devices")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| r.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

async fn devices_count(State(ctx): State<Arc<Ctx>>) -> Json<serde_json::Value> {
    let n = {
        let conn = ctx.devices.lock().expect("devices lock");
        device_count(&conn)
    };
    Json(serde_json::json!({ "devices": n }))
}

// ---------------------------------------------------------------------------
// API de eventos (o app iOS lê o feed daqui)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<u32>,
}

async fn get_events(
    State(ctx): State<Arc<Ctx>>,
    Query(q): Query<EventsQuery>,
) -> Result<Json<Vec<models::NewsEvent>>, axum::http::StatusCode> {
    let db = ctx.db.lock().map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    db.recent(q.limit.unwrap_or(100).min(500))
        .map(Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

// ---------------------------------------------------------------------------
// Loop de ingestão (mesma lógica do app: scrape -> dedup -> Gemini -> push)
// ---------------------------------------------------------------------------

async fn run_loop(ctx: Arc<Ctx>) {
    let base_interval: u64 = std::env::var("SCRAPE_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let max_per_cycle: usize = std::env::var("MAX_ANALYSES_PER_CYCLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);

    ctx.gemini.log_available_models().await;
    match ctx.gemini.calibrate().await {
        Ok(m) => log::info!("Motor ONLINE usando modelo Gemini '{m}'"),
        Err(e) => log::error!("Gemini sem modelo utilizável agora: {e:#} (re-tentando por ciclo)"),
    }

    let mut fail_counts: HashMap<String, u32> = HashMap::new();

    loop {
        let mut batch: Vec<RawNewsItem> = Vec::new();
        match scrapers::rss::fetch_all(&[]).await {
            Ok(mut v) => batch.append(&mut v),
            Err(e) => log::warn!("Feeds RSS falharam neste ciclo: {e:#}"),
        }
        match scrapers::truth_social::fetch_all().await {
            Ok(mut v) => batch.append(&mut v),
            Err(e) => log::warn!("Truth Social falhou neste ciclo: {e:#}"),
        }

        let mut analyzed = 0usize;
        for item in batch {
            if analyzed >= max_per_cycle {
                break;
            }
            let asset = ctx.gemini.asset();
            let key = item.dedup_key_for(&asset);

            let is_new = {
                let db = ctx.db.lock().expect("db lock");
                db.is_new(&key)
            };
            if !matches!(is_new, Ok(true)) {
                continue;
            }
            if fail_counts.get(&key).copied().unwrap_or(0) >= MAX_ITEM_FAILURES {
                continue;
            }

            analyzed += 1;
            match ctx.gemini.analyze(&item).await {
                Ok(analysis) => {
                    fail_counts.remove(&key);
                    let event = {
                        let db = ctx.db.lock().expect("db lock");
                        db.insert(&key, &asset, None, &analysis)
                    };
                    match event {
                        Ok(ev) => {
                            log::info!(
                                "[{}] {} | {} | {}",
                                ev.analysis.impact_level,
                                ev.analysis.event,
                                ev.analysis.sentiment,
                                ev.analysis.projected_target_pts
                            );
                            if ev.analysis.requires_native_alert() {
                                notify_all(&ctx, &ev.analysis).await;
                            }
                        }
                        Err(e) => log::error!("Falha ao persistir: {e:#}"),
                    }
                }
                Err(e) => {
                    log::error!("Gemini falhou para '{}': {e:#}", item.headline);
                    *fail_counts.entry(key).or_insert(0) += 1;
                }
            }
        }

        let jitter = rand::thread_rng().gen_range(0..=10);
        tokio::time::sleep(Duration::from_secs(base_interval + jitter)).await;
    }
}

/// Envia push APNs a todos os aparelhos registrados; remove tokens inválidos.
async fn notify_all(ctx: &Ctx, a: &models::GeminiAnalysis) {
    let tokens = {
        let conn = ctx.devices.lock().expect("devices lock");
        all_devices(&conn)
    };
    if tokens.is_empty() && ctx.pusher.is_configured() {
        return;
    }
    let title = format!("⚠ {} — {}", a.impact_level, a.event);
    let body = format!(
        "{} | {} (↑{}% ↓{}%)",
        a.sentiment,
        a.projected_target_pts,
        a.sp500_direction_probability.up,
        a.sp500_direction_probability.down
    );
    for token in tokens {
        match ctx.pusher.send(&token, &title, &body).await {
            Ok(true) => {}
            Ok(false) => {
                let conn = ctx.devices.lock().expect("devices lock");
                let _ = conn.execute("DELETE FROM devices WHERE token = ?1", rusqlite::params![token]);
            }
            Err(e) => log::error!("Push falhou: {e:#}"),
        }
    }
    // Sem aparelhos e sem APNs configurado: loga para o operador ver o fluxo.
    if !ctx.pusher.is_configured() {
        let _ = ctx.pusher.send("nenhum-aparelho", &title, &body).await;
    }
}
