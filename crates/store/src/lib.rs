//! AppDoctor's local database: one SQLite file in the user's data directory.
//!
//! Everything a user tracks lives here and nowhere else: their apps,
//! keywords per app and country, every rank check (the history), cached
//! keyword popularity, competitors and plain settings. Secrets never do;
//! those go to the OS keychain.
//!
//! The connection runs in WAL mode so a long write (a rank check recording
//! hundreds of snapshots) never blocks the UI reading the keyword table.
//! Timestamps are Unix seconds, passed in by the caller so tests are
//! deterministic.

mod migrations;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};

pub type KeywordId = i64;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not create the data directory: {0}")]
    Io(#[from] std::io::Error),
    #[error("keyword is empty")]
    EmptyKeyword,
}

pub type Result<T> = std::result::Result<T, Error>;

/// An app the user tracks: their own or a competitor's, with the details
/// last fetched from the store.
#[derive(Debug, Clone, PartialEq)]
pub struct AppRecord {
    pub id: u64,
    pub name: String,
    pub bundle_id: Option<String>,
    pub developer: Option<String>,
    pub icon_url: Option<String>,
    /// The storefront the app was added from; new keywords default to it.
    pub country: String,
    pub rating_count: u64,
    pub average_rating: f32,
    pub added_at: i64,
    pub updated_at: i64,
}

/// A keyword row as the keyword table shows it: the keyword plus its latest
/// and previous rank checks.
#[derive(Debug, Clone, PartialEq)]
pub struct KeywordRow {
    pub id: KeywordId,
    pub app_id: u64,
    pub term: String,
    pub country: String,
    pub created_at: i64,
    pub latest: Option<CheckSummary>,
    pub previous: Option<CheckSummary>,
    pub popularity: Option<u8>,
    /// The top results at the latest check, best first.
    pub top_apps: Vec<TopApp>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckSummary {
    /// 1-based rank, or `None` when the app wasn't in the results.
    pub rank: Option<u32>,
    pub checked_at: i64,
    pub difficulty: Option<f32>,
    /// How many results the search returned.
    pub results_count: u32,
}

/// An app in a keyword's top results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopApp {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
}

/// How a keyword's rank moved between its last two checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankChange {
    /// Never checked.
    Unchecked,
    /// Checked once; nothing to compare with.
    First,
    Same,
    /// Moved up this many places.
    Up(u32),
    Down(u32),
    /// Wasn't ranked last time, is now.
    Entered,
    /// Was ranked last time, isn't now.
    Dropped,
}

impl KeywordRow {
    pub fn rank(&self) -> Option<u32> {
        self.latest.and_then(|check| check.rank)
    }

    pub fn change(&self) -> RankChange {
        let Some(latest) = self.latest else {
            return RankChange::Unchecked;
        };
        let Some(previous) = self.previous else {
            return RankChange::First;
        };
        match (previous.rank, latest.rank) {
            (None, None) => RankChange::Same,
            (None, Some(_)) => RankChange::Entered,
            (Some(_), None) => RankChange::Dropped,
            (Some(before), Some(now)) if now < before => RankChange::Up(before - now),
            (Some(before), Some(now)) if now > before => RankChange::Down(now - before),
            _ => RankChange::Same,
        }
    }
}

/// One rank check of one keyword.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub keyword_id: KeywordId,
    pub checked_at: i64,
    pub rank: Option<u32>,
    /// How many results the search returned (the depth `rank` was looked for in).
    pub results_count: u32,
    pub difficulty: Option<f32>,
    /// The search source's name, so history from different sources isn't mixed.
    pub source: String,
    /// The top results, best first, for the keyword table and competitor tracking.
    pub top_apps: Vec<TopApp>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Popularity {
    pub term: String,
    pub country: String,
    /// Apple Ads popularity, 0–100.
    pub popularity: u8,
    pub source: String,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Competitor {
    pub app_id: u64,
    pub competitor_id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    pub added_at: i64,
}

/// A handle to the database. Cheap to share behind an `Arc`; every call
/// takes the connection briefly.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Opens (creating if needed) the database at `path` and migrates it.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrations::run(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.conn().pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    // Apps

    /// Inserts the app or refreshes its store details, keeping `added_at`.
    pub fn upsert_app(&self, app: &AppRecord) -> Result<()> {
        self.conn().execute(
            "INSERT INTO apps (id, name, bundle_id, developer, icon_url, country, rating_count,
                               average_rating, added_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT (id) DO UPDATE SET
                name = excluded.name, bundle_id = excluded.bundle_id,
                developer = excluded.developer, icon_url = excluded.icon_url,
                rating_count = excluded.rating_count, average_rating = excluded.average_rating,
                updated_at = excluded.updated_at",
            params![
                app.id as i64,
                app.name,
                app.bundle_id,
                app.developer,
                app.icon_url,
                app.country,
                app.rating_count as i64,
                app.average_rating,
                app.added_at,
                app.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn apps(&self) -> Result<Vec<AppRecord>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id, name, bundle_id, developer, icon_url, country, rating_count, average_rating,
                    added_at, updated_at
             FROM apps ORDER BY added_at, id",
        )?;
        let rows = statement.query_map([], app_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn app(&self, id: u64) -> Result<Option<AppRecord>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, name, bundle_id, developer, icon_url, country, rating_count,
                        average_rating, added_at, updated_at
                 FROM apps WHERE id = ?1",
                [id as i64],
                app_from_row,
            )
            .optional()?)
    }

    /// Deletes the app with its keywords, history and competitors.
    pub fn delete_app(&self, id: u64) -> Result<()> {
        self.conn().execute("DELETE FROM apps WHERE id = ?1", [id as i64])?;
        Ok(())
    }

    // Keywords

    /// Adds a keyword for an app in one country. The term is trimmed,
    /// lowercased and its whitespace collapsed; adding one that already
    /// exists returns the existing ID.
    pub fn add_keyword(&self, app_id: u64, term: &str, country: &str, now: i64) -> Result<KeywordId> {
        let term = normalize_term(term);
        if term.is_empty() {
            return Err(Error::EmptyKeyword);
        }
        let country = country.to_ascii_lowercase();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO keywords (app_id, term, country, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (app_id, term, country) DO NOTHING",
            params![app_id as i64, term, country, now],
        )?;
        Ok(conn.query_row(
            "SELECT id FROM keywords WHERE app_id = ?1 AND term = ?2 AND country = ?3",
            params![app_id as i64, term, country],
            |row| row.get(0),
        )?)
    }

    pub fn delete_keyword(&self, id: KeywordId) -> Result<()> {
        self.conn().execute("DELETE FROM keywords WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Every keyword of an app with its latest two checks and popularity,
    /// sorted by term then country.
    pub fn keywords(&self, app_id: u64) -> Result<Vec<KeywordRow>> {
        let conn = self.conn();
        let mut statement = conn.prepare_cached(
            "WITH ranked AS (
                SELECT s.keyword_id, s.rank, s.checked_at, s.difficulty, s.results_count, s.top_apps,
                       ROW_NUMBER() OVER (PARTITION BY s.keyword_id
                                          ORDER BY s.checked_at DESC, s.id DESC) AS rn
                FROM rank_snapshots s JOIN keywords k ON k.id = s.keyword_id
                WHERE k.app_id = ?1
             )
             SELECT k.id, k.app_id, k.term, k.country, k.created_at,
                    cur.checked_at, cur.rank, cur.difficulty, cur.results_count,
                    prev.checked_at, prev.rank, prev.difficulty, prev.results_count,
                    p.popularity, cur.top_apps
             FROM keywords k
             LEFT JOIN ranked cur ON cur.keyword_id = k.id AND cur.rn = 1
             LEFT JOIN ranked prev ON prev.keyword_id = k.id AND prev.rn = 2
             LEFT JOIN popularity_cache p ON p.term = k.term AND p.country = k.country
             WHERE k.app_id = ?1
             ORDER BY k.term, k.country",
        )?;
        let rows = statement.query_map([app_id as i64], |row| {
            let check = |offset: usize| -> rusqlite::Result<Option<CheckSummary>> {
                let checked_at: Option<i64> = row.get(offset)?;
                Ok(match checked_at {
                    Some(checked_at) => Some(CheckSummary {
                        checked_at,
                        rank: row.get::<_, Option<i64>>(offset + 1)?.map(|rank| rank as u32),
                        difficulty: row.get::<_, Option<f64>>(offset + 2)?.map(|d| d as f32),
                        results_count: row.get::<_, Option<u32>>(offset + 3)?.unwrap_or(0),
                    }),
                    None => None,
                })
            };
            Ok(KeywordRow {
                id: row.get(0)?,
                app_id: row.get::<_, i64>(1)? as u64,
                term: row.get(2)?,
                country: row.get(3)?,
                created_at: row.get(4)?,
                latest: check(5)?,
                previous: check(9)?,
                popularity: row.get::<_, Option<i64>>(13)?.map(|p| p.clamp(0, 100) as u8),
                top_apps: row
                    .get::<_, Option<String>>(14)?
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // Rank history

    pub fn record_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        let top = serde_json::to_string(&snapshot.top_apps).unwrap_or_else(|_| "[]".into());
        self.conn().execute(
            "INSERT INTO rank_snapshots (keyword_id, checked_at, rank, results_count, difficulty,
                                         source, top_apps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snapshot.keyword_id,
                snapshot.checked_at,
                snapshot.rank.map(i64::from),
                snapshot.results_count,
                snapshot.difficulty.map(f64::from),
                snapshot.source,
                top,
            ],
        )?;
        Ok(())
    }

    /// A keyword's checks, newest first.
    pub fn rank_history(&self, keyword_id: KeywordId, limit: usize) -> Result<Vec<Snapshot>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT keyword_id, checked_at, rank, results_count, difficulty, source, top_apps
             FROM rank_snapshots WHERE keyword_id = ?1
             ORDER BY checked_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![keyword_id, limit as i64], |row| {
            let top: String = row.get(6)?;
            Ok(Snapshot {
                keyword_id: row.get(0)?,
                checked_at: row.get(1)?,
                rank: row.get::<_, Option<i64>>(2)?.map(|rank| rank as u32),
                results_count: row.get(3)?,
                difficulty: row.get::<_, Option<f64>>(4)?.map(|d| d as f32),
                source: row.get(5)?,
                top_apps: serde_json::from_str(&top).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // Popularity

    pub fn set_popularity(&self, popularity: &Popularity) -> Result<()> {
        self.conn().execute(
            "INSERT INTO popularity_cache (term, country, popularity, source, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (term, country) DO UPDATE SET
                popularity = excluded.popularity, source = excluded.source,
                fetched_at = excluded.fetched_at",
            params![
                normalize_term(&popularity.term),
                popularity.country.to_ascii_lowercase(),
                popularity.popularity,
                popularity.source,
                popularity.fetched_at,
            ],
        )?;
        Ok(())
    }

    pub fn popularity(&self, term: &str, country: &str) -> Result<Option<Popularity>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT term, country, popularity, source, fetched_at FROM popularity_cache
                 WHERE term = ?1 AND country = ?2",
                params![normalize_term(term), country.to_ascii_lowercase()],
                |row| {
                    Ok(Popularity {
                        term: row.get(0)?,
                        country: row.get(1)?,
                        popularity: row.get::<_, i64>(2)?.clamp(0, 100) as u8,
                        source: row.get(3)?,
                        fetched_at: row.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    // Competitors

    pub fn add_competitor(&self, competitor: &Competitor) -> Result<()> {
        self.conn().execute(
            "INSERT INTO competitors (app_id, competitor_id, name, icon_url, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (app_id, competitor_id) DO UPDATE SET
                name = excluded.name, icon_url = excluded.icon_url",
            params![
                competitor.app_id as i64,
                competitor.competitor_id as i64,
                competitor.name,
                competitor.icon_url,
                competitor.added_at,
            ],
        )?;
        Ok(())
    }

    pub fn competitors(&self, app_id: u64) -> Result<Vec<Competitor>> {
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT app_id, competitor_id, name, icon_url, added_at FROM competitors
             WHERE app_id = ?1 ORDER BY added_at, competitor_id",
        )?;
        let rows = statement.query_map([app_id as i64], |row| {
            Ok(Competitor {
                app_id: row.get::<_, i64>(0)? as u64,
                competitor_id: row.get::<_, i64>(1)? as u64,
                name: row.get(2)?,
                icon_url: row.get(3)?,
                added_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // Settings (non-secret)

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

/// The form keywords are stored and compared in.
pub fn normalize_term(term: &str) -> String {
    term.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn app_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppRecord> {
    Ok(AppRecord {
        id: row.get::<_, i64>(0)? as u64,
        name: row.get(1)?,
        bundle_id: row.get(2)?,
        developer: row.get(3)?,
        icon_url: row.get(4)?,
        country: row.get(5)?,
        rating_count: row.get::<_, i64>(6)? as u64,
        average_rating: row.get::<_, f64>(7)? as f32,
        added_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests;
