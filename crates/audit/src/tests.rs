use appdoctor_store::{CheckSummary, KeywordRow, Listing, ListingText};

use super::*;

const DAY: i64 = 86_400;
const NOW: i64 = 20_000 * DAY;

fn app() -> AppRecord {
    AppRecord {
        id: 1,
        name: "Strobia".into(),
        bundle_id: None,
        developer: None,
        icon_url: None,
        country: "us".into(),
        rating_count: 0,
        average_rating: 0.,
        added_at: 0,
        updated_at: 0,
    }
}

fn keyword(term: &str, rank: Option<u32>, popularity: u8) -> KeywordRow {
    KeywordRow {
        id: 1,
        app_id: 1,
        term: term.into(),
        country: "us".into(),
        created_at: 0,
        latest: Some(CheckSummary {
            rank,
            checked_at: NOW - DAY,
            difficulty: Some(40.),
            results_count: 200,
        }),
        previous: None,
        popularity: Some(popularity),
        top_apps: Vec::new(),
    }
}

fn listing(text: ListingText) -> Listing {
    Listing {
        locale: "en-US".into(),
        text,
        version: String::new(),
        info_editable: true,
        version_editable: true,
        pulled_at: NOW,
    }
}

fn live(screenshots: u32, ratings: u64) -> Live {
    Live {
        country: "us".into(),
        name: "Strobia".into(),
        description: Some("A light meditation app. ".repeat(40)),
        release_notes: Some("Bug fixes.".into()),
        released: Some("2024-01-01T00:00:00Z".into()),
        screenshots,
        ipad_screenshots: 0,
        languages: vec!["EN".into(), "DE".into(), "FR".into()],
        content_rating: Some("17+".into()),
        rating_count: ratings,
        average_rating: 4.7,
        fetched_at: NOW,
    }
}

#[test]
fn a_thin_listing_scores_low_with_its_worst_problems_first() {
    let app = app();
    let live = live(2, 0);
    let listings = [listing(ListingText {
        name: Some("Strobia".into()),
        ..Default::default()
    })];
    let keywords = [keyword("sleep sounds", None, 50)];
    let countries = ["us".to_owned()];
    let report = audit(&Inputs {
        app: &app,
        live: Some(&live),
        listings: &listings,
        keywords: &keywords,
        competitors: 0,
        countries: &countries,
        designs: Designs::default(),
        now: NOW,
    });
    assert!(report.score < 50, "score {}", report.score);
    assert_eq!(report.grade, "Needs work");
    assert_eq!(report.findings[0].level, Level::Critical);
    let titles: Vec<&str> = report.findings.iter().map(|f| f.title.as_str()).collect();
    for expected in ["No subtitle", "Keyword field is empty", "Only 2 screenshots on the App Store", "No ratings yet", "Rated 17+"] {
        assert!(titles.contains(&expected), "{expected} missing from {titles:?}");
    }
    assert!(titles.iter().any(|t| t.starts_with("Last update")));
    assert!(titles.iter().any(|t| t.contains("aren't in your listing")));
}

#[test]
fn a_full_listing_scores_well() {
    let app = app();
    let mut live = live(8, 400);
    live.released = Some("2024-09-01T00:00:00Z".into());
    live.content_rating = Some("4+".into());
    live.release_notes = Some("Journeys now come with music you choose, and the light follows its beat.".into());
    live.languages = vec!["EN".into()];
    let listings = [
        listing(ListingText {
            name: Some("Strobia: Light Meditation".into()),
            subtitle: Some("Sleep sounds and calm visuals".into()),
            keywords: Some("relax,focus,breathing,anxiety,stress,mindfulness,insomnia,hypnosis,colour,dream,rest,peace".into()),
            promotional_text: Some("New: music journeys.".into()),
            description: live.description.clone(),
            whats_new: live.release_notes.clone(),
        }),
        Listing { locale: "es-MX".into(), ..listing(ListingText::default()) },
    ];
    let keywords: Vec<KeywordRow> = (0..12)
        .map(|i| keyword(["sleep sounds", "light meditation", "calm"][i % 3], Some(3 + i as u32), 40))
        .collect();
    let countries = ["us".to_owned()];
    let report = audit(&Inputs {
        app: &app,
        live: Some(&live),
        listings: &listings,
        keywords: &keywords,
        competitors: 3,
        countries: &countries,
        designs: Designs {
            iphone_slides: 6,
            captioned_first_three: 3,
            ipad_slides: 0,
            video: true,
            header: true,
        },
        now: NOW,
    });
    let problems: Vec<&str> = report.to_fix().map(|f| f.title.as_str()).collect();
    assert!(report.score >= 85, "score {} with {problems:?}", report.score);
    assert!(!problems.iter().any(|t| t.contains("keyword")), "{problems:?}");
}

#[test]
fn counts_days_from_apples_dates() {
    assert_eq!(days_since("1970-01-11T00:00:00Z", 10 * DAY), Some(0));
    assert_eq!(days_since("2026-10-10T08:58:17Z", 20_736 * DAY), Some(0));
}

#[test]
fn what_it_couldnt_check_doesnt_score_well() {
    let app = app();
    let mut live = live(6, 0);
    live.name = "Meditation & Sleep: Strobia".into();
    live.released = Some("2024-09-20T00:00:00Z".into());
    let keywords = [keyword("meditation", None, 0), keyword("sleep", None, 0)];
    let countries = ["us".to_owned()];
    let report = audit(&Inputs {
        app: &app,
        live: Some(&live),
        listings: &[],
        keywords: &keywords,
        competitors: 0,
        countries: &countries,
        designs: Designs {
            iphone_slides: 5,
            captioned_first_three: 3,
            ipad_slides: 0,
            video: true,
            header: true,
        },
        now: NOW,
    });
    let area = |a: Area| report.areas.iter().find(|s| s.area == a).unwrap().score;
    assert!(area(Area::Listing) <= 70);
    assert!(area(Area::Keywords) <= 60);
    assert!(report.score < 75, "score {}", report.score);
    assert_eq!(report.findings[0].title, "No ratings yet");
}
