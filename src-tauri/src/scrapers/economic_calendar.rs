//! Acompanhamento em tempo real do calendário econômico (CPI, NFP, decisões
//! de juros, PIB, etc.) — o que o usuário pediu como "calendários econômicos
//! tipo Investing/TradingEconomics".
//!
//! Investing.com está bloqueado (Cloudflare, 403 confirmado em teste manual)
//! e TradingEconomics exige assinatura para a API. A fonte usada aqui é o
//! calendário público da ForexFactory (`nfs.faireconomy.media`), amplamente
//! usado por bots/EAs de trading — JSON aberto, sem chave, com previsão e
//! valor anterior REAIS por evento.
//!
//! HONESTIDADE SOBRE OS DADOS: esse feed específico NÃO preenche o valor
//! "actual" (confirmado em teste — mesmo para eventos já ocorridos). Por
//! isso este módulo NUNCA inventa um "actual": ele emite um aviso PRÉVIO
//! (com forecast/previous reais, actual="N/A") pouco antes da divulgação. O
//! valor realizado chega depois pela via normal — as manchetes da Reuters/
//! Bloomberg/CNBC/WSJ que já ingerimos, que citam o número divulgado — e são
//! analisadas com a mesma trava anti-alucinação do resto do pipeline.

use crate::models::RawNewsItem;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;

const CALENDAR_URL: &str = "https://nfs.faireconomy.media/ff_calendar_thisweek.json";

#[derive(Deserialize, Debug, Clone)]
struct CalendarEvent {
    title: String,
    country: String,
    date: DateTime<Utc>,
    impact: String,
    #[serde(default)]
    forecast: String,
    #[serde(default)]
    previous: String,
}

/// Busca o calendário e devolve avisos prévios para eventos de alto/médio
/// impacto cuja divulgação cai dentro da janela de antecedência configurada
/// (`CALENDAR_LOOKAHEAD_MINUTES`, padrão 30min). Cada evento gera UM aviso
/// (a deduplicação normal do pipeline garante que não repete a cada ciclo).
pub async fn fetch_upcoming_events() -> Result<Vec<RawNewsItem>> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let events: Vec<CalendarEvent> = client
        .get(CALENDAR_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let lookahead_minutes: i64 = std::env::var("CALENDAR_LOOKAHEAD_MINUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let now = Utc::now();
    let items = build_heads_up(&events, now, lookahead_minutes);

    log::info!(
        "Calendário econômico: {} evento(s) de alto/médio impacto na janela de {}min",
        items.len(),
        lookahead_minutes
    );
    Ok(items)
}

/// Lógica pura (testável sem rede): filtra High/Medium impact cuja divulgação
/// cai entre `now` e `now + lookahead_minutes`, e monta o RawNewsItem de
/// aviso prévio — SEM valor "actual" (ainda não divulgado, honestidade).
///
/// IMPORTANTE: o texto da manchete usa o horário FIXO da divulgação (não uma
/// contagem regressiva "faltam N min"), de propósito — a deduplicação do
/// pipeline é por texto exato (ver `dedup_key_for`), e um texto que muda a
/// cada ciclo faria o MESMO evento ser reanalisado repetidas vezes dentro da
/// janela, gerando cards duplicados. Horário fixo = mesmo texto sempre = um
/// único aviso por evento, com retry correto se a análise falhar.
fn build_heads_up(
    events: &[CalendarEvent],
    now: DateTime<Utc>,
    lookahead_minutes: i64,
) -> Vec<RawNewsItem> {
    events
        .iter()
        .filter(|e| e.impact == "High" || e.impact == "Medium")
        .filter_map(|e| {
            let minutes_until = (e.date - now).num_minutes();
            if !(0..=lookahead_minutes).contains(&minutes_until) {
                return None;
            }
            let country = country_label(&e.country);
            let headline = format!(
                "[CALENDÁRIO] {} ({}) — divulgação prevista às {} UTC",
                e.title,
                country,
                e.date.format("%H:%M")
            );
            Some(RawNewsItem {
                source: "Calendário Econômico".to_string(),
                headline,
                actual: None, // honestidade: nunca inventar o valor divulgado
                forecast: non_empty(&e.forecast),
                previous: non_empty(&e.previous),
                impact_hint: format!("calendar-{}", e.impact.to_lowercase()),
                timestamp_utc: e.date.to_rfc3339(),
            })
        })
        .collect()
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Mapeia os códigos de moeda do feed para um rótulo legível. Cai no próprio
/// código quando não reconhecido (não trava a exibição de nenhum país).
fn country_label(code: &str) -> &str {
    match code {
        "USD" => "EUA",
        "EUR" => "Zona do Euro",
        "GBP" => "Reino Unido",
        "JPY" => "Japão",
        "CNY" => "China",
        "AUD" => "Austrália",
        "CAD" => "Canadá",
        "CHF" => "Suíça",
        "NZD" => "Nova Zelândia",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(title: &str, country: &str, impact: &str, date: &str, forecast: &str, previous: &str) -> CalendarEvent {
        CalendarEvent {
            title: title.to_string(),
            country: country.to_string(),
            impact: impact.to_string(),
            date: date.parse().unwrap(),
            forecast: forecast.to_string(),
            previous: previous.to_string(),
        }
    }

    #[test]
    fn deserializa_json_real_da_forexfactory() {
        // Amostra real capturada do feed (formato exato retornado pela API).
        let json = r#"[
            {"title":"CPI m/m","country":"AUD","date":"2026-07-28T21:30:00-04:00","impact":"High","forecast":"0.2%","previous":"-0.7%"},
            {"title":"German ifo Business Climate","country":"EUR","date":"2026-07-27T04:00:00-04:00","impact":"Low","forecast":"86.1","previous":"85.6"}
        ]"#;
        let events: Vec<CalendarEvent> = serde_json::from_str(json).expect("deve desserializar o JSON real da ForexFactory");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "CPI m/m");
        assert_eq!(events[0].impact, "High");
    }

    #[test]
    fn emite_aviso_so_dentro_da_janela_e_so_high_medium() {
        let now: DateTime<Utc> = "2026-07-31T10:00:00Z".parse().unwrap();
        let events = vec![
            // Dentro da janela (15 min), alto impacto -> DEVE emitir
            ev("CPI m/m", "USD", "High", "2026-07-31T10:15:00Z", "0.2%", "0.3%"),
            // Dentro da janela, médio impacto -> DEVE emitir
            ev("Retail Sales", "USD", "Medium", "2026-07-31T10:20:00Z", "0.4%", "0.1%"),
            // Dentro da janela, mas BAIXO impacto -> NÃO deve emitir
            ev("Ibope Confidence", "EUR", "Low", "2026-07-31T10:10:00Z", "", ""),
            // Longe no futuro (2h) -> fora da janela de 30min, NÃO deve emitir
            ev("FOMC Statement", "USD", "High", "2026-07-31T12:00:00Z", "", ""),
            // Já passou -> NÃO deve emitir
            ev("NFP", "USD", "High", "2026-07-31T09:00:00Z", "180K", "150K"),
        ];
        let out = build_heads_up(&events, now, 30);
        assert_eq!(out.len(), 2, "só CPI e Retail Sales devem passar");
        assert!(out[0].headline.contains("CPI m/m"));
        assert!(out[0].headline.contains("EUA"));
        assert!(out[0].headline.contains("às 10:15 UTC"));
        assert_eq!(out[0].actual, None, "nunca inventar o valor divulgado");
        assert_eq!(out[0].forecast.as_deref(), Some("0.2%"));
        assert_eq!(out[0].previous.as_deref(), Some("0.3%"));
        assert!(out[1].headline.contains("Retail Sales"));
    }

    #[test]
    fn forecast_e_previous_vazios_viram_none_nao_string_vazia() {
        let now: DateTime<Utc> = "2026-07-31T10:00:00Z".parse().unwrap();
        let events = vec![ev("ECOFIN Meetings", "EUR", "High", "2026-07-31T10:05:00Z", "", "")];
        let out = build_heads_up(&events, now, 30);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].forecast, None);
        assert_eq!(out[0].previous, None);
    }

    #[test]
    fn country_label_mapeia_principais_e_cai_no_codigo_para_desconhecidos() {
        assert_eq!(country_label("USD"), "EUA");
        assert_eq!(country_label("EUR"), "Zona do Euro");
        assert_eq!(country_label("XXX"), "XXX");
    }
}
