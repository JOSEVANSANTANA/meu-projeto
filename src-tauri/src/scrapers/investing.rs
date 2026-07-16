use crate::models::RawNewsItem;
use crate::scrapers::{browser_client, polite_delay};
use anyhow::Result;
use scraper::{Html, Selector};

const CALENDAR_URL: &str = "https://www.investing.com/economic-calendar/";

/// Conector do calendário econômico do Investing.com.
///
/// FOCO: apenas eventos de ALTO IMPACTO (3 estrelas / "High Volatility
/// Expected") dos EUA — CPI, Payroll, FOMC etc. Eventos de 1-2 estrelas
/// são ignorados na origem para não desperdiçar chamadas ao Gemini.
///
/// NOTA DE MANUTENÇÃO: seletores CSS de sites de terceiros mudam sem
/// aviso. Se o parse retornar 0 itens por vários ciclos, inspecione o
/// HTML atual e ajuste os seletores abaixo.
pub async fn fetch_high_impact_events() -> Result<Vec<RawNewsItem>> {
    // Anti-bot: delay aleatório de 2–5s ANTES da requisição
    polite_delay().await;

    let client = browser_client()?;
    let html = client
        .get(CALENDAR_URL)
        .header("Referer", "https://www.investing.com/")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_calendar(&html)
}

fn parse_calendar(html: &str) -> Result<Vec<RawNewsItem>> {
    let doc = Html::parse_document(html);

    let row_sel = Selector::parse("tr.js-event-item").unwrap();
    let stars_sel = Selector::parse("td.sentiment i.grayFullBullishIcon").unwrap();
    let event_sel = Selector::parse("td.event").unwrap();
    let actual_sel = Selector::parse("td[id^='eventActual']").unwrap();
    let forecast_sel = Selector::parse("td[id^='eventForecast']").unwrap();
    let previous_sel = Selector::parse("td[id^='eventPrevious']").unwrap();
    let country_sel = Selector::parse("td.flagCur span[title]").unwrap();

    let mut items = Vec::new();

    for row in doc.select(&row_sel) {
        // 3 ícones de "touro cheio" = evento 3 estrelas (alto impacto)
        let stars = row.select(&stars_sel).count();
        if stars < 3 {
            continue;
        }

        // Apenas Estados Unidos (o que move o ES diretamente)
        let country = row
            .select(&country_sel)
            .next()
            .and_then(|el| el.value().attr("title"))
            .unwrap_or("");
        if !country.contains("United States") {
            continue;
        }

        let text_of = |sel: &Selector| -> Option<String> {
            row.select(sel).next().map(|el| {
                el.text().collect::<String>().trim().to_string()
            })
        };

        let headline = match text_of(&event_sel) {
            Some(h) if !h.is_empty() => h,
            _ => continue,
        };

        // Sem "actual" ainda = evento futuro; só analisamos após a divulgação
        let actual = text_of(&actual_sel).filter(|v| !v.is_empty() && v != "\u{a0}");
        if actual.is_none() {
            continue;
        }

        let datetime = row
            .value()
            .attr("data-event-datetime")
            .unwrap_or_default()
            .to_string();

        items.push(RawNewsItem {
            source: "Investing.com".to_string(),
            headline,
            actual,
            forecast: text_of(&forecast_sel).filter(|v| !v.is_empty()),
            previous: text_of(&previous_sel).filter(|v| !v.is_empty()),
            impact_hint: "3-star".to_string(),
            timestamp_utc: datetime,
        });
    }

    log::info!("Investing.com: {} evento(s) 3-estrelas com dado divulgado", items.len());
    Ok(items)
}
