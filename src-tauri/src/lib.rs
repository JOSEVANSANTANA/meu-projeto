mod db;
mod engine;
mod gemini;
mod instruments;
mod models;
mod prices;
mod scrapers;

use db::Database;
use engine::AppState;
use gemini::GeminiClient;
use instruments::Resolved;
use models::NewsEvent;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Estado de configuração enviado à UI (topo do dashboard).
#[derive(Serialize)]
struct RuntimeStatus {
    keys_count: usize,
    active_key: usize,
    asset: String,
}

/// Command: hidrata o dashboard com o histórico local ao abrir o app.
#[tauri::command]
fn get_recent_events(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<NewsEvent>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.recent(limit.unwrap_or(100)).map_err(|e| e.to_string())
}

/// Command: adiciona uma nova API key do Gemini ao pool (para quando a atual
/// esgota a cota). Retorna o total de chaves cadastradas.
#[tauri::command]
fn add_api_key(state: tauri::State<'_, AppState>, key: String) -> Result<usize, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Chave vazia".to_string());
    }
    Ok(state.gemini.add_key(&key))
}

/// Command: define o ativo/índice prioritário das análises (ES, NQ, etc.).
/// Resolve por CÓDIGO (mesu6, es…) ou NOME (micro nasdaq…) para o instrumento
/// real e devolve o que foi reconhecido — assim a UI confirma o vínculo.
#[tauri::command]
fn set_priority_asset(
    state: tauri::State<'_, AppState>,
    asset: String,
) -> Result<Resolved, String> {
    if asset.trim().is_empty() {
        return Err("Ativo vazio".to_string());
    }
    let resolved = instruments::resolve(&asset);
    // Grava o nome canônico (reconhecido) ou o texto cru (não reconhecido).
    state.gemini.set_asset(&resolved.name);
    Ok(resolved)
}

/// Command: placar de acerto da IA (autoaprendizagem).
#[tauri::command]
fn get_accuracy(state: tauri::State<'_, AppState>) -> Result<db::AccuracyStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.accuracy_stats().map_err(|e| e.to_string())
}

/// Command: estado de configuração para a UI (nº de chaves, chave ativa, ativo).
#[tauri::command]
fn get_runtime_status(state: tauri::State<'_, AppState>) -> RuntimeStatus {
    RuntimeStatus {
        keys_count: state.gemini.keys_len(),
        active_key: state.gemini.active_key_index(),
        asset: state.gemini.asset(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Carrega .env de vários locais para funcionar tanto em `tauri dev` (raiz do
    // projeto) quanto no app INSTALADO (ao lado do .exe). O primeiro a definir
    // uma variável vence; os demais só preenchem o que faltar.
    dotenvy::dotenv().ok(); // diretório atual (dev)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = dotenvy::from_path(dir.join(".env")); // ao lado do executável
        }
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // SQLite + .env também no diretório de dados do app (%APPDATA%).
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let _ = dotenvy::from_path(data_dir.join(".env"));
            let db = Database::open(&data_dir.join("esf_news.db"))?;

            // Cliente Gemini compartilhado. Tolera ausência de chave no startup:
            // o app abre, mostra o erro e você pode colar a chave no dashboard.
            let gemini = Arc::new(GeminiClient::from_env().map_err(|e| {
                log::error!("Falha ao iniciar cliente Gemini: {e:#}");
                e
            })?);

            // Normaliza o ativo padrão para o nome canônico do instrumento.
            let resolved = instruments::resolve(&gemini.asset());
            gemini.set_asset(&resolved.name);
            log::info!("Ativo prioritário inicial: {}", resolved.name);

            app.manage(AppState {
                db: Mutex::new(db),
                gemini: gemini.clone(),
            });

            // Loop de ingestão em background — vive enquanto o app viver
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(engine::run_loop(handle, gemini));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_recent_events,
            add_api_key,
            set_priority_asset,
            get_runtime_status,
            get_accuracy
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a aplicação Tauri");
}
