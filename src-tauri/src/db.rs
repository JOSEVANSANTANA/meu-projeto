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

    pub fn insert(&self, dedup_key: &str, a: &GeminiAnalysis) -> Result<NewsEvent> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"INSERT OR IGNORE INTO news_events
               (dedup_key, received_at_utc, source, event, impact_level,
                actual, forecast, previous, sentiment, prob_up, prob_down,
                projected_target_pts, rationale, alert_type)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"#,
            params![
                dedup_key,
                now,
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
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(NewsEvent {
            id,
            dedup_key: dedup_key.to_string(),
            received_at_utc: now,
            analysis: a.clone(),
        })
    }

    /// Histórico recente para hidratar o dashboard ao abrir o app.
    pub fn recent(&self, limit: u32) -> Result<Vec<NewsEvent>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, dedup_key, received_at_utc, source, event, impact_level,
                      actual, forecast, previous, sentiment, prob_up, prob_down,
                      projected_target_pts, rationale, alert_type
               FROM news_events ORDER BY id DESC LIMIT ?1"#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(NewsEvent {
                id: row.get(0)?,
                dedup_key: row.get(1)?,
                received_at_utc: row.get(2)?,
                analysis: GeminiAnalysis {
                    source: row.get(3)?,
                    event: row.get(4)?,
                    impact_level: row.get(5)?,
                    actual: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    forecast: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    previous: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    sentiment: row.get(9)?,
                    sp500_direction_probability: DirectionProbability {
                        up: row.get::<_, i64>(10)? as u8,
                        down: row.get::<_, i64>(11)? as u8,
                    },
                    projected_target_pts: row
                        .get::<_, Option<String>>(12)?
                        .unwrap_or_default(),
                    rationale: row.get::<_, Option<String>>(13)?.unwrap_or_default(),
                    alert_type: row.get::<_, Option<String>>(14)?.unwrap_or_default(),
                },
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
