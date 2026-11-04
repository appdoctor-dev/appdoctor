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
        -- JSON array of {id, name, icon_url} for the top results, best first.
        top_apps       TEXT NOT NULL DEFAULT '[]'
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
    // 2: the full result order per check (comma-separated app IDs), so any
    // app's rank (a competitor's) can be read from checks already made.
    "ALTER TABLE rank_snapshots ADD COLUMN ranked_ids TEXT NOT NULL DEFAULT '';",
    // 3: every storefront an app is tracked in (not just the one it was
    // added from), its App Store Connect listing as last pulled, and listing
    // edits an AI suggested for the user to review.
    "CREATE TABLE app_countries (
        app_id    INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        country   TEXT NOT NULL,
        added_at  INTEGER NOT NULL,
        PRIMARY KEY (app_id, country)
    );
    INSERT OR IGNORE INTO app_countries (app_id, country, added_at) SELECT id, country, added_at FROM apps;
    INSERT OR IGNORE INTO app_countries (app_id, country, added_at)
        SELECT app_id, country, MIN(created_at) FROM keywords GROUP BY app_id, country;

    CREATE TABLE listings (
        app_id            INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        locale            TEXT NOT NULL,
        name              TEXT,
        subtitle          TEXT,
        keywords          TEXT,
        promotional_text  TEXT,
        description       TEXT,
        whats_new         TEXT,
        -- The App Store version the text belongs to, as \"1.2 (PREPARE_FOR_SUBMISSION)\".
        version           TEXT NOT NULL DEFAULT '',
        info_editable     INTEGER NOT NULL DEFAULT 0,
        version_editable  INTEGER NOT NULL DEFAULT 0,
        pulled_at         INTEGER NOT NULL,
        PRIMARY KEY (app_id, locale)
    );

    CREATE TABLE listing_drafts (
        app_id            INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        locale            TEXT NOT NULL,
        name              TEXT,
        subtitle          TEXT,
        keywords          TEXT,
        promotional_text  TEXT,
        description       TEXT,
        whats_new         TEXT,
        note              TEXT NOT NULL DEFAULT '',
        created_at        INTEGER NOT NULL,
        PRIMARY KEY (app_id, locale)
    );",
    // 4: earlier versions of each design and each language of the listing,
    // to go back to. `label` is the user's name for one (`named`) or what
    // happened at the time; empty for an autosave.
    "CREATE TABLE design_versions (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id      INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        target      TEXT NOT NULL,
        created_at  INTEGER NOT NULL,
        label       TEXT NOT NULL DEFAULT '',
        named       INTEGER NOT NULL DEFAULT 0,
        design      BLOB NOT NULL
    );
    CREATE INDEX design_versions_by_target ON design_versions (app_id, target, id DESC);

    CREATE TABLE listing_versions (
        id                INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id            INTEGER NOT NULL REFERENCES apps (id) ON DELETE CASCADE,
        locale            TEXT NOT NULL,
        created_at        INTEGER NOT NULL,
        label             TEXT NOT NULL DEFAULT '',
        named             INTEGER NOT NULL DEFAULT 0,
        name              TEXT,
        subtitle          TEXT,
        keywords          TEXT,
        promotional_text  TEXT,
        description       TEXT,
        whats_new         TEXT
    );
    CREATE INDEX listing_versions_by_locale ON listing_versions (app_id, locale, id DESC);",
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
