//! An App Store optimization audit: a score out of 100 for one app, in five
//! areas, and plain findings, each with what to do about it.
//!
//! It works from what AppDoctor already has: the live product page (the
//! public App Store lookup, [`Live`]), the listing pulled from App Store
//! Connect, the keywords tracked with their ranks and popularity, the
//! designs made in AppDoctor ([`Designs`]), competitors and the storefronts
//! the app is tracked in. What it can't see, it says, rather than guess.
//! Nothing here touches the network; the caller gathers [`Inputs`].

use std::collections::BTreeSet;
use std::path::Path;

use appdoctor_screenshots::{Design, LayerKind, Target};
use appdoctor_store::{AppRecord, KeywordRow, Listing, ListingText, locale_name, locale_storefront, storefront_locale};
use serde::{Deserialize, Serialize};

/// Where the app saves the live product page, per app, in its settings, so
/// agents audit from the same page.
pub fn live_key(app_id: u64) -> String {
    format!("live_listing:{app_id}")
}

/// The live product page, from the public App Store lookup.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Live {
    pub country: String,
    pub name: String,
    pub description: Option<String>,
    pub release_notes: Option<String>,
    /// When the current version came out (RFC 3339).
    pub released: Option<String>,
    pub screenshots: u32,
    pub ipad_screenshots: u32,
    /// Languages the app itself supports (ISO 639-1).
    pub languages: Vec<String>,
    pub content_rating: Option<String>,
    pub rating_count: u64,
    pub average_rating: f32,
    pub fetched_at: i64,
}

/// What the app's AppDoctor designs hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Designs {
    pub iphone_slides: usize,
    /// How many of the first three iPhone slides carry words.
    pub captioned_first_three: usize,
    pub ipad_slides: usize,
    /// An app preview (a design made a video).
    pub video: bool,
    /// Product page header or search art (iOS 27).
    pub header: bool,
}

impl Designs {
    /// Reads the designs in an app's screenshots folder.
    pub fn read(folder: &Path) -> Designs {
        let iphone = Design::load_for(folder, Target::Iphone);
        let ipad = Design::load_for(folder, Target::Ipad);
        let captioned = iphone
            .slides
            .iter()
            .take(3)
            .filter(|slide| {
                slide.layers.iter().any(|layer| {
                    !layer.hidden && matches!(&layer.kind, LayerKind::Text { text, .. } if !text.trim().is_empty())
                })
            })
            .count();
        Designs {
            iphone_slides: iphone.slides.len(),
            captioned_first_three: captioned,
            ipad_slides: ipad.slides.len(),
            video: iphone.video.is_some() || ipad.video.is_some(),
            header: [Target::Header, Target::Search]
                .iter()
                .any(|target| !Design::load_for(folder, *target).slides.is_empty()),
        }
    }
}

/// Everything the audit looks at.
pub struct Inputs<'a> {
    pub app: &'a AppRecord,
    pub live: Option<&'a Live>,
    /// The App Store Connect listing as last pulled; empty when it hasn't been.
    pub listings: &'a [Listing],
    pub keywords: &'a [KeywordRow],
    pub competitors: usize,
    pub countries: &'a [String],
    pub designs: Designs,
    /// Unix seconds.
    pub now: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    Listing,
    Keywords,
    Visuals,
    Ratings,
    Markets,
}

impl Area {
    pub const ALL: [Area; 5] = [Area::Listing, Area::Keywords, Area::Visuals, Area::Ratings, Area::Markets];

    pub fn label(self) -> &'static str {
        match self {
            Area::Listing => "Listing text",
            Area::Keywords => "Keywords",
            Area::Visuals => "Screenshots and video",
            Area::Ratings => "Ratings and updates",
            Area::Markets => "Countries and languages",
        }
    }

    /// Its share of the overall score.
    fn weight(self) -> f32 {
        match self {
            Area::Listing => 0.32,
            Area::Keywords => 0.26,
            Area::Visuals => 0.20,
            Area::Ratings => 0.12,
            Area::Markets => 0.10,
        }
    }
}

/// How much a finding matters, worst first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Critical,
    Warning,
    Tip,
    Good,
}

impl Level {
    fn cost(self) -> u32 {
        match self {
            Level::Critical => 35,
            Level::Warning => 15,
            Level::Tip => 5,
            Level::Good => 0,
        }
    }
}

/// Where in AppDoctor a finding is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    None,
    Localization,
    Keywords,
    Screenshots,
    Video,
    Header,
    Competitors,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub area: Area,
    pub level: Level,
    pub title: String,
    pub detail: String,
    pub fix: Fix,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AreaScore {
    pub area: Area,
    pub label: &'static str,
    pub score: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// 0–100.
    pub score: u8,
    pub grade: &'static str,
    pub areas: Vec<AreaScore>,
    /// Worst first.
    pub findings: Vec<Finding>,
}

impl Report {
    /// The findings to act on, worst first.
    pub fn to_fix(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.level != Level::Good)
    }
}

pub fn grade(score: u8) -> &'static str {
    match score {
        90.. => "Great",
        75..90 => "Good",
        55..75 => "Fair",
        _ => "Needs work",
    }
}

/// The words App Store search indexes from a listing.
pub fn indexed_words(text: &ListingText) -> BTreeSet<String> {
    [&text.name, &text.subtitle, &text.keywords]
        .into_iter()
        .flatten()
        .flat_map(|field| words(field))
        .collect()
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn chars(text: &Option<String>) -> usize {
    text.as_deref().map_or(0, |t| t.trim().chars().count())
}

/// "1 rating", "3 ratings".
fn counted(n: impl Into<u64>, word: &str) -> String {
    let n = n.into();
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn list(items: &[String], limit: usize) -> String {
    let shown: Vec<String> = items.iter().take(limit).map(|item| format!("“{item}”")).collect();
    let more = items.len().saturating_sub(limit);
    if more > 0 { format!("{} and {more} more", shown.join(", ")) } else { shown.join(", ") }
}

/// Days since an RFC 3339 date ("2026-10-10T08:58:17Z").
fn days_since(date: &str, now: i64) -> Option<i64> {
    let (y, rest) = date.split_once('-')?;
    let (m, rest) = rest.split_once('-')?;
    let d = rest.get(..2)?;
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    // Days from the civil calendar (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(now / 86_400 - days)
}

struct Audit<'a> {
    inputs: &'a Inputs<'a>,
    findings: Vec<Finding>,
    /// The most an area can score when too little of it could be checked.
    caps: Vec<(Area, u8)>,
}

impl Audit<'_> {
    fn cap(&mut self, area: Area, most: u8) {
        self.caps.push((area, most));
    }

    fn add(&mut self, area: Area, level: Level, title: impl Into<String>, detail: impl Into<String>, fix: Fix) {
        self.findings.push(Finding {
            area,
            level,
            title: title.into(),
            detail: detail.into(),
            fix,
        });
    }

    /// The listing for the storefront the app was added from, else the first.
    fn home_listing(&self) -> Option<&Listing> {
        let locale = storefront_locale(&self.inputs.app.country);
        self.inputs
            .listings
            .iter()
            .find(|l| Some(l.locale.as_str()) == locale)
            .or(self.inputs.listings.first())
    }

    fn home_keywords(&self) -> Vec<&KeywordRow> {
        self.inputs.keywords.iter().filter(|k| k.country == self.inputs.app.country).collect()
    }

    fn listing(&mut self) {
        use Area::Listing as A;
        let Some(listing) = self.home_listing().cloned() else {
            let name = self.inputs.live.map(|l| l.name.clone()).unwrap_or_else(|| self.inputs.app.name.clone());
            self.name(&name);
            // Only the name and description could be judged.
            self.cap(A, 70);
            self.add(
                A,
                Level::Warning,
                "Subtitle and keyword field not checked",
                "Open Localization with an App Store Connect key in Settings, so AppDoctor can read the subtitle and the hidden keyword field too.",
                Fix::Localization,
            );
            if let Some(live) = self.inputs.live {
                self.description(&live.description);
                self.whats_new(&live.release_notes);
            }
            return;
        };
        let text = &listing.text;
        self.name(text.name.as_deref().unwrap_or(&self.inputs.app.name));

        let subtitle = chars(&text.subtitle);
        let name_words: BTreeSet<String> = text.name.iter().flat_map(|n| words(n)).collect();
        if subtitle == 0 {
            self.add(
                A,
                Level::Critical,
                "No subtitle",
                "The subtitle shows under the name and App Store search reads it. Use its 30 characters for a benefit with keywords the name doesn't have.",
                Fix::Localization,
            );
        } else {
            let repeats: Vec<String> = text
                .subtitle
                .iter()
                .flat_map(|s| words(s))
                .filter(|w| name_words.contains(w))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if !repeats.is_empty() {
                self.add(
                    A,
                    Level::Tip,
                    "Subtitle repeats the name",
                    format!("{} already count from the name; new words in their place would reach more searches.", list(&repeats, 4)),
                    Fix::Localization,
                );
            } else if subtitle < 20 {
                self.add(
                    A,
                    Level::Tip,
                    format!("Subtitle uses {subtitle} of 30 characters"),
                    "There's room for another keyword.",
                    Fix::Localization,
                );
            } else {
                self.add(A, Level::Good, "Subtitle is well used", format!("{subtitle} of 30 characters, no repeats."), Fix::None);
            }
        }

        let keywords = chars(&text.keywords);
        if keywords == 0 {
            self.add(
                A,
                Level::Critical,
                "Keyword field is empty",
                "These 100 hidden characters are searched like the name and subtitle. Fill them with words people search for, separated by commas.",
                Fix::Localization,
            );
        } else {
            let field = text.keywords.clone().unwrap_or_default();
            if keywords < 80 {
                self.add(
                    A,
                    Level::Warning,
                    format!("Keyword field uses {keywords} of 100 characters"),
                    "Every unused character is a search you could appear in.",
                    Fix::Localization,
                );
            }
            if field.contains(", ") || field.contains(" ,") {
                self.add(
                    A,
                    Level::Tip,
                    "Spaces in the keyword field",
                    "Spaces after commas use characters search doesn't need. Separate keywords with commas only.",
                    Fix::Localization,
                );
            }
            let shown: BTreeSet<String> = name_words.iter().cloned().chain(text.subtitle.iter().flat_map(|s| words(s))).collect();
            let mut seen = BTreeSet::new();
            let wasted: Vec<String> = words(&field)
                .into_iter()
                .filter(|w| shown.contains(w) || !seen.insert(w.clone()))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if !wasted.is_empty() {
                self.add(
                    A,
                    Level::Warning,
                    "Keyword field repeats words",
                    format!("{} are already in the name, subtitle or the field itself. Apple counts each word once, so swap them for new ones.", list(&wasted, 5)),
                    Fix::Localization,
                );
            }
            let filler: Vec<String> = words(&field)
                .into_iter()
                .filter(|w| matches!(w.as_str(), "app" | "apps" | "free" | "iphone" | "ipad" | "best"))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if !filler.is_empty() {
                self.add(
                    A,
                    Level::Tip,
                    "Words that don't help in the keyword field",
                    format!("{} rarely win searches on their own; their characters are worth more as real keywords.", list(&filler, 4)),
                    Fix::Localization,
                );
            }
            if keywords >= 90 && wasted.is_empty() {
                self.add(A, Level::Good, "Keyword field is full", format!("{keywords} of 100 characters, no repeats."), Fix::None);
            }
        }

        if chars(&text.promotional_text) == 0 {
            self.add(
                A,
                Level::Tip,
                "No promotional text",
                "The 170 characters above the description can change any time without a new version: good for news, offers and seasons.",
                Fix::Localization,
            );
        }
        let description = text.description.clone().or_else(|| self.inputs.live.and_then(|l| l.description.clone()));
        self.description(&description);
        let notes = text.whats_new.clone().or_else(|| self.inputs.live.and_then(|l| l.release_notes.clone()));
        self.whats_new(&notes);
    }

    fn name(&mut self, name: &str) {
        use Area::Listing as A;
        let length = name.chars().count();
        if length < 15 {
            self.add(
                A,
                Level::Warning,
                format!("Name uses {length} of 30 characters"),
                "The name weighs most in search. Add your strongest keyword after the brand, as in “Brand: Keyword Phrase”.",
                Fix::Localization,
            );
        } else if length < 24 {
            self.add(
                A,
                Level::Tip,
                format!("Name uses {length} of 30 characters"),
                "There may be room for one more keyword.",
                Fix::Localization,
            );
        } else {
            self.add(A, Level::Good, "Name is well used", format!("{length} of 30 characters."), Fix::None);
        }
        let tracked = self.home_keywords();
        if !tracked.is_empty() {
            let name_words: BTreeSet<String> = words(name).into_iter().collect();
            let any = tracked.iter().any(|k| k.term.split_whitespace().any(|w| name_words.contains(w)));
            if !any {
                self.add(
                    A,
                    Level::Warning,
                    "Name has none of your tracked keywords",
                    "Words in the name rank best. Work your most valuable keyword into it.",
                    Fix::Localization,
                );
            }
        }
    }

    fn description(&mut self, description: &Option<String>) {
        let length = chars(description);
        if length == 0 {
            return;
        }
        if length < 500 {
            self.add(
                Area::Listing,
                Level::Warning,
                "Short description",
                format!("{length} characters. People who tap in want to know what they get: features, how it works, why it's different."),
                Fix::Localization,
            );
        } else {
            self.add(Area::Listing, Level::Good, "Description has substance", format!("{length} characters."), Fix::None);
        }
    }

    fn whats_new(&mut self, notes: &Option<String>) {
        let Some(notes) = notes.as_deref().map(str::trim).filter(|n| !n.is_empty()) else {
            return;
        };
        let lower = notes.to_lowercase();
        if notes.chars().count() < 80 && (lower.contains("bug") || lower.contains("performance") || lower.contains("improvement")) {
            self.add(
                Area::Listing,
                Level::Tip,
                "What's New says little",
                "“Bug fixes and improvements” tells people nothing. Name one thing that's better for them.",
                Fix::Localization,
            );
        }
    }

    fn keywords(&mut self) {
        use Area::Keywords as A;
        let all = self.inputs.keywords;
        if all.is_empty() {
            // Nothing to judge, which is worse than a few keywords.
            self.cap(A, 30);
            self.add(
                A,
                Level::Critical,
                "No keywords tracked",
                "Track the searches you want to be found for, so AppDoctor can check where you rank and find what's missing.",
                Fix::Keywords,
            );
            return;
        }
        if all.len() < 5 {
            // Too few to know where the app stands.
            self.cap(A, 60);
        }
        if all.len() < 10 {
            self.add(
                A,
                Level::Warning,
                format!("Only {} tracked", counted(all.len() as u64, "keyword")),
                "Track 15–30 across popular and easier searches to see the whole picture.",
                Fix::Keywords,
            );
        } else {
            self.add(A, Level::Good, format!("{} keywords tracked", all.len()), "A good spread to learn from.", Fix::None);
        }
        let unchecked = all.iter().filter(|k| k.latest.is_none()).count();
        if unchecked > 0 {
            self.add(
                A,
                Level::Warning,
                format!("{} never checked", counted(unchecked as u64, "keyword")),
                "Run Check ranks to see where you appear for them.",
                Fix::Keywords,
            );
        }
        let newest = all.iter().filter_map(|k| k.latest.map(|c| c.checked_at)).max();
        if let Some(newest) = newest
            && self.inputs.now - newest > 7 * 86_400
        {
            self.add(
                A,
                Level::Tip,
                format!("Ranks are {} days old", (self.inputs.now - newest) / 86_400),
                "Check ranks weekly to catch changes early.",
                Fix::Keywords,
            );
        }

        // Coverage: tracked keywords the listing can't rank for.
        if let Some(listing) = self.home_listing() {
            let indexed = indexed_words(&listing.text);
            let country = locale_storefront(&listing.locale).unwrap_or(&self.inputs.app.country).to_owned();
            let mut missing: Vec<&KeywordRow> = all
                .iter()
                .filter(|k| k.country == country)
                .filter(|k| !k.term.split_whitespace().all(|w| indexed.contains(w)))
                .collect();
            missing.sort_by_key(|k| std::cmp::Reverse(k.popularity));
            if missing.is_empty() {
                self.add(
                    A,
                    Level::Good,
                    "Your listing covers every tracked keyword",
                    format!("Each word is in the {} name, subtitle or keyword field.", locale_name(&listing.locale)),
                    Fix::None,
                );
            } else {
                let terms: Vec<String> = missing.iter().map(|k| k.term.clone()).collect();
                self.add(
                    A,
                    Level::Warning,
                    format!("{} tracked keywords aren't in your listing", missing.len()),
                    format!("Search can't match {} because not all their words are in the name, subtitle or keyword field.", list(&terms, 4)),
                    Fix::Localization,
                );
            }
        }

        let checked: Vec<&KeywordRow> = all.iter().filter(|k| k.latest.is_some()).collect();
        let top10 = checked.iter().filter(|k| k.rank().is_some_and(|r| r <= 10)).count();
        if checked.len() >= 3 {
            if top10 == 0 {
                self.add(
                    A,
                    Level::Warning,
                    "No keyword in the top 10",
                    "Most downloads from search go to the first few results. Aim for easier, more specific phrases you can win, then build up.",
                    Fix::Keywords,
                );
            } else {
                self.add(
                    A,
                    Level::Good,
                    format!("{top10} of {} checked keywords in the top 10", checked.len()),
                    "Where most search downloads happen.",
                    Fix::None,
                );
            }
        }
        let mut popular_unranked: Vec<&KeywordRow> = checked
            .iter()
            .copied()
            .filter(|k| k.rank().is_none() && k.popularity.is_some_and(|p| p >= 30))
            .collect();
        popular_unranked.sort_by_key(|k| std::cmp::Reverse(k.popularity));
        if !popular_unranked.is_empty() {
            let terms: Vec<String> = popular_unranked.iter().map(|k| k.term.clone()).collect();
            self.add(
                A,
                Level::Tip,
                "Popular searches you don't appear in",
                format!("{}: worth working into the listing if they fit the app.", list(&terms, 4)),
                Fix::Localization,
            );
        }
        let hard: Vec<String> = checked
            .iter()
            .filter(|k| {
                k.latest.and_then(|c| c.difficulty).is_some_and(|d| d >= 75.) && k.rank().is_none_or(|r| r > 50)
            })
            .map(|k| k.term.clone())
            .collect();
        if !hard.is_empty() {
            self.add(
                A,
                Level::Tip,
                "Very hard keywords",
                format!("{} are dominated by big apps. Longer phrases around them are easier to rank for.", list(&hard, 4)),
                Fix::Keywords,
            );
        }
        if all.iter().all(|k| k.popularity.is_none()) {
            self.add(
                A,
                Level::Tip,
                "No popularity data",
                "Connect Apple Ads in Settings (it's free) to see how often each keyword is searched.",
                Fix::Settings,
            );
        }
    }

    fn visuals(&mut self) {
        use Area::Visuals as A;
        let designs = self.inputs.designs;
        match self.inputs.live.map(|l| l.screenshots) {
            Some(0) => self.add(
                A,
                Level::Critical,
                "No iPhone screenshots on the App Store",
                "Screenshots decide most downloads. Design them in Screenshots.",
                Fix::Screenshots,
            ),
            Some(count) if count < 4 => self.add(
                A,
                Level::Warning,
                format!("Only {} on the App Store", counted(count, "screenshot")),
                "Apple allows 10. Use at least 5 to show what the app does.",
                Fix::Screenshots,
            ),
            Some(count) if count < 6 => self.add(
                A,
                Level::Tip,
                format!("{count} screenshots on the App Store"),
                "Apple allows 10; a few more can show the features people look for.",
                Fix::Screenshots,
            ),
            Some(count) => self.add(A, Level::Good, format!("{count} screenshots on the App Store"), "Plenty to tell the story.", Fix::None),
            None if designs.iphone_slides == 0 => self.add(
                A,
                Level::Warning,
                "No screenshots designed",
                "AppDoctor couldn't read the live page and has no screenshot design for this app yet.",
                Fix::Screenshots,
            ),
            None => {}
        }
        if designs.iphone_slides > 0 && designs.captioned_first_three < designs.iphone_slides.min(3) {
            self.add(
                A,
                Level::Tip,
                "Some of the first three screenshots have no headline",
                "Search results show the first three. A short benefit headline on each sells the app before anyone taps.",
                Fix::Screenshots,
            );
        }
        if designs.video {
            self.add(A, Level::Good, "App preview designed", "Previews autoplay in search results and on the page.", Fix::None);
        } else {
            self.add(
                A,
                Level::Tip,
                "No app preview video",
                "A 15–30 second preview autoplays in search. Make one in Screenshots with Make a video, or ask the AI.",
                Fix::Video,
            );
        }
        if !designs.header {
            self.add(
                A,
                Level::Tip,
                "No product page header or search art",
                "iOS 27 can show a wide header on your page and art in search. Design them under Screenshots.",
                Fix::Header,
            );
        }
    }

    fn ratings(&mut self) {
        use Area::Ratings as A;
        let (count, average) = match self.inputs.live {
            Some(live) => (live.rating_count, live.average_rating),
            None => (self.inputs.app.rating_count, self.inputs.app.average_rating),
        };
        if count == 0 {
            self.add(
                A,
                Level::Critical,
                "No ratings yet",
                "Ratings lift ranking and trust. Ask at a happy moment with Apple's review prompt (SKStoreReviewController).",
                Fix::None,
            );
        } else if count < 50 {
            self.add(
                A,
                Level::Tip,
                format!("Only {}", counted(count, "rating")),
                "A few more from happy users go a long way; prompt after a success, not at launch.",
                Fix::None,
            );
        } else {
            self.add(A, Level::Good, format!("{count} ratings"), format!("Averaging {average:.1}."), Fix::None);
        }
        if count >= 10 && average < 4.0 {
            self.add(
                A,
                Level::Warning,
                format!("Average rating {average:.1}"),
                "Below 4 stars puts people off. Read the low reviews for what to fix, and reply to them.",
                Fix::None,
            );
        }
        if let Some(days) = self.inputs.live.and_then(|l| l.released.as_deref()).and_then(|d| days_since(d, self.inputs.now)) {
            if days > 120 {
                self.add(
                    A,
                    Level::Tip,
                    format!("Last update {days} days ago"),
                    "Regular updates show the app is cared for and give you a fresh What's New.",
                    Fix::None,
                );
            } else {
                self.add(A, Level::Good, "Updated recently", format!("{days} days ago."), Fix::None);
            }
        }
        if let Some(rating) = self.inputs.live.and_then(|l| l.content_rating.as_deref())
            && rating == "17+"
        {
            self.add(
                A,
                Level::Tip,
                "Rated 17+",
                "Devices with age limits won't show the app. If nothing in it needs 17+, review the age rating answers in App Store Connect.",
                Fix::None,
            );
        }
    }

    fn markets(&mut self) {
        use Area::Markets as A;
        let languages: BTreeSet<&str> = self.inputs.listings.iter().map(|l| l.locale.as_str()).collect();
        if !self.inputs.listings.is_empty() {
            let without: Vec<String> = self
                .inputs
                .countries
                .iter()
                .filter_map(|country| storefront_locale(country))
                .filter(|locale| !languages.contains(locale))
                .map(locale_name)
                .collect();
            if !without.is_empty() {
                self.add(
                    A,
                    Level::Warning,
                    "Countries you track have no listing in their language",
                    format!("Add {}: each is its own name, subtitle and keyword field for that storefront.", list(&without, 3)),
                    Fix::Localization,
                );
            }
            if languages.len() == 1 {
                self.add(
                    A,
                    Level::Tip,
                    "One language in the listing",
                    "Every language you add is another listing with its own keywords for the countries that read it.",
                    Fix::Localization,
                );
            } else {
                self.add(A, Level::Good, format!("{} languages in the listing", languages.len()), "", Fix::None);
            }
            if self.inputs.countries.iter().any(|c| c == "us") && !languages.contains("es-MX") {
                self.add(
                    A,
                    Level::Tip,
                    "Spanish (Mexico) for the US",
                    "Apple's US App Store also reads the Spanish (Mexico) listing, so it adds keyword room there too.",
                    Fix::Localization,
                );
            }
        }
        if let Some(live) = self.inputs.live
            && live.languages.len() > languages.len().max(1)
        {
            self.add(
                A,
                Level::Tip,
                format!("The app speaks {} languages", live.languages.len()),
                format!(
                    "The listing has {}. Translating it lets people in those countries find and understand the app.",
                    languages.len().max(1)
                ),
                Fix::Localization,
            );
        }
        if self.inputs.countries.len() == 1 {
            self.add(
                A,
                Level::Tip,
                "Tracked in one country",
                "Add the app from another country (the UK, Canada, Australia) to see where else it can grow.",
                Fix::None,
            );
        }
        if self.inputs.competitors == 0 {
            self.add(
                A,
                Level::Tip,
                "No competitors tracked",
                "Track a few rivals to compare ranks and see the keywords they win.",
                Fix::Competitors,
            );
        }
    }
}

/// Audits the app.
pub fn audit(inputs: &Inputs) -> Report {
    let mut audit = Audit {
        inputs,
        findings: Vec::new(),
        caps: Vec::new(),
    };
    audit.listing();
    audit.keywords();
    audit.visuals();
    audit.ratings();
    audit.markets();
    let caps = audit.caps;
    let mut findings = audit.findings;
    findings.sort_by_key(|f| (f.level, f.area));

    let areas: Vec<AreaScore> = Area::ALL
        .iter()
        .map(|area| {
            let lost: u32 = findings.iter().filter(|f| f.area == *area).map(|f| f.level.cost()).sum();
            let most = caps.iter().filter(|(a, _)| a == area).map(|(_, most)| *most).min().unwrap_or(100);
            AreaScore {
                area: *area,
                label: area.label(),
                score: (100u32.saturating_sub(lost) as u8).min(most),
            }
        })
        .collect();
    let score = areas.iter().map(|a| a.score as f32 * a.area.weight()).sum::<f32>().round() as u8;
    Report {
        score,
        grade: grade(score),
        areas,
        findings,
    }
}

#[cfg(test)]
mod tests;
