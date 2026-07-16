use crate::models::{DirectionProbability, GeminiAnalysis, NewsEvent};
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

/// Camada de persistência local (SQLite embutido, sem servidor).
/// WAL mode: escrita do loop de scraping não bloqueia leituras da UI.
pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS news_events (
                id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                dedup_key            TEXT NOT NULL UNIQUE,
                received_at_utc      TEXT NOT NULL,
                source               TEXT NOT NULL,
                event                TEXT NOT NULL,
                impact_level         TEXT NOT NULL,
                actual               TEXT,
                forecast             TEXT,
                previous             TEXT,
                sentiment            TEXT NOT NULL,
                prob_up              INTEGER NOT NULL,
                prob_down            INTEGER NOT NULL,
                projected_target_pts TEXT,
                rationale            TEXT,
                alert_type           TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_events_received
                ON news_events (received_at_utc DESC);
            "#,
        )?;

        // Migrações idempotentes (ignoram erro se a coluna já existir).
        for stmt in [
            "ALTER TABLE news_events ADD COLUMN asset TEXT NOT NULL DEFAULT ''",
            // Autoaprendizagem: preço-base na hora da notícia, movimento real
            // medido depois, flag de pontuado e acerto de direção (1/0/NULL).
            "ALTER TABLE news_events ADD COLUMN baseline_price REAL",
            "ALTER TABLE news_events ADD COLUMN realized_pts REAL",
            "ALTER TABLE news_events ADD COLUMN scored INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE news_events ADD COLUMN hit INTEGER",
        ] {
            let _ = conn.execute(stmt, []);
        }

        Ok(Self { conn })
    }

    /// Retorna true se o evento é novo (chave ainda não vista).
    pub fn is_new(&self, dedup_key: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM news_events WHERE dedup_key = ?1",
            params![dedup_key],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }

    pub fn insert(
        &self,
        dedup_key: &str,
        asset: &str,
        baseline_price: Option<f64>,
        a: &GeminiAnalysis,
    ) -> Result<NewsEvent> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"INSERT OR IGNORE INTO news_events
               (dedup_key, received_at_utc, asset, source, event, impact_level,
                actual, forecast, previous, sentiment, prob_up, prob_down,
                projected_target_pts, rationale, alert_type, baseline_price)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)"#,
            params![
                dedup_key,
                now,
                asset,
                a.source,
                a.event,
                a.impact_level,
                a.actual,
                a.forecast,
                a.previous,
                a.sentiment,
                a.sp500_direction_probability.up,
                a.sp500_direction_probability.down,
                a.projected_target_pts,
                a.rationale,
                a.alert_type,
                baseline_price,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(NewsEvent {
            id,
            dedup_key: dedup_key.to_string(),
            received_at_utc: now,
            asset: asset.to_string(),
            analysis: a.clone(),
        })
    }

    /// Histórico recente para hidratar o dashboard ao abrir o app.
    pub fn recent(&self, limit: u32) -> Result<Vec<NewsEvent>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, dedup_key, received_at_utc, asset, source, event, impact_level,
                      actual, forecast, previous, sentiment, prob_up, prob_down,
                      projected_target_pts, rationale, alert_type
               FROM news_events ORDER BY id DESC LIMIT ?1"#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(NewsEvent {
                id: row.get(0)?,
                dedup_key: row.get(1)?,
                received_at_utc: row.get(2)?,
                asset: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                analysis: GeminiAnalysis {
                    source: row.get(4)?,
                    event: row.get(5)?,
                    impact_level: row.get(6)?,
                    actual: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    forecast: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    previous: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    sentiment: row.get(10)?,
                    sp500_direction_probability: DirectionProbability {
                        up: row.get::<_, i64>(11)? as u8,
                        down: row.get::<_, i64>(12)? as u8,
                    },
                    projected_target_pts: row
                        .get::<_, Option<String>>(13)?
                        .unwrap_or_default(),
                    rationale: row.get::<_, Option<String>>(14)?.unwrap_or_default(),
                    alert_type: row.get::<_, Option<String>>(15)?.unwrap_or_default(),
                },
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ---- Autoaprendizagem: pontuação das predições vs. preço real ----------

    /// Predições ainda não pontuadas, com preço-base, cujo horizonte já passou.
    pub fn pending_scores(&self, older_than_secs: i64) -> Result<Vec<PendingScore>> {
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::seconds(older_than_secs)).to_rfc3339();
        let mut stmt = self.conn.prepare(
            r#"SELECT id, asset, baseline_price, projected_target_pts, received_at_utc
               FROM news_events
               WHERE scored = 0 AND baseline_price IS NOT NULL AND received_at_utc <= ?1
               ORDER BY id ASC LIMIT 40"#,
        )?;
        let rows = stmt.query_map(params![cutoff], |row| {
            Ok(PendingScore {
                id: row.get(0)?,
                asset: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                baseline_price: row.get(2)?,
                projected_target_pts: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                received_at_utc: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Grava o resultado da pontuação. `hit`: Some(true/false) para direção,
    /// None quando não pontuável (neutro / mercado fechado / sem símbolo).
    pub fn record_score(&self, id: i64, realized_pts: Option<f64>, hit: Option<bool>) -> Result<()> {
        self.conn.execute(
            "UPDATE news_events SET scored = 1, realized_pts = ?2, hit = ?3 WHERE id = ?1",
            params![id, realized_pts, hit.map(|h| h as i64)],
        )?;
        Ok(())
    }

    /// Marca como pontuado sem resultado (não pontuável), sem mexer no preço.
    pub fn mark_unscorable(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE news_events SET scored = 1, hit = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Estatísticas de acerto agregadas (base do placar e do aprendizado).
    pub fn accuracy_stats(&self) -> Result<AccuracyStats> {
        // Acerto de direção (só onde hit não é NULL).
        let (hits, scored_dir): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(hit),0), COUNT(hit) FROM news_events WHERE hit IS NOT NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        // Magnitude: soma |previsto| e |realizado| onde há previsão direcional.
        let mut stmt = self.conn.prepare(
            "SELECT projected_target_pts, realized_pts FROM news_events \
             WHERE scored = 1 AND realized_pts IS NOT NULL",
        )?;
        let mut sum_pred = 0.0f64;
        let mut sum_real = 0.0f64;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
            ))
        })?;
        for row in rows.flatten() {
            let pred = parse_pts(&row.0).abs();
            if pred > 0.0 {
                sum_pred += pred;
                sum_real += row.1.abs();
            }
        }
        let magnitude_factor = if sum_pred > 0.0 {
            (sum_real / sum_pred).clamp(0.2, 5.0)
        } else {
            1.0
        };

        // Acerto por nível de impacto.
        let mut by_impact = Vec::new();
        let mut s2 = self.conn.prepare(
            "SELECT impact_level, COALESCE(SUM(hit),0), COUNT(hit) FROM news_events \
             WHERE hit IS NOT NULL GROUP BY impact_level",
        )?;
        let ir = s2.query_map([], |r| {
            Ok(ImpactAccuracy {
                impact_level: r.get(0)?,
                hits: r.get(1)?,
                total: r.get(2)?,
            })
        })?;
        for row in ir.flatten() {
            by_impact.push(row);
        }

        Ok(AccuracyStats {
            hits,
            scored: scored_dir,
            hit_rate: if scored_dir > 0 {
                (hits as f64 / scored_dir as f64 * 100.0).round() as i64
            } else {
                0
            },
            magnitude_factor: (magnitude_factor * 100.0).round() / 100.0,
            by_impact,
        })
    }
}

/// Predição aguardando pontuação.
pub struct PendingScore {
    pub id: i64,
    pub asset: String,
    pub baseline_price: f64,
    pub projected_target_pts: String,
    pub received_at_utc: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ImpactAccuracy {
    pub impact_level: String,
    pub hits: i64,
    pub total: i64,
}

#[derive(serde::Serialize, Clone)]
pub struct AccuracyStats {
    pub hits: i64,
    pub scored: i64,
    pub hit_rate: i64,
    /// >1 = a IA SUBESTIMA a magnitude; <1 = SUPERESTIMA.
    pub magnitude_factor: f64,
    pub by_impact: Vec<ImpactAccuracy>,
}

/// Extrai o número (com sinal) de "projected_target_pts" — igual ao frontend.
/// "+15 pts" -> 15 ; "-25 pts" -> -25 ; "0 pts" / "N/A" -> 0.
pub fn parse_pts(s: &str) -> f64 {
    let s = s.replace(',', ".");
    let mut num = String::new();
    let mut started = false;
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            started = true;
        } else if c == '-' && !started {
            num.push(c);
        } else if started {
            break;
        }
    }
    num.parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::parse_pts;

    #[test]
    fn parse_pts_sinais() {
        assert_eq!(parse_pts("+15 pts"), 15.0);
        assert_eq!(parse_pts("-25 pts"), -25.0);
        assert_eq!(parse_pts("0 pts"), 0.0);
        assert_eq!(parse_pts("N/A"), 0.0);
        assert_eq!(parse_pts("+8.5 pts"), 8.5);
    }
}
