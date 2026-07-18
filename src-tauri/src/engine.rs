use crate::db::Database;
use crate::gemini::GeminiClient;
use crate::models::RawNewsItem;
use crate::{prices, scrapers};
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
    /// Caminho do settings.json (persistência de chaves + ativo).
    pub settings_path: std::path::PathBuf,
    /// Feeds RSS adicionados pelo usuário (URLs), ajustáveis em runtime.
    pub extra_feeds: Mutex<Vec<String>>,
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

    // Teto de análises por ciclo: suaviza rajadas (evita 429 no nível gratuito).
    let max_per_cycle: usize = std::env::var("MAX_ANALYSES_PER_CYCLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);

    // Contador de falhas por item (dedup_key -> nº de falhas) entre ciclos.
    let mut fail_counts: HashMap<String, u32> = HashMap::new();
    // Falhas consecutivas (qualquer item) para acionar rotação de chave.
    let mut consecutive_failures: u32 = 0;
    // Calibra sob demanda: cobre "sem chave no startup" e "chave adicionada
    // pela UI depois" — enquanto offline, tenta calibrar a cada ciclo.
    let mut online = false;

    loop {
        // CALIBRAÇÃO (sonda e trava num modelo que responde 200). Só quando há
        // ao menos uma chave; senão sinaliza à UI e aguarda a chave.
        if !online {
            if gemini.has_keys() {
                gemini.log_available_models().await;
                match gemini.calibrate().await {
                    Ok(model) => {
                        online = true;
                        log::info!("Motor ONLINE usando modelo Gemini '{model}'");
                        let _ = app.emit("engine-status", "ONLINE");
                    }
                    Err(e) => {
                        log::error!("Gemini sem modelo utilizável: {e:#}");
                        let _ = app.emit("engine-error", format!("{e}"));
                        tokio::time::sleep(Duration::from_secs(base_interval)).await;
                        continue;
                    }
                }
            } else {
                let _ = app.emit(
                    "engine-error",
                    "Configure a GEMINI_API_KEY (.env ou campo do dashboard).".to_string(),
                );
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        }

        let mut batch: Vec<RawNewsItem> = Vec::new();

        // Pilar 1 — conectores rodam em sequência, cada um com seu
        // polite_delay() interno (2–5s aleatórios por requisição).

        // Fonte principal: feeds RSS (padrão + os que o usuário adicionou).
        let extra_feeds = {
            let state = app.state::<AppState>();
            let f = state.extra_feeds.lock().expect("extra_feeds mutex");
            f.clone()
        };
        match scrapers::rss::fetch_all(&extra_feeds).await {
            Ok(mut v) => batch.append(&mut v),
            Err(e) => log::warn!("Feeds RSS falharam neste ciclo: {e:#}"),
        }

        // Truth Social: espelho do Trump (ligado) + conector logado (opcional).
        match scrapers::truth_social::fetch_all().await {
            Ok(mut v) => batch.append(&mut v),
            Err(e) => log::warn!("Truth Social falhou neste ciclo: {e:#}"),
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

        // Preço-base do ativo ativo neste ciclo (referência p/ pontuar as
        // predições depois). Se o Yahoo falhar, fica None e não pontuamos.
        let baseline_price = current_asset_price(&gemini).await;

        let mut analyzed_this_cycle = 0usize;
        for item in batch {
            if analyzed_this_cycle >= max_per_cycle {
                break; // o restante é reprocessado no próximo ciclo (via dedup)
            }

            // Chave de dedup ESCOPADA PELO ATIVO ativo: trocar de ativo faz as
            // notícias serem re-analisadas sob a ótica do novo ativo.
            let asset = gemini.asset();
            let key = item.dedup_key_for(&asset);

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
            if analyze_and_store(&app, &gemini, &item, &key, &asset, baseline_price).await {
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
                            // Deixa o topo do loop re-tentar a calibração.
                            online = false;
                            log::error!("Gemini: nova chave sem modelo utilizável: {e:#}");
                        }
                    }
                }
            }
        }

        // Autoaprendizagem: pontua predições antigas contra o preço real e
        // atualiza o texto de aprendizado injetado no prompt.
        run_scoring(&app, &gemini).await;

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
    asset: &str,
    baseline_price: Option<f64>,
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
        match db.insert(key, asset, baseline_price, &analysis) {
            Ok(ev) => ev,
            Err(e) => {
                log::error!("Falha ao persistir evento: {e:#}");
                return false;
            }
        }
    };

    log::info!(
        "[{}] {} | {} | {} {}",
        event.analysis.impact_level,
        event.analysis.event,
        event.analysis.sentiment,
        asset,
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

// ---------------------------------------------------------------------------
// Autoaprendizagem: preço real, pontuação das predições e aprendizado
// ---------------------------------------------------------------------------

/// Preço atual do ativo prioritário (via Yahoo). None se não houver símbolo
/// mapeado ou se a cotação falhar.
async fn current_asset_price(gemini: &GeminiClient) -> Option<f64> {
    let symbol = prices::yahoo_symbol_for(&gemini.asset())?;
    match prices::fetch_last_price(symbol).await {
        Ok(p) => Some(p),
        Err(e) => {
            log::warn!("Cotação de referência indisponível ({symbol}): {e:#}");
            None
        }
    }
}

/// Pontua as predições cujo horizonte já passou, comparando o movimento real
/// (preço agora vs. preço-base) com a direção prevista; depois recalcula o
/// texto de aprendizado e o injeta no Gemini.
async fn run_scoring(app: &AppHandle, gemini: &GeminiClient) {
    let horizon: i64 = std::env::var("SCORE_HORIZON_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1200); // 20 min (cobre o atraso da cotação)
    let max_defer: i64 = 3 * 3600; // após 3h sem movimento medível, desiste

    let pending = {
        let state = app.state::<AppState>();
        let db = state.db.lock().expect("db mutex");
        db.pending_scores(horizon).unwrap_or_default()
    };
    if pending.is_empty() {
        return;
    }

    // Cache de preço por símbolo (uma requisição por símbolo neste ciclo).
    let mut price_cache: HashMap<&str, Option<f64>> = HashMap::new();
    for p in &pending {
        let symbol = match prices::yahoo_symbol_for(&p.asset) {
            Some(s) => s,
            None => {
                let state = app.state::<AppState>();
                let db = state.db.lock().expect("db mutex");
                let _ = db.mark_unscorable(p.id);
                continue;
            }
        };
        let price = match price_cache.get(symbol) {
            Some(v) => *v,
            None => {
                let v = prices::fetch_last_price(symbol).await.ok();
                price_cache.insert(symbol, v);
                v
            }
        };
        let Some(price) = price else { continue }; // cotação falhou: tenta depois

        let realized = price - p.baseline_price;
        let predicted = crate::db::parse_pts(&p.projected_target_pts);

        let state = app.state::<AppState>();
        let db = state.db.lock().expect("db mutex");
        if predicted.abs() < f64::EPSILON {
            // Predição neutra: registra o movimento, sem acerto de direção.
            let _ = db.record_score(p.id, Some(realized), None);
        } else if realized.abs() < 0.01 {
            // Sem movimento medível (mercado fechado?). Adia; desiste após max.
            if event_age_secs(&p.received_at_utc) > max_defer {
                let _ = db.mark_unscorable(p.id);
            }
        } else {
            let hit = predicted.signum() == realized.signum();
            let _ = db.record_score(p.id, Some(realized), Some(hit));
        }
    }

    // Recalcula o aprendizado e injeta no prompt.
    let stats = {
        let state = app.state::<AppState>();
        let db = state.db.lock().expect("db mutex");
        db.accuracy_stats().ok()
    };
    if let Some(stats) = stats {
        let text = build_learning(&stats);
        gemini.set_learning(&text);
        let _ = app.emit("accuracy-updated", &stats);
    }
}

fn event_age_secs(received_at_utc: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(received_at_utc)
        .map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds())
        .unwrap_or(0)
}

/// Monta o texto de aprendizado (em PT) a partir das estatísticas de acerto.
fn build_learning(stats: &crate::db::AccuracyStats) -> String {
    if stats.scored == 0 {
        return String::new();
    }
    let mut s = format!(
        "- Direção: {}% de acerto em {} predições pontuadas.",
        stats.hit_rate, stats.scored
    );
    let mag = stats.magnitude_factor;
    if mag > 1.15 {
        s += &format!(
            " Você SUBESTIMA a magnitude (~{mag:.2}x o real); aumente proporcionalmente os pontos projetados."
        );
    } else if mag < 0.85 {
        s += &format!(
            " Você SUPERESTIMA a magnitude (~{mag:.2}x o real); reduza proporcionalmente os pontos projetados."
        );
    } else {
        s += " Magnitude bem calibrada.";
    }
    let by: Vec<String> = stats
        .by_impact
        .iter()
        .filter(|i| i.total > 0)
        .map(|i| format!("{} {}/{}", i.impact_level, i.hits, i.total))
        .collect();
    if !by.is_empty() {
        s += &format!(" Acerto por impacto: {}.", by.join(", "));
    }
    s
}
