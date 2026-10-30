//! The App Store listing: the keywords the user tracks (with their ranks,
//! popularity and difficulty in each storefront), the listing text as
//! AppDoctor last pulled it from App Store Connect, and edits for the user
//! to review. An agent never changes the App Store itself: a suggestion
//! waits in AppDoctor's Localization screen until the user saves it with
//! their own App Store Connect key, after a confirmation.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use appdoctor_store::{
    KeywordRow, ListingDraft, ListingText, LOCALES, Store, locale_name, locale_storefront, storefront_locale,
};
use serde_json::{Value, json};

use crate::design::{Args, Result, store, text, tool};

const FIELDS: &[(&str, &str)] = &[
    ("name", "The app's name on the App Store. Searched, and weighs most."),
    ("subtitle", "Under the name. Searched."),
    ("keywords", "Hidden search terms, comma-separated without spaces. Don't repeat words from the name or subtitle."),
    ("promotional_text", "Above the description; can change any time without a new version. Not searched."),
    ("description", "Not searched; written for people deciding to download."),
    ("whats_new", "Release notes for the version being prepared."),
];

pub(crate) fn tools() -> Vec<Value> {
    let app = json!({ "app_id": { "type": "integer", "description": "The app's App Store ID, from list_apps." } });
    let with = |extra: Value| crate::design::with(app.clone(), extra);
    let mut fields = serde_json::Map::new();
    for (key, what) in FIELDS {
        fields.insert((*key).into(), json!({ "type": "string", "description": what }));
    }
    vec![
        tool(
            "get_keywords",
            "Tracked keywords",
            "The keywords the user tracks for an app in each storefront (country), with the app's latest rank in App Store search (null: not in the results), the rank before, Apple's popularity (5–100, higher is searched more), difficulty (0–100, how hard the top 10 is to break into) and the top apps for the search. Plus the app's competitors.",
            with(json!({ "country": { "type": "string", "description": "A storefront's two-letter code (us, gb); every one by default." } })),
            &["app_id"],
            true,
        ),
        tool(
            "get_listing",
            "Read the listing",
            "The app's App Store listing in each language, as AppDoctor last pulled it from App Store Connect: name, subtitle, keywords, promotional text, description and What's New, with their lengths and limits, which tracked keywords the searchable fields cover and miss, and any suggestion waiting for the user. Each storefront shows the listing in its own language (English (U.K.) in the UK) when there is one, else the primary language.",
            with(json!({ "locale": { "type": "string", "description": "One language, as App Store Connect codes it (en-US, en-GB, de-DE); all by default." } })),
            &["app_id"],
            true,
        ),
        tool(
            "suggest_listing",
            "Suggest listing changes",
            "Proposes new text for one language of the listing. It doesn't change App Store Connect: the user sees the suggestion in AppDoctor's Localization screen, reviews it in place and saves it with their App Store Connect key. Give only the fields to change; a later suggestion for the same language adds to it. Limits: name and subtitle 30 characters, keywords 100, promotional text 170, description and What's New 4000. A language the listing doesn't have yet (en-GB, say) can be suggested too; the user adds it before saving. Returns the changes and anything to fix.",
            with(crate::design::with(
                json!({
                    "locale": { "type": "string", "description": "App Store Connect's code: en-US, en-GB, de-DE, ja…" },
                    "note": { "type": "string", "description": "One or two sentences for the user on why: which keywords it targets and what it fixes." }
                }),
                Value::Object(fields),
            )),
            &["app_id", "locale", "note"],
            false,
        ),
    ]
}

pub(crate) fn call(data_dir: &Path, name: &str, args: &Args) -> Option<Result<Vec<Value>>> {
    Some(match name {
        "get_keywords" => get_keywords(data_dir, args),
        "get_listing" => get_listing(data_dir, args),
        "suggest_listing" => suggest_listing(data_dir, args),
        _ => return None,
    })
}

fn app(store: &Store, args: &Args) -> Result<(u64, appdoctor_store::AppRecord)> {
    let id = args.u64("app_id")?;
    let app = store
        .app(id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("AppDoctor has no app {id}. Call list_apps for the apps it tracks."))?;
    Ok((id, app))
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// "3 days ago", for when something was checked or pulled.
fn ago(at: i64) -> String {
    let seconds = (now() - at).max(0);
    match seconds {
        0..120 => "just now".into(),
        120..7200 => format!("{} minutes ago", seconds / 60),
        7200..172_800 => format!("{} hours ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

fn keyword_json(row: &KeywordRow) -> Value {
    json!({
        "keyword": row.term,
        "country": row.country,
        "rank": row.rank(),
        "rank_before": row.previous.and_then(|check| check.rank),
        "popularity": row.popularity,
        "difficulty": row.latest.and_then(|check| check.difficulty).map(|d| d.round()),
        "results": row.latest.map(|check| check.results_count),
        "checked": row.latest.map(|check| ago(check.checked_at)),
        "top_apps": row.top_apps.iter().take(3).map(|app| app.name.clone()).collect::<Vec<_>>(),
    })
}

fn get_keywords(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let store = store(data_dir)?;
    let (id, app) = app(&store, args)?;
    let country = args.opt_str("country").map(str::to_ascii_lowercase);
    let mut rows: Vec<KeywordRow> = store
        .keywords(id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|row| country.as_ref().is_none_or(|c| &row.country == c))
        .collect();
    // Most searched first in each storefront.
    rows.sort_by(|a, b| a.country.cmp(&b.country).then(b.popularity.cmp(&a.popularity)).then(a.term.cmp(&b.term)));
    let competitors = store.competitors(id).map_err(|error| error.to_string())?;
    Ok(text(json!({
        "app": {
            "name": app.name,
            "developer": app.developer,
            "rating": app.average_rating,
            "ratings": app.rating_count,
        },
        "countries": store.app_countries(id).unwrap_or_default(),
        "keywords": rows.iter().map(keyword_json).collect::<Vec<_>>(),
        "competitors": competitors.iter().map(|c| json!({ "app_id": c.competitor_id, "name": c.name })).collect::<Vec<_>>(),
        "note": if rows.is_empty() {
            "No keywords tracked here yet. Suggest some for the user to add in AppDoctor's Keywords screen."
        } else {
            "Ranks come from AppDoctor's last check; the user runs Check ranks for fresh ones."
        },
    })))
}

/// The words App Store search indexes from a listing's searchable fields.
fn words(text: &ListingText) -> BTreeSet<String> {
    [&text.name, &text.subtitle, &text.keywords]
        .into_iter()
        .flatten()
        .flat_map(|field| {
            field
                .split(|c: char| !c.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Tracked keywords whose every word the listing's searchable fields have,
/// and those it misses.
fn coverage(text: &ListingText, keywords: &[&KeywordRow]) -> (Vec<String>, Vec<String>) {
    let indexed = words(text);
    keywords
        .iter()
        .map(|row| row.term.clone())
        .partition(|term| term.split_whitespace().all(|word| indexed.contains(word)))
}

/// Words the keyword field repeats from the name or subtitle: characters
/// that buy nothing.
fn repeated(text: &ListingText) -> Vec<String> {
    let shown = words(&ListingText {
        name: text.name.clone(),
        subtitle: text.subtitle.clone(),
        ..Default::default()
    });
    let mut seen = BTreeSet::new();
    text.keywords
        .iter()
        .flat_map(|field| field.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase).collect::<Vec<_>>())
        .filter(|word| !word.is_empty())
        .filter(|word| shown.contains(word) || !seen.insert(word.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn fields_json(text: &ListingText) -> Value {
    let mut fields = serde_json::Map::new();
    for (key, _, limit, value) in text.fields() {
        fields.insert(
            key.into(),
            json!({ "text": value, "length": value.map_or(0, |v| v.chars().count()), "limit": limit }),
        );
    }
    Value::Object(fields)
}

fn get_listing(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let store = store(data_dir)?;
    let (id, app) = app(&store, args)?;
    let only = args.opt_str("locale");
    let listings = store.listings(id).map_err(|error| error.to_string())?;
    let drafts = store.drafts(id).map_err(|error| error.to_string())?;
    let keywords = store.keywords(id).map_err(|error| error.to_string())?;
    let countries = store.app_countries(id).unwrap_or_default();
    let tracked_in = |locale: &str| -> Vec<&KeywordRow> {
        let country = locale_storefront(locale);
        keywords.iter().filter(|row| Some(row.country.as_str()) == country).collect()
    };
    let languages: Vec<Value> = listings
        .iter()
        .filter(|listing| only.is_none_or(|code| listing.locale == code))
        .map(|listing| {
            let rows = tracked_in(&listing.locale);
            let (covered, missing) = coverage(&listing.text, &rows);
            json!({
                "locale": listing.locale,
                "language": locale_name(&listing.locale),
                "storefront": locale_storefront(&listing.locale),
                "fields": fields_json(&listing.text),
                "tracked_keywords_covered": covered,
                "tracked_keywords_missing": missing,
                "keyword_field_repeats": repeated(&listing.text),
                "suggestion_waiting": drafts.iter().find(|d| d.locale == listing.locale).map(|d| json!({
                    "note": d.note,
                    "fields": fields_json(&d.text),
                })),
            })
        })
        .collect();
    // Storefronts the app is tracked in that have no listing of their own.
    let uncovered: Vec<Value> = countries
        .iter()
        .filter_map(|country| {
            let locale = storefront_locale(country)?;
            (!listings.iter().any(|l| l.locale == locale)).then(|| {
                json!({ "country": country, "locale": locale, "language": locale_name(locale) })
            })
        })
        .collect();
    let first = listings.first();
    Ok(text(json!({
        "app": app.name,
        "pulled": first.map(|l| ago(l.pulled_at)),
        "version": first.map(|l| l.version.clone()),
        "name_and_subtitle_editable": first.map(|l| l.info_editable),
        "version_fields_editable": first.map(|l| l.version_editable),
        "languages": languages,
        "tracked_countries_without_their_language": uncovered,
        "drafts_for_new_languages": drafts
            .iter()
            .filter(|d| !listings.iter().any(|l| l.locale == d.locale))
            .map(|d| json!({ "locale": d.locale, "language": locale_name(&d.locale), "note": d.note, "fields": fields_json(&d.text) }))
            .collect::<Vec<_>>(),
        "note": if listings.is_empty() {
            "AppDoctor hasn't pulled this app's listing yet: the user opens Localization in AppDoctor (with an App Store Connect key in Settings) to pull it. You can still suggest a listing from what you know."
        } else {
            "Name, subtitle and keywords are what App Store search indexes. Suggest edits with suggest_listing."
        },
    })))
}

fn suggest_listing(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let store = store(data_dir)?;
    let (id, _) = app(&store, args)?;
    let locale = args.opt_str("locale").unwrap_or_default().trim().to_owned();
    if !LOCALES.iter().any(|(code, _)| *code == locale) {
        return Err(format!(
            "{locale:?} isn't an App Store Connect language. Use one of: {}.",
            LOCALES.iter().map(|(code, _)| *code).collect::<Vec<_>>().join(", ")
        ));
    }
    let field = |key: &str| args.opt_str(key).map(|value| value.trim().to_owned());
    let mut suggested = ListingText {
        name: field("name"),
        subtitle: field("subtitle"),
        // Spaces after commas spend characters App Store search ignores.
        keywords: field("keywords").map(|k| k.split(',').map(str::trim).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(",")),
        promotional_text: field("promotional_text"),
        description: field("description"),
        whats_new: field("whats_new"),
    };
    if suggested.is_empty() {
        return Err("Give at least one field to change.".into());
    }
    for (_, label, limit, value) in suggested.fields() {
        if let Some(value) = value {
            let length = value.chars().count();
            if length > limit {
                return Err(format!("{label} is {length} characters; the limit is {limit}. Shorten it and suggest again."));
            }
        }
    }
    let current = store
        .listings(id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|l| l.locale == locale);
    // Unchanged fields aren't suggestions.
    if let Some(current) = &current {
        let same = |now: &Option<String>, was: &Option<String>| now.is_some() && now.as_deref().unwrap_or_default() == was.as_deref().unwrap_or_default();
        let was = &current.text;
        let t = &mut suggested;
        for (now, before) in [
            (&mut t.name, &was.name),
            (&mut t.subtitle, &was.subtitle),
            (&mut t.keywords, &was.keywords),
            (&mut t.promotional_text, &was.promotional_text),
            (&mut t.description, &was.description),
            (&mut t.whats_new, &was.whats_new),
        ] {
            if same(now, before) {
                *now = None;
            }
        }
        if suggested.is_empty() {
            return Err("That's what the listing already says; nothing to suggest.".into());
        }
    }
    let draft = store
        .suggest_listing(
            id,
            &ListingDraft {
                locale: locale.clone(),
                text: suggested,
                note: args.opt_str("note").unwrap_or_default().trim().to_owned(),
                created_at: now(),
            },
        )
        .map_err(|error| error.to_string())?;

    let mut notes = Vec::new();
    let whole = current.as_ref().map_or_else(|| draft.text.clone(), |c| c.text.merged(&draft.text));
    let repeats = repeated(&whole);
    if !repeats.is_empty() {
        notes.push(format!(
            "The keyword field repeats words the name, subtitle or itself already has: {}. Those characters could hold new words.",
            repeats.join(", ")
        ));
    }
    match &current {
        None => notes.push(format!(
            "The listing has no {} yet; the user adds the language in Localization before saving.",
            locale_name(&locale)
        )),
        Some(current) => {
            if !current.info_editable && (draft.text.name.is_some() || draft.text.subtitle.is_some()) {
                notes.push("Name and subtitle can't change until the user creates the next version in App Store Connect.".into());
            }
            if !current.version_editable
                && (draft.text.keywords.is_some() || draft.text.description.is_some() || draft.text.whats_new.is_some())
            {
                notes.push("Keywords, description and What's New can't change until the user creates the next version in App Store Connect.".into());
            }
        }
    }
    let changes: Vec<Value> = draft
        .text
        .fields()
        .into_iter()
        .filter_map(|(key, label, limit, value)| {
            let value = value?;
            let before = current.as_ref().and_then(|c| c.text.fields().into_iter().find(|f| f.0 == key)?.3.cloned());
            Some(json!({ "field": label, "before": before, "after": value, "length": value.chars().count(), "limit": limit }))
        })
        .collect();
    Ok(text(json!({
        "locale": locale,
        "language": locale_name(&locale),
        "changes": changes,
        "notes": notes,
        "status": "Waiting for the user in AppDoctor's Localization screen. Nothing has changed on the App Store.",
    })))
}
