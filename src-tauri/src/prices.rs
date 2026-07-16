//! Cotação de referência (grátis, sem broker) via API pública do Yahoo Finance.
//!
//! Usada para PONTUAR as predições da IA contra o movimento real do mercado
//! (base do laço de autoaprendizagem). É uma cotação com atraso (~10-15 min),
//! adequada para MEDIR acertos — não para execução ao vivo.

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::Duration;

/// Mapeia o ativo (nome canônico do instrumento) para o símbolo de futuro do
/// Yahoo. Micros e minis compartilham o mesmo índice subjacente (mesmos pontos).
pub fn yahoo_symbol_for(asset: &str) -> Option<&'static str> {
    let a = asset.to_lowercase();
    if a.contains("nasdaq") {
        Some("NQ=F")
    } else if a.contains("russell") {
        Some("RTY=F")
    } else if a.contains("dow") {
        Some("YM=F")
    } else if a.contains("midcap") {
        Some("EMD=F")
    } else if a.contains("nikkei") {
        Some("NIY=F")
    } else if a.contains("s&p") || a.contains("sp 500") || a.contains("(es)") || a.contains("(mes)")
    {
        Some("ES=F")
    } else {
        None
    }
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .gzip(true)
        .timeout(Duration::from_secs(20))
        .build()?)
}

/// Extrai o último preço do JSON do Yahoo (regularMarketPrice, com fallback
/// para o último fechamento não-nulo da série). Isolado para teste.
pub fn parse_last_price(json: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(json).ok()?;
    let result = &v["chart"]["result"][0];
    if let Some(p) = result["meta"]["regularMarketPrice"].as_f64() {
        return Some(p);
    }
    // Fallback: último close não-nulo da série de candles.
    result["indicators"]["quote"][0]["close"]
        .as_array()?
        .iter()
        .rev()
        .find_map(|x| x.as_f64())
}

/// Busca o último preço para um símbolo do Yahoo (ex.: "ES=F").
pub async fn fetch_last_price(symbol: &str) -> Result<f64> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{symbol}?range=1d&interval=5m"
    );
    let text = client()?
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_last_price(&text).ok_or_else(|| anyhow!("preço ausente na resposta do Yahoo p/ {symbol}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapeia_simbolos() {
        assert_eq!(yahoo_symbol_for("E-mini S&P 500 (ES)"), Some("ES=F"));
        assert_eq!(yahoo_symbol_for("Micro E-mini S&P 500 (MES)"), Some("ES=F"));
        assert_eq!(yahoo_symbol_for("Micro E-mini Nasdaq-100 (MNQ)"), Some("NQ=F"));
        assert_eq!(yahoo_symbol_for("E-mini Dow Jones (YM)"), Some("YM=F"));
        assert_eq!(yahoo_symbol_for("E-mini Russell 2000 (RTY)"), Some("RTY=F"));
        assert_eq!(yahoo_symbol_for("Nikkei 225 (NKD)"), Some("NIY=F"));
        assert_eq!(yahoo_symbol_for("Ativo Desconhecido"), None);
    }

    #[test]
    fn parseia_preco_yahoo() {
        let json = r#"{"chart":{"result":[{"meta":{"regularMarketPrice":7566.5},"indicators":{"quote":[{"close":[7560.0,null,7566.5]}]}}]}}"#;
        assert_eq!(parse_last_price(json), Some(7566.5));
        // Fallback quando não há regularMarketPrice: último close não-nulo.
        let json2 = r#"{"chart":{"result":[{"meta":{},"indicators":{"quote":[{"close":[7560.0,7565.0,null]}]}}]}}"#;
        assert_eq!(parse_last_price(json2), Some(7565.0));
    }
}
