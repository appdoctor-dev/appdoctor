//! Earlier versions of a design and of a listing's languages. A design goes
//! back straight away, as the editor takes in any change to its file. A
//! listing goes back the way every listing edit does: as a suggestion, which
//! the user reviews and saves themselves.

use std::path::Path;

use appdoctor_screenshots::{Design, write_atomic};
use appdoctor_store::locale_name;
use serde_json::{Value, json};

use crate::design::{Args, Doc, Result, design_args, store, text, tool, with};
use crate::listing::{ago, fields_json, now};

pub(crate) fn tools() -> Vec<Value> {
    let version = json!({ "version": { "type": "integer", "description": "The version's id, from list_versions." } });
    vec![
        tool(
            "list_versions",
            "List a design's versions",
            "Earlier versions of a design, newest first. AppDoctor saves them as the user works and before an agent's edits; the user can name them too. Each has an id, when it was saved, a label saying what happened and how many slides it has.",
            design_args(),
            &["app_id"],
            true,
        ),
        tool(
            "save_version",
            "Name this version",
            "Keeps the design as it is now as a named version, which stays until the user deletes it. Use it before a big change the user may want to compare or go back from.",
            with(
                design_args(),
                json!({ "name": { "type": "string", "description": "What to call it: \"Before the dark redesign\"." } }),
            ),
            &["app_id", "name"],
            false,
        ),
        tool(
            "restore_version",
            "Restore a version",
            "Makes an earlier version the design. What's there now is saved as a version first, and the user can undo the change in AppDoctor.",
            with(design_args(), version),
            &["app_id", "version"],
            false,
        ),
        tool(
            "list_listing_versions",
            "List a language's versions",
            "Earlier versions of one language of the App Store listing, newest first: what was pulled from and saved to App Store Connect, edits the user replaced and versions they named. To go back to one, give its fields to suggest_listing, for the user to review and save.",
            json!({
                "app_id": { "type": "integer", "description": "The app's App Store ID, from list_apps." },
                "locale": { "type": "string", "description": "App Store Connect's code: en-US, en-GB, de-DE, ja…" }
            }),
            &["app_id", "locale"],
            true,
        ),
    ]
}

pub(crate) fn call(data_dir: &Path, name: &str, args: &Args) -> Option<Result<Vec<Value>>> {
    Some(match name {
        "list_versions" => list_versions(data_dir, args),
        "save_version" => save_version(data_dir, args),
        "restore_version" => restore_version(data_dir, args),
        "list_listing_versions" => list_listing_versions(data_dir, args),
        _ => return None,
    })
}

fn list_versions(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let doc = Doc::open(data_dir, args)?;
    let versions = store(data_dir)?
        .design_versions(doc.app_id, &doc.version_key())
        .map_err(|error| error.to_string())?;
    let versions: Vec<Value> = versions
        .iter()
        .map(|version| {
            json!({
                "id": version.id,
                "saved": ago(version.created_at),
                "label": if version.label.is_empty() { "Autosaved" } else { version.label.as_str() },
                "named": version.named,
                "slides": Design::from_json(&version.design).map(|design| design.slides.len()),
            })
        })
        .collect();
    Ok(text(json!({ "versions": versions })))
}

fn save_version(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let doc = Doc::open(data_dir, args)?;
    let name = args.opt_str("name").map(str::trim).filter(|name| !name.is_empty());
    let name = name.ok_or("name is required")?;
    let id = store(data_dir)?
        .save_design_version(doc.app_id, &doc.version_key(), &doc.design.to_json(), name, true, now())
        .map_err(|error| error.to_string())?;
    Ok(text(json!({ "saved": name, "id": id })))
}

fn restore_version(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let doc = Doc::open(data_dir, args)?;
    let id = args.u64("version")? as i64;
    let store = store(data_dir)?;
    let key = doc.version_key();
    let versions = store
        .design_versions(doc.app_id, &key)
        .map_err(|error| error.to_string())?;
    let version = versions
        .iter()
        .find(|version| version.id == id)
        .ok_or_else(|| format!("This design has no version {id}. Call list_versions for the ones it has."))?;
    let restored = Design::from_json(&version.design).ok_or("That version can't be read.")?;
    store
        .save_design_version(
            doc.app_id,
            &key,
            &doc.design.to_json(),
            "Before restoring",
            false,
            now(),
        )
        .map_err(|error| error.to_string())?;
    let path = doc.path();
    let design = Design {
        slides: restored.slides,
        video: restored.video,
        ..doc.design
    };
    write_atomic(&path, &design.to_json())
        .map_err(|error| format!("Couldn't save the design: {error}"))?;
    Ok(text(json!({ "restored": id, "slides": design.slides.len() })))
}

fn list_listing_versions(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let app_id = args.u64("app_id")?;
    let locale = args.opt_str("locale").ok_or("locale is required")?;
    let versions = store(data_dir)?
        .listing_versions(app_id, locale)
        .map_err(|error| error.to_string())?;
    let versions: Vec<Value> = versions
        .iter()
        .map(|version| {
            json!({
                "id": version.id,
                "saved": ago(version.created_at),
                "label": if version.label.is_empty() { "Autosaved" } else { version.label.as_str() },
                "named": version.named,
                "fields": fields_json(&version.text),
            })
        })
        .collect();
    Ok(text(json!({ "language": locale_name(locale), "versions": versions })))
}
