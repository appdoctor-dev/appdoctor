//! Keyword difficulty from the apps that already rank for a keyword.
//!
//! The score answers "how hard is it to break into the top 10 for this
//! search?" on a 0–100 scale. It looks only at public data for the current
//! top results, so it works without any Apple Ads or App Store Connect
//! account:
//!
//! - **Strength**: how established each ranking app is, from its rating count
//!   on a log scale (ratings are the best public proxy for downloads).
//! - **Quality**: its average rating.
//! - **Relevance**: whether it targets the keyword in its title or subtitle.
//!   Apps that rank *without* the keyword in their metadata are ranking on
//!   brand or download velocity, which is hard to beat; apps that rank *with*
//!   it show the keyword is actively contested.
//!
//! Higher positions weigh more, and an empty slot (fewer than ten results)
//! counts as zero, so a sparse search scores as easy.

mod candidates;

pub use candidates::candidates;

/// How many top results the score looks at.
pub const TOP_N: usize = 10;

/// One app in the search results for a keyword, in rank order.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedApp {
    pub title: String,
    pub subtitle: Option<String>,
    pub rating_count: u64,
    pub average_rating: f32,
}

/// A difficulty score and the factors it was built from, for display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Difficulty {
    /// 0 (easy) – 100 (very hard).
    pub score: f32,
    /// Weighted mean strength of the top results, 0–1.
    pub strength: f32,
    /// Weighted mean relevance of the top results, 0–1.
    pub relevance: f32,
    /// How many results the score was computed from (≤ [`TOP_N`]).
    pub results: usize,
}

/// Rating count at which an app counts as fully established. A million
/// ratings puts an app among the largest on the store.
const ESTABLISHED_RATINGS: f64 = 1_000_000.0;

const STRENGTH_WEIGHT: f32 = 0.55;
const QUALITY_WEIGHT: f32 = 0.15;
const RELEVANCE_WEIGHT: f32 = 0.30;

/// Computes difficulty from the results in rank order. Only the first
/// [`TOP_N`] are considered.
pub fn difficulty(keyword: &str, results: &[RankedApp]) -> Difficulty {
    let keyword_tokens = tokens(keyword);
    let mut weighted_score = 0.0f32;
    let mut weighted_strength = 0.0f32;
    let mut weighted_relevance = 0.0f32;
    let mut total_weight = 0.0f32;

    for slot in 0..TOP_N {
        let weight = position_weight(slot);
        total_weight += weight;
        let Some(app) = results.get(slot) else {
            continue;
        };
        let strength = strength(app.rating_count);
        let quality = (app.average_rating / 5.0).clamp(0.0, 1.0);
        let relevance = relevance(&keyword_tokens, app);
        weighted_strength += weight * strength;
        weighted_relevance += weight * relevance;
        weighted_score +=
            weight * (STRENGTH_WEIGHT * strength + QUALITY_WEIGHT * quality + RELEVANCE_WEIGHT * relevance);
    }

    Difficulty {
        score: (100.0 * weighted_score / total_weight).clamp(0.0, 100.0),
        strength: weighted_strength / total_weight,
        relevance: weighted_relevance / total_weight,
        results: results.len().min(TOP_N),
    }
}

/// Rank 1 weighs 1.0, rank 10 about 0.3: the top few slots take most taps.
fn position_weight(slot: usize) -> f32 {
    1.0 / (1.0 + 0.25 * slot as f32)
}

fn strength(rating_count: u64) -> f32 {
    let value = ((rating_count as f64) + 1.0).log10() / (ESTABLISHED_RATINGS + 1.0).log10();
    value.clamp(0.0, 1.0) as f32
}

/// 1.0 for the exact phrase in the title, less for the phrase only in the
/// subtitle or for some of its words spread across both.
fn relevance(keyword_tokens: &[String], app: &RankedApp) -> f32 {
    if keyword_tokens.is_empty() {
        return 0.0;
    }
    let title = tokens(&app.title);
    let subtitle = app.subtitle.as_deref().map(tokens).unwrap_or_default();
    if contains_phrase(&title, keyword_tokens) {
        return 1.0;
    }
    if contains_phrase(&subtitle, keyword_tokens) {
        return 0.85;
    }
    let matched = keyword_tokens
        .iter()
        .filter(|token| title.contains(token) || subtitle.contains(token))
        .count();
    if matched == keyword_tokens.len() {
        0.7
    } else {
        0.5 * matched as f32 / keyword_tokens.len() as f32
    }
}

fn contains_phrase(haystack: &[String], phrase: &[String]) -> bool {
    !phrase.is_empty()
        && haystack.len() >= phrase.len()
        && haystack.windows(phrase.len()).any(|window| window == phrase)
}

/// Lowercased alphanumeric words, folding common Latin diacritics so
/// "café" in a title matches a "cafe" search.
pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.chars().flat_map(fold).collect::<String>())
        .collect()
}

fn fold(c: char) -> impl Iterator<Item = char> {
    let base = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        other => other,
    };
    base.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(title: &str, subtitle: Option<&str>, rating_count: u64, average_rating: f32) -> RankedApp {
        RankedApp {
            title: title.into(),
            subtitle: subtitle.map(Into::into),
            rating_count,
            average_rating,
        }
    }

    #[test]
    fn no_results_is_zero() {
        let d = difficulty("habit tracker", &[]);
        assert_eq!(d.score, 0.0);
        assert_eq!(d.results, 0);
    }

    #[test]
    fn established_exact_matches_are_very_hard() {
        let results: Vec<_> = (0..10)
            .map(|i| app(&format!("Habit Tracker {i}"), None, 2_000_000, 4.8))
            .collect();
        let d = difficulty("habit tracker", &results);
        assert!(d.score > 90.0, "score {}", d.score);
        assert_eq!(d.relevance, 1.0);
    }

    #[test]
    fn small_unrelated_apps_are_easy() {
        let results: Vec<_> = (0..10).map(|_| app("Daily Planner", None, 12, 3.1)).collect();
        let d = difficulty("habit tracker", &results);
        assert!(d.score < 30.0, "score {}", d.score);
        assert_eq!(d.relevance, 0.0);
    }

    #[test]
    fn sparse_results_are_easier_than_full_ones() {
        let one = vec![app("Habit Tracker", None, 50_000, 4.5)];
        let ten = vec![app("Habit Tracker", None, 50_000, 4.5); 10];
        assert!(difficulty("habit tracker", &one).score < difficulty("habit tracker", &ten).score);
    }

    #[test]
    fn more_ratings_never_lowers_difficulty() {
        let mut last = -1.0;
        for count in [0, 10, 1_000, 100_000, 10_000_000] {
            let results = vec![app("Habit Tracker", None, count, 4.5); 10];
            let score = difficulty("habit tracker", &results).score;
            assert!(score >= last, "{count} ratings scored {score} < {last}");
            last = score;
        }
    }

    #[test]
    fn top_positions_weigh_more() {
        let strong = app("Habit Tracker", None, 1_000_000, 4.9);
        let weak = app("Notes", None, 5, 2.0);
        let strong_first: Vec<_> = std::iter::once(strong.clone())
            .chain(std::iter::repeat_n(weak.clone(), 9))
            .collect();
        let strong_last: Vec<_> = std::iter::repeat_n(weak, 9).chain(std::iter::once(strong)).collect();
        assert!(difficulty("habit tracker", &strong_first).score > difficulty("habit tracker", &strong_last).score);
    }

    #[test]
    fn relevance_levels() {
        let kw = tokens("habit tracker");
        assert_eq!(relevance(&kw, &app("Streaks: Habit Tracker", None, 0, 0.0)), 1.0);
        assert_eq!(relevance(&kw, &app("Streaks", Some("The habit tracker"), 0, 0.0)), 0.85);
        assert_eq!(relevance(&kw, &app("Tracker for every Habit", None, 0, 0.0)), 0.7);
        assert_eq!(relevance(&kw, &app("Habit Coach", None, 0, 0.0)), 0.25);
        assert_eq!(relevance(&kw, &app("Calendar", None, 0, 0.0)), 0.0);
    }

    #[test]
    fn tokens_fold_case_punctuation_and_accents() {
        assert_eq!(tokens("Café — Crème-Brûlée!"), vec!["cafe", "creme", "brulee"]);
        assert_eq!(tokens("  "), Vec::<String>::new());
        assert_eq!(tokens("日本語 アプリ"), vec!["日本語", "アプリ"]);
    }

    #[test]
    fn only_top_ten_count() {
        let mut results = vec![app("Notes", None, 5, 2.0); 10];
        let base = difficulty("habit tracker", &results).score;
        results.extend(std::iter::repeat_n(app("Habit Tracker", None, 10_000_000, 5.0), 50));
        let d = difficulty("habit tracker", &results);
        assert_eq!(d.score, base);
        assert_eq!(d.results, 10);
    }
}
