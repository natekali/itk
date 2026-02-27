use chrono::Utc;
use rusqlite::{params, Connection, Result};
use std::path::PathBuf;

/// Open (or create) the history database, applying the schema.
pub fn open() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&path)?;
    apply_schema(&conn)?;
    Ok(conn)
}

fn db_path() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base).join("itk").join("history.db")
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{home}/.local/share")
        });
        PathBuf::from(base).join("itk").join("history.db")
    }
}

fn apply_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS runs (
             id              INTEGER PRIMARY KEY AUTOINCREMENT,
             ts              TEXT    NOT NULL,
             content_type    TEXT    NOT NULL,
             original_tokens INTEGER NOT NULL,
             cleaned_tokens  INTEGER NOT NULL,
             savings_pct     REAL    NOT NULL
         );",
    )
}

/// Record one ITK run into the history database.
pub fn record_run(
    conn: &mut Connection,
    content_type: &str,
    original_tokens: u64,
    cleaned_tokens: u64,
) -> Result<()> {
    let savings_pct = if original_tokens > 0 {
        100.0 - (cleaned_tokens as f64 / original_tokens as f64 * 100.0)
    } else {
        0.0
    };
    conn.execute(
        "INSERT INTO runs (ts, content_type, original_tokens, cleaned_tokens, savings_pct)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            Utc::now().to_rfc3339(),
            content_type,
            original_tokens as i64,
            cleaned_tokens as i64,
            savings_pct,
        ],
    )?;
    Ok(())
}

// ── Query types ───────────────────────────────────────────────────────────────

pub struct TodayStats {
    pub runs: u64,
    pub original_tokens: u64,
    pub cleaned_tokens: u64,
    pub avg_savings_pct: f64,
}

pub struct TotalStats {
    pub runs: u64,
    pub original_tokens: u64,
    pub cleaned_tokens: u64,
    pub avg_savings_pct: f64,
}

pub struct RunRecord {
    pub ts: String,
    pub content_type: String,
    pub original_tokens: i64,
    pub cleaned_tokens: i64,
    pub savings_pct: f64,
}

pub fn query_today(conn: &Connection) -> Result<TodayStats> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(original_tokens), 0),
                COALESCE(SUM(cleaned_tokens), 0),
                COALESCE(AVG(savings_pct), 0.0)
         FROM runs
         WHERE ts >= ?1",
        params![format!("{today}T00:00:00Z")],
        |row| {
            Ok(TodayStats {
                runs: row.get::<_, i64>(0)? as u64,
                original_tokens: row.get::<_, i64>(1)? as u64,
                cleaned_tokens: row.get::<_, i64>(2)? as u64,
                avg_savings_pct: row.get(3)?,
            })
        },
    )
}

pub fn query_total(conn: &Connection) -> Result<TotalStats> {
    conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(original_tokens), 0),
                COALESCE(SUM(cleaned_tokens), 0),
                COALESCE(AVG(savings_pct), 0.0)
         FROM runs",
        [],
        |row| {
            Ok(TotalStats {
                runs: row.get::<_, i64>(0)? as u64,
                original_tokens: row.get::<_, i64>(1)? as u64,
                cleaned_tokens: row.get::<_, i64>(2)? as u64,
                avg_savings_pct: row.get(3)?,
            })
        },
    )
}

pub fn query_history(conn: &Connection, limit: u32) -> Result<Vec<RunRecord>> {
    let mut stmt = conn.prepare(
        "SELECT ts, content_type, original_tokens, cleaned_tokens, savings_pct
         FROM runs
         ORDER BY id DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(RunRecord {
            ts: row.get(0)?,
            content_type: row.get(1)?,
            original_tokens: row.get(2)?,
            cleaned_tokens: row.get(3)?,
            savings_pct: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn query_by_type(conn: &Connection) -> Result<Vec<(String, u64, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT content_type, COUNT(*), AVG(savings_pct)
         FROM runs
         GROUP BY content_type
         ORDER BY COUNT(*) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)? as u64,
            row.get::<_, f64>(2)?,
        ))
    })?;
    rows.collect()
}

// ── Date-range queries ───────────────────────────────────────────────────────

/// Stats for a date range (since N days ago).
pub fn query_range(conn: &Connection, since_days: u32) -> Result<TotalStats> {
    let cutoff = Utc::now() - chrono::Duration::days(since_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(original_tokens), 0),
                COALESCE(SUM(cleaned_tokens), 0),
                COALESCE(AVG(savings_pct), 0.0)
         FROM runs
         WHERE ts >= ?1",
        params![cutoff_str],
        |row| {
            Ok(TotalStats {
                runs: row.get::<_, i64>(0)? as u64,
                original_tokens: row.get::<_, i64>(1)? as u64,
                cleaned_tokens: row.get::<_, i64>(2)? as u64,
                avg_savings_pct: row.get(3)?,
            })
        },
    )
}

/// Daily breakdown: (date_str, runs, original_tokens, cleaned_tokens, avg_savings_pct).
pub struct DailyStats {
    pub date: String,
    pub runs: u64,
    pub original_tokens: u64,
    pub cleaned_tokens: u64,
    pub avg_savings_pct: f64,
}

pub fn query_daily(conn: &Connection, since_days: u32) -> Result<Vec<DailyStats>> {
    let cutoff = Utc::now() - chrono::Duration::days(since_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT SUBSTR(ts, 1, 10) AS day,
                COUNT(*),
                COALESCE(SUM(original_tokens), 0),
                COALESCE(SUM(cleaned_tokens), 0),
                COALESCE(AVG(savings_pct), 0.0)
         FROM runs
         WHERE ts >= ?1
         GROUP BY day
         ORDER BY day DESC",
    )?;
    let rows = stmt.query_map(params![cutoff_str], |row| {
        Ok(DailyStats {
            date: row.get(0)?,
            runs: row.get::<_, i64>(1)? as u64,
            original_tokens: row.get::<_, i64>(2)? as u64,
            cleaned_tokens: row.get::<_, i64>(3)? as u64,
            avg_savings_pct: row.get(4)?,
        })
    })?;
    rows.collect()
}

/// History filtered by date range.
pub fn query_history_since(conn: &Connection, since_days: u32, limit: u32) -> Result<Vec<RunRecord>> {
    let cutoff = Utc::now() - chrono::Duration::days(since_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT ts, content_type, original_tokens, cleaned_tokens, savings_pct
         FROM runs
         WHERE ts >= ?1
         ORDER BY id DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![cutoff_str, limit as i64], |row| {
        Ok(RunRecord {
            ts: row.get(0)?,
            content_type: row.get(1)?,
            original_tokens: row.get(2)?,
            cleaned_tokens: row.get(3)?,
            savings_pct: row.get(4)?,
        })
    })?;
    rows.collect()
}

/// All runs in a date range (for export).
pub fn query_all_since(conn: &Connection, since_days: u32) -> Result<Vec<RunRecord>> {
    let cutoff = Utc::now() - chrono::Duration::days(since_days as i64);
    let cutoff_str = cutoff.to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT ts, content_type, original_tokens, cleaned_tokens, savings_pct
         FROM runs
         WHERE ts >= ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![cutoff_str], |row| {
        Ok(RunRecord {
            ts: row.get(0)?,
            content_type: row.get(1)?,
            original_tokens: row.get(2)?,
            cleaned_tokens: row.get(3)?,
            savings_pct: row.get(4)?,
        })
    })?;
    rows.collect()
}
