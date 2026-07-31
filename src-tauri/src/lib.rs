mod db;
mod engine;
mod gemini;
mod instruments;
mod models;
mod prices;
mod scrapers;
mod settings;

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

/// Resultado da adição de um feed RSS (com validação/autodescoberta).
#[derive(Serialize)]
struct AddFeedResult {
    url: String,
    count: usize,
}

/// Persiste chaves + ativo + feeds RSS atuais no settings.json (best-effort).
fn persist_settings(state: &AppState) {
    let s = settings::Settings {
        api_keys: state.gemini.keys(),
        asset: Some(state.gemini.asset()),
        rss_feeds: state
            .extra_feeds
            .lock()
            .map(|f| f.clone())
            .unwrap_or_default(),
    };
    settings::save(&state.settings_path, &s);
}

/// Command: adiciona uma fonte RSS pela URL. Valida e AUTODESCOBRE o feed
/// (aceita colar o site, ex.: https://finance.yahoo.com/). Persiste em disco.
#[tauri::command]
async fn add_rss_feed(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<AddFeedResult, String> {
    let url = url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("URL inválida — comece com http:// ou https://".to_string());
    }
    let (feed_url, count) = scrapers::rss::probe_feed(&url).await.map_err(|e| e.to_string())?;
    {
        let mut feeds = state.extra_feeds.lock().map_err(|e| e.to_string())?;
        if !feeds.contains(&feed_url) {
            feeds.push(feed_url.clone());
        }
    }
    persist_settings(&state);
    Ok(AddFeedResult { url: feed_url, count })
}

/// Command: lista as fontes RSS adicionadas pelo usuário.
#[tauri::command]
fn get_rss_feeds(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.extra_feeds.lock().map(|f| f.clone()).unwrap_or_default()
}

/// Command: remove uma fonte RSS.
#[tauri::command]
fn remove_rss_feed(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Vec<String>, String> {
    let list = {
        let mut feeds = state.extra_feeds.lock().map_err(|e| e.to_string())?;
        feeds.retain(|u| u != &url);
        feeds.clone()
    };
    persist_settings(&state);
    Ok(list)
}

/// Command: adiciona uma nova API key do Gemini ao pool (para quando a atual
/// esgota a cota) e PERSISTE em disco. Retorna o total de chaves cadastradas.
#[tauri::command]
fn add_api_key(state: tauri::State<'_, AppState>, key: String) -> Result<usize, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Chave vazia".to_string());
    }
    let total = state.gemini.add_key(&key);
    persist_settings(&state);
    Ok(total)
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
    persist_settings(&state);
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

    // Notificações são disparadas via notify-rust DIRETAMENTE em engine.rs (não
    // via plugin) — ver o comentário em fire_native_alert() para o motivo.
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();
    // Auto-update é só desktop (no iOS a atualização vem pela App Store).
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
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

            // PERSISTÊNCIA: carrega chaves + ativo salvos e aplica por cima do
            // que veio do .env (assim a chave informada uma vez sobrevive a
            // reinícios, sem precisar redigitar).
            let settings_path = data_dir.join("settings.json");
            let saved = settings::load(&settings_path);
            for k in &saved.api_keys {
                gemini.add_key(k);
            }
            if let Some(a) = saved.asset.as_ref().filter(|a| !a.trim().is_empty()) {
                gemini.set_asset(&instruments::resolve(a).name);
            }
            log::info!(
                "Ativo prioritário: {} | chaves carregadas: {} | feeds RSS extras: {}",
                gemini.asset(),
                gemini.keys_len(),
                saved.rss_feeds.len()
            );

            app.manage(AppState {
                db: Mutex::new(db),
                gemini: gemini.clone(),
                settings_path,
                extra_feeds: Mutex::new(saved.rss_feeds),
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
            get_accuracy,
            add_rss_feed,
            get_rss_feeds,
            remove_rss_feed
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a aplicação Tauri");
}
