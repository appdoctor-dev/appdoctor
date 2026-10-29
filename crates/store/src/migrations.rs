use rusqlite::Connection;

/// Schema migrations, applied in order. `PRAGMA user_version` records how
/// many have run. Never edit a shipped migration; append a new one.
const MIGRATIONS: &[&str] = &[
    // 1: apps, keywords, rank history, popularity cache, competitors, settings.
    "CREATE TABLE apps (
        id              INTEGER PRIMARY KEY,
        name            TEXT NOT NULL,
        bundle_id       TEXT,
        developer       TEXT,
        icon_url        TEXT,
        country         TEXT NOT NULL,
        rating_count    INTEGER NOT NULL DEFAULT 0,
        average_rating  REAL NOT NULL DEFAULT 0,
        added_at        INTEGER NOT NULL,
        updated_at      INTEGER NOT NULL
    );

    CREATE TABLE keywords (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id      INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        term        TEXT NOT NULL,
        country     TEXT NOT NULL,
        created_at  INTEGER NOT NULL,
        UNIQUE (app_id, term, country)
    );

    CREATE TABLE rank_snapshots (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        keyword_id     INTEGER NOT NULL REFERENCES keywords (id) ON DELETE CASCADE,
        checked_at     INTEGER NOT NULL,
        rank           INTEGER,
        results_count  INTEGER NOT NULL,
        difficulty     REAL,
        source         TEXT NOT NULL,
        top_app_ids    TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX rank_snapshots_by_keyword ON rank_snapshots (keyword_id, checked_at DESC);

    CREATE TABLE popularity_cache (
        term        TEXT NOT NULL,
        country     TEXT NOT NULL,
        popularity  INTEGER NOT NULL,
        source      TEXT NOT NULL,
        fetched_at  INTEGER NOT NULL,
        PRIMARY KEY (term, country)
    );

    CREATE TABLE competitors (
        app_id         INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        competitor_id  INTEGER NOT NULL,
        name           TEXT NOT NULL,
        icon_url       TEXT,
        added_at       INTEGER NOT NULL,
        PRIMARY KEY (app_id, competitor_id)
    );

    CREATE TABLE settings (
        key    TEXT PRIMARY KEY,
        value  TEXT NOT NULL
    );",
];

pub(crate) fn run(conn: &mut Connection) -> rusqlite::Result<()> {
    let applied: usize = conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))? as usize;
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(migration)?;
        tx.pragma_update(None, "user_version", (index + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) const LATEST: usize = MIGRATIONS.len();
