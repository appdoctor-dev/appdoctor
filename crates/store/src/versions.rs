//! Earlier versions of what the user makes, so any of it can be brought back:
//! each design (as its saved file) and each language of the listing.
//!
//! Versions come from autosaves, from the moments before something replaces
//! the work (an agent's edits, a restore), and from the user naming one.
//! Named versions stay until the user deletes them; the rest thin out with
//! age, so a long history stays small.

use rusqlite::{OptionalExtension as _, params};

use crate::{ListingText, Result, Store, text_from_row};

/// A version saved because an agent was about to edit the design.
pub const BEFORE_AGENT: &str = "Before your AI's edits";
/// How long one agent's run of edits counts as one, for [`BEFORE_AGENT`].
const AGENT_RUN: i64 = 600;

/// The newest unnamed versions are all kept; older ones thin out to one a day.
const KEEP_RECENT: usize = 30;
/// The most unnamed versions kept for one design or language.
const KEEP_UNNAMED: usize = 100;

/// One saved state of a design, newest first in lists.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignVersion {
    pub id: i64,
    pub created_at: i64,
    /// The user's name for it or what happened at the time; empty for an autosave.
    pub label: String,
    /// Named by the user, so it's kept until they delete it.
    pub named: bool,
    /// The design file's bytes.
    pub design: Vec<u8>,
}

/// One saved state of a language of the listing.
#[derive(Debug, Clone, PartialEq)]
pub struct ListingVersion {
    pub id: i64,
    pub locale: String,
    pub created_at: i64,
    pub label: String,
    pub named: bool,
    pub text: ListingText,
}

/// Which unnamed versions to delete, given `(id, created_at)` newest first.
fn thinned(rows: &[(i64, i64)]) -> Vec<i64> {
    let (mut kept, mut last_day, mut gone) = (0, None, Vec::new());
    for (index, (id, created_at)) in rows.iter().enumerate() {
        let day = created_at / 86_400;
        if kept < KEEP_UNNAMED && (index < KEEP_RECENT || last_day != Some(day)) {
            kept += 1;
        } else {
            gone.push(*id);
        }
        last_day = Some(day);
    }
    gone
}

impl Store {
    /// Saves a version of a design, unless the newest one is the same. Naming
    /// a version that's the same as an unnamed newest one names that one.
    /// Returns the version's ID, or `None` when nothing was saved.
    pub fn save_design_version(
        &self,
        app_id: u64,
        target: &str,
        design: &[u8],
        label: &str,
        named: bool,
        now: i64,
    ) -> Result<Option<i64>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let newest: Option<(i64, Vec<u8>, bool)> = tx
            .query_row(
                "SELECT id, design, named FROM design_versions
                 WHERE app_id = ?1 AND target = ?2 ORDER BY id DESC LIMIT 1",
                params![app_id as i64, target],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let id = match newest {
            Some((_, same, _)) if same == design && !named => None,
            Some((id, same, false)) if same == design => {
                tx.execute(
                    "UPDATE design_versions SET label = ?2, named = 1 WHERE id = ?1",
                    params![id, label],
                )?;
                Some(id)
            }
            _ => {
                tx.execute(
                    "INSERT INTO design_versions (app_id, target, created_at, label, named, design)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![app_id as i64, target, now, label, named, design],
                )?;
                let id = tx.last_insert_rowid();
                let old = {
                    let mut statement = tx.prepare_cached(
                        "SELECT id, created_at FROM design_versions
                         WHERE app_id = ?1 AND target = ?2 AND named = 0 ORDER BY id DESC",
                    )?;
                    let rows =
                        statement.query_map(params![app_id as i64, target], |row| Ok((row.get(0)?, row.get(1)?)))?;
                    thinned(&rows.collect::<rusqlite::Result<Vec<_>>>()?)
                };
                for gone in old {
                    tx.execute("DELETE FROM design_versions WHERE id = ?1", [gone])?;
                }
                Some(id)
            }
        };
        tx.commit()?;
        Ok(id)
    }

    /// Keeps the design as it is before an agent edits it, once per run of
    /// edits: each of an agent's edits writes the file, and only what was
    /// there before the first is worth going back to.
    pub fn save_design_before_agent(&self, app_id: u64, target: &str, design: &[u8], now: i64) -> Result<()> {
        let newest: Option<(i64, i64, String, Vec<u8>, bool)> = self
            .conn()
            .query_row(
                "SELECT id, created_at, label, design, named FROM design_versions
                 WHERE app_id = ?1 AND target = ?2 ORDER BY id DESC LIMIT 1",
                params![app_id as i64, target],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        match newest {
            Some((_, created_at, label, ..)) if label == BEFORE_AGENT && now - created_at < AGENT_RUN => {}
            // Already saved as an autosave; it was still the design until now.
            Some((id, _, _, same, false)) if same == design => {
                self.conn().execute(
                    "UPDATE design_versions SET label = ?2, created_at = ?3 WHERE id = ?1",
                    params![id, BEFORE_AGENT, now],
                )?;
            }
            Some((.., same, true)) if same == design => {}
            _ => {
                self.save_design_version(app_id, target, design, BEFORE_AGENT, false, now)?;
            }
        }
        Ok(())
    }

    /// A design's versions, newest first.
    pub fn design_versions(&self, app_id: u64, target: &str) -> Result<Vec<DesignVersion>> {
        let conn = self.conn();
        let mut statement = conn.prepare_cached(
            "SELECT id, created_at, label, named, design FROM design_versions
             WHERE app_id = ?1 AND target = ?2 ORDER BY id DESC",
        )?;
        let rows = statement.query_map(params![app_id as i64, target], |row| {
            Ok(DesignVersion {
                id: row.get(0)?,
                created_at: row.get(1)?,
                label: row.get(2)?,
                named: row.get(3)?,
                design: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_design_version(&self, id: i64) -> Result<()> {
        self.conn().execute("DELETE FROM design_versions WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Saves a version of one language of the listing, unless the newest one
    /// is the same. Naming works as in [`Store::save_design_version`].
    pub fn save_listing_version(
        &self,
        app_id: u64,
        locale: &str,
        text: &ListingText,
        label: &str,
        named: bool,
        now: i64,
    ) -> Result<Option<i64>> {
        let newest = self.listing_versions(app_id, locale)?.into_iter().next();
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let id = match newest {
            Some(version) if version.text == *text && !named => None,
            Some(version) if version.text == *text && !version.named => {
                tx.execute(
                    "UPDATE listing_versions SET label = ?2, named = 1 WHERE id = ?1",
                    params![version.id, label],
                )?;
                Some(version.id)
            }
            _ => {
                tx.execute(
                    "INSERT INTO listing_versions (app_id, locale, created_at, label, named, name, subtitle,
                                                   keywords, promotional_text, description, whats_new)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        app_id as i64,
                        locale,
                        now,
                        label,
                        named,
                        text.name,
                        text.subtitle,
                        text.keywords,
                        text.promotional_text,
                        text.description,
                        text.whats_new,
                    ],
                )?;
                let id = tx.last_insert_rowid();
                let old = {
                    let mut statement = tx.prepare_cached(
                        "SELECT id, created_at FROM listing_versions
                         WHERE app_id = ?1 AND locale = ?2 AND named = 0 ORDER BY id DESC",
                    )?;
                    let rows =
                        statement.query_map(params![app_id as i64, locale], |row| Ok((row.get(0)?, row.get(1)?)))?;
                    thinned(&rows.collect::<rusqlite::Result<Vec<_>>>()?)
                };
                for gone in old {
                    tx.execute("DELETE FROM listing_versions WHERE id = ?1", [gone])?;
                }
                Some(id)
            }
        };
        tx.commit()?;
        Ok(id)
    }

    /// One language's versions, newest first.
    pub fn listing_versions(&self, app_id: u64, locale: &str) -> Result<Vec<ListingVersion>> {
        let conn = self.conn();
        let mut statement = conn.prepare_cached(
            "SELECT id, locale, created_at, label, named, name, subtitle, keywords, promotional_text,
                    description, whats_new
             FROM listing_versions WHERE app_id = ?1 AND locale = ?2 ORDER BY id DESC",
        )?;
        let rows = statement.query_map(params![app_id as i64, locale], |row| {
            Ok(ListingVersion {
                id: row.get(0)?,
                locale: row.get(1)?,
                created_at: row.get(2)?,
                label: row.get(3)?,
                named: row.get(4)?,
                text: text_from_row(row, 5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_listing_version(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM listing_versions WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppRecord;

    const APP: u64 = 1;
    const DAY: i64 = 86_400;

    fn store() -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .upsert_app(&AppRecord {
                id: APP,
                name: "App".into(),
                bundle_id: None,
                developer: None,
                icon_url: None,
                country: "us".into(),
                rating_count: 0,
                average_rating: 0.,
                added_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
    }

    fn text(name: &str) -> ListingText {
        ListingText {
            name: Some(name.into()),
            ..ListingText::default()
        }
    }

    fn labels(store: &Store) -> Vec<(String, Vec<u8>)> {
        store
            .design_versions(APP, "iphone")
            .unwrap()
            .into_iter()
            .map(|v| (v.label, v.design))
            .collect()
    }

    #[test]
    fn a_design_version_is_saved_only_when_it_changed() {
        let store = store();
        assert!(
            store
                .save_design_version(APP, "iphone", b"a", "", false, 10)
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .save_design_version(APP, "iphone", b"a", "", false, 20)
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .save_design_version(APP, "iphone", b"b", "", false, 30)
                .unwrap()
                .is_some()
        );
        // Each device has its own history.
        assert!(
            store
                .save_design_version(APP, "ipad", b"b", "", false, 30)
                .unwrap()
                .is_some()
        );

        let versions = store.design_versions(APP, "iphone").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].design, b"b");
        assert_eq!(versions[1].design, b"a");
    }

    #[test]
    fn naming_the_newest_version_doesnt_copy_it() {
        let store = store();
        let id = store.save_design_version(APP, "iphone", b"a", "", false, 10).unwrap();
        assert_eq!(
            store
                .save_design_version(APP, "iphone", b"a", "Launch", true, 20)
                .unwrap(),
            id
        );

        let versions = store.design_versions(APP, "iphone").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].label, "Launch");
        assert!(versions[0].named);

        // A second name for the same design is a version of its own.
        store
            .save_design_version(APP, "iphone", b"a", "Final", true, 30)
            .unwrap();
        assert_eq!(store.design_versions(APP, "iphone").unwrap().len(), 2);
    }

    #[test]
    fn a_run_of_agent_edits_keeps_only_what_came_before_it() {
        let store = store();
        store.save_design_before_agent(APP, "iphone", b"mine", 100).unwrap();
        // The agent's first edit is on disk when its second one saves.
        store
            .save_design_before_agent(APP, "iphone", b"first edit", 150)
            .unwrap();
        assert_eq!(labels(&store), vec![(BEFORE_AGENT.to_owned(), b"mine".to_vec())]);

        // Later, it's a new run.
        store
            .save_design_before_agent(APP, "iphone", b"second edit", 100 + AGENT_RUN)
            .unwrap();
        assert_eq!(store.design_versions(APP, "iphone").unwrap().len(), 2);
    }

    #[test]
    fn an_autosave_of_the_same_design_becomes_the_before_agent_version() {
        let store = store();
        store
            .save_design_version(APP, "iphone", b"mine", "", false, 10)
            .unwrap();
        store.save_design_before_agent(APP, "iphone", b"mine", 5000).unwrap();
        let versions = store.design_versions(APP, "iphone").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(
            (versions[0].label.as_str(), versions[0].created_at),
            (BEFORE_AGENT, 5000)
        );

        // A version the user named keeps its name.
        let store = self::store();
        store
            .save_design_version(APP, "iphone", b"mine", "Launch", true, 10)
            .unwrap();
        store.save_design_before_agent(APP, "iphone", b"mine", 5000).unwrap();
        assert_eq!(labels(&store), vec![("Launch".to_owned(), b"mine".to_vec())]);
    }

    #[test]
    fn old_unnamed_versions_thin_out_to_one_a_day() {
        // Newest first: the recent ones all stay, older days keep their newest.
        let mut rows: Vec<(i64, i64)> = (0..KEEP_RECENT as i64)
            .map(|i| (1000 - i, 100 * DAY + 1000 - i))
            .collect();
        rows.push((500, 99 * DAY + 50));
        rows.push((499, 99 * DAY + 40));
        rows.push((498, 98 * DAY));
        assert_eq!(thinned(&rows), vec![499]);

        // And there's a limit.
        let many: Vec<(i64, i64)> = (0..KEEP_UNNAMED as i64 + 5)
            .map(|i| (1000 - i, (500 - i) * DAY))
            .collect();
        assert_eq!(thinned(&many).len(), 5);
    }

    #[test]
    fn named_versions_survive_thinning() {
        let store = store();
        store
            .save_design_version(APP, "iphone", b"keep", "Launch", true, 0)
            .unwrap();
        for i in 0..(KEEP_UNNAMED + 50) {
            let bytes = format!("v{i}");
            store
                .save_design_version(APP, "iphone", bytes.as_bytes(), "", false, i as i64 * DAY)
                .unwrap();
        }
        let versions = store.design_versions(APP, "iphone").unwrap();
        assert_eq!(versions.iter().filter(|v| !v.named).count(), KEEP_UNNAMED);
        assert!(versions.iter().any(|v| v.label == "Launch"));
    }

    #[test]
    fn listing_versions_are_per_language() {
        let store = store();
        store
            .save_listing_version(APP, "en-US", &text("One"), "Pulled", false, 10)
            .unwrap();
        assert!(
            store
                .save_listing_version(APP, "en-US", &text("One"), "Pulled", false, 20)
                .unwrap()
                .is_none()
        );
        store
            .save_listing_version(APP, "en-US", &text("Two"), "Saved", false, 30)
            .unwrap();
        store
            .save_listing_version(APP, "de-DE", &text("Eins"), "Pulled", false, 30)
            .unwrap();

        let versions = store.listing_versions(APP, "en-US").unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].text.name.as_deref(), Some("Two"));
        assert_eq!(versions[1].label, "Pulled");

        // Named, then deleted.
        let id = store
            .save_listing_version(APP, "en-US", &text("Two"), "Launch", true, 40)
            .unwrap()
            .unwrap();
        assert_eq!(id, versions[0].id);
        store.delete_listing_version(id).unwrap();
        assert_eq!(store.listing_versions(APP, "en-US").unwrap().len(), 1);
    }

    #[test]
    fn deleting_an_app_deletes_its_versions() {
        let store = store();
        store.save_design_version(APP, "iphone", b"a", "", false, 10).unwrap();
        store
            .save_listing_version(APP, "en-US", &text("One"), "", false, 10)
            .unwrap();
        store.delete_app(APP).unwrap();
        assert!(store.design_versions(APP, "iphone").unwrap().is_empty());
        assert!(store.listing_versions(APP, "en-US").unwrap().is_empty());
    }
}
