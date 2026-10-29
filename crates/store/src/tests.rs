use super::*;

fn app(id: u64) -> AppRecord {
    AppRecord {
        id,
        name: format!("App {id}"),
        bundle_id: Some(format!("com.example.app{id}")),
        developer: Some("Example".into()),
        icon_url: None,
        country: "us".into(),
        rating_count: 10,
        average_rating: 4.5,
        added_at: 100,
        updated_at: 100,
    }
}

fn snapshot(keyword_id: KeywordId, checked_at: i64, rank: Option<u32>) -> Snapshot {
    Snapshot {
        keyword_id,
        checked_at,
        rank,
        results_count: 200,
        difficulty: Some(42.0),
        source: "itunes-search".into(),
        top_apps: (1..=3)
            .map(|id| TopApp {
                id,
                name: format!("App {id}"),
                icon_url: None,
            })
            .collect(),
    }
}

#[test]
fn migrates_to_latest_and_is_idempotent() {
    let dir = std::env::temp_dir().join(format!("appdoctor-store-{}", std::process::id()));
    let path = dir.join("test.db");
    let _ = std::fs::remove_file(&path);
    {
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), migrations::LATEST as i64);
        store.upsert_app(&app(1)).unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), migrations::LATEST as i64);
    assert_eq!(store.apps().unwrap().len(), 1);
    let mode: String = store
        .conn()
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    drop(store);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn upsert_refreshes_details_but_keeps_added_at() {
    let store = Store::open_in_memory().unwrap();
    store.upsert_app(&app(7)).unwrap();
    let mut updated = app(7);
    updated.name = "Renamed".into();
    updated.rating_count = 99;
    updated.added_at = 999;
    updated.updated_at = 200;
    store.upsert_app(&updated).unwrap();
    let stored = store.app(7).unwrap().unwrap();
    assert_eq!(stored.name, "Renamed");
    assert_eq!(stored.rating_count, 99);
    assert_eq!(stored.added_at, 100);
    assert_eq!(stored.updated_at, 200);
    assert!(store.app(8).unwrap().is_none());
}

#[test]
fn keywords_normalize_and_dedupe() {
    let store = Store::open_in_memory().unwrap();
    store.upsert_app(&app(1)).unwrap();
    let a = store.add_keyword(1, "  Habit   Tracker ", "US", 1).unwrap();
    let b = store.add_keyword(1, "habit tracker", "us", 2).unwrap();
    let c = store.add_keyword(1, "habit tracker", "gb", 3).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(matches!(store.add_keyword(1, "   ", "us", 4), Err(Error::EmptyKeyword)));
    let rows = store.keywords(1).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].term, "habit tracker");
    assert_eq!((rows[0].country.as_str(), rows[1].country.as_str()), ("gb", "us"));
    assert_eq!(rows[0].change(), RankChange::Unchecked);
}

#[test]
fn keyword_rows_carry_latest_and_previous_checks() {
    let store = Store::open_in_memory().unwrap();
    store.upsert_app(&app(1)).unwrap();
    let up = store.add_keyword(1, "up", "us", 1).unwrap();
    let down = store.add_keyword(1, "down", "us", 1).unwrap();
    let entered = store.add_keyword(1, "entered", "us", 1).unwrap();
    let dropped = store.add_keyword(1, "dropped", "us", 1).unwrap();
    let once = store.add_keyword(1, "once", "us", 1).unwrap();

    for (id, before, now) in [
        (up, Some(12), Some(4)),
        (down, Some(3), Some(9)),
        (entered, None, Some(150)),
        (dropped, Some(20), None),
    ] {
        store.record_snapshot(&snapshot(id, 10, Some(999))).unwrap();
        store.record_snapshot(&snapshot(id, 20, before)).unwrap();
        store.record_snapshot(&snapshot(id, 30, now)).unwrap();
    }
    store.record_snapshot(&snapshot(once, 30, Some(1))).unwrap();

    let rows = store.keywords(1).unwrap();
    let change = |term: &str| rows.iter().find(|row| row.term == term).unwrap().change();
    assert_eq!(change("up"), RankChange::Up(8));
    assert_eq!(change("down"), RankChange::Down(6));
    assert_eq!(change("entered"), RankChange::Entered);
    assert_eq!(change("dropped"), RankChange::Dropped);
    assert_eq!(change("once"), RankChange::First);

    let up_row = rows.iter().find(|row| row.term == "up").unwrap();
    assert_eq!(up_row.rank(), Some(4));
    assert_eq!(up_row.latest.unwrap().checked_at, 30);
    assert_eq!(up_row.previous.unwrap().checked_at, 20);

    let history = store.rank_history(up, 10).unwrap();
    assert_eq!(
        history.iter().map(|s| s.rank).collect::<Vec<_>>(),
        vec![Some(4), Some(12), Some(999)]
    );
    assert_eq!(
        history[0].top_apps.iter().map(|app| app.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(up_row.top_apps.len(), 3);
    assert_eq!(up_row.latest.unwrap().results_count, 200);
}

#[test]
fn popularity_joins_into_keyword_rows() {
    let store = Store::open_in_memory().unwrap();
    store.upsert_app(&app(1)).unwrap();
    store.add_keyword(1, "habit tracker", "us", 1).unwrap();
    store
        .set_popularity(&Popularity {
            term: "Habit Tracker".into(),
            country: "US".into(),
            popularity: 61,
            source: "apple-ads".into(),
            fetched_at: 5,
        })
        .unwrap();
    assert_eq!(store.keywords(1).unwrap()[0].popularity, Some(61));
    assert_eq!(store.popularity("habit tracker", "us").unwrap().unwrap().popularity, 61);
}

#[test]
fn deleting_an_app_cascades() {
    let store = Store::open_in_memory().unwrap();
    store.upsert_app(&app(1)).unwrap();
    let id = store.add_keyword(1, "habit", "us", 1).unwrap();
    store.record_snapshot(&snapshot(id, 1, Some(3))).unwrap();
    store
        .add_competitor(&Competitor {
            app_id: 1,
            competitor_id: 2,
            name: "Rival".into(),
            icon_url: None,
            added_at: 1,
        })
        .unwrap();
    assert_eq!(store.competitors(1).unwrap().len(), 1);
    store.delete_app(1).unwrap();
    assert!(store.keywords(1).unwrap().is_empty());
    assert!(store.rank_history(id, 10).unwrap().is_empty());
    assert!(store.competitors(1).unwrap().is_empty());
}

#[test]
fn settings_round_trip() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.setting("device_id").unwrap(), None);
    store.set_setting("device_id", "a").unwrap();
    store.set_setting("device_id", "b").unwrap();
    assert_eq!(store.setting("device_id").unwrap().as_deref(), Some("b"));
}
