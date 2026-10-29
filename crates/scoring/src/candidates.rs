//! Candidate searches an app probably ranks for, from its own metadata.
//!
//! Apple doesn't publish an app's keyword field, so "what does this
//! competitor rank for?" starts from what it says about itself: the phrases
//! in its title, its title words and word pairs, and the phrases its
//! description repeats. Each candidate then gets a real search to see where
//! the app actually ranks.

use std::collections::HashMap;

use crate::tokens;

/// Words that carry no search intent on their own.
const STOP_WORDS: &[&str] = &[
    "a", "about", "all", "an", "and", "any", "app", "apps", "are", "as", "at", "be", "best", "by", "can", "do", "for",
    "free", "from", "get", "has", "have", "how", "in", "into", "is", "it", "its", "just", "lets", "like", "make",
    "more", "most", "new", "no", "not", "now", "of", "on", "one", "or", "our", "out", "over", "so", "than", "that",
    "the", "their", "them", "then", "there", "these", "they", "this", "to", "up", "us", "use", "using", "very", "via",
    "was", "we", "what", "when", "where", "which", "while", "who", "will", "with", "you", "your",
];

fn is_stop(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

/// Useful as a search on its own: not a stop word, not a bare number,
/// at least three characters (unless it's a script without word spacing).
fn is_content(word: &str) -> bool {
    !is_stop(word) && !word.chars().all(|c| c.is_ascii_digit()) && (word.chars().count() >= 3 || !word.is_ascii())
}

/// Up to `limit` distinct candidate searches, most likely first: whole
/// title phrases, then title word pairs and words, then the description's
/// most repeated word pairs and words.
pub fn candidates(title: &str, description: &str, limit: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let push = |candidate: String, out: &mut Vec<String>| {
        if out.len() < limit && !candidate.is_empty() && !out.contains(&candidate) {
            out.push(candidate);
        }
    };

    // "Things 3: To Do List & Planner" → "things 3", "to do list", "planner".
    let segments: Vec<Vec<String>> = title
        .split([':', '-', '–', '—', '|', '&', ',', '+'])
        .map(tokens)
        .filter(|words| !words.is_empty())
        .collect();
    for words in &segments {
        if words.len() <= 4 && words.iter().any(|w| is_content(w)) {
            push(words.join(" "), &mut out);
        }
    }
    // Word pairs within a segment; a pair across ':' or '&' isn't a phrase.
    for words in &segments {
        for pair in words.windows(2) {
            if pair.iter().all(|w| !is_stop(w)) {
                push(pair.join(" "), &mut out);
            }
        }
    }
    for word in segments.iter().flatten() {
        if is_content(word) {
            push(word.clone(), &mut out);
        }
    }

    // Repetition in the description is a strong hint at what the app
    // wants to be found for.
    let mut pairs: HashMap<String, usize> = HashMap::new();
    let mut singles: HashMap<String, usize> = HashMap::new();
    for sentence in description.split(['.', '!', '?', '\n', '•', ';']) {
        let words = tokens(sentence);
        for pair in words.windows(2) {
            if pair.iter().all(|w| is_content(w)) {
                *pairs.entry(pair.join(" ")).or_default() += 1;
            }
        }
        for word in words.iter().filter(|w| is_content(w)) {
            *singles.entry(word.clone()).or_default() += 1;
        }
    }
    let by_count = |map: HashMap<String, usize>, minimum: usize| {
        let mut items: Vec<_> = map.into_iter().filter(|(_, n)| *n >= minimum).collect();
        items.sort_by(|(a, x), (b, y)| y.cmp(x).then_with(|| a.cmp(b)));
        items.into_iter().map(|(text, _)| text).collect::<Vec<_>>()
    };
    for pair in by_count(pairs, 2) {
        push(pair, &mut out);
    }
    for word in by_count(singles, 3) {
        push(word, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_phrases_come_first() {
        let found = candidates("Things 3: To Do List & Planner", "", 20);
        assert_eq!(found, vec!["things 3", "to do list", "planner", "things", "list"]);
        let found = candidates("Habit Tracker - Daily Routine", "", 20);
        assert_eq!(
            found,
            vec!["habit tracker", "daily routine", "habit", "tracker", "daily", "routine"]
        );
    }

    #[test]
    fn description_repetition_counts() {
        let description = "Track every habit. Habit tracker with streaks. \
            The habit tracker that keeps streaks going. Daily streaks, daily habit reminders.";
        let found = candidates("Streaks", description, 20);
        assert_eq!(found[0], "streaks");
        assert!(found.contains(&"habit tracker".to_owned()));
        assert!(found.contains(&"habit".to_owned()));
        assert!(!found.contains(&"keeps".to_owned()), "words said once are noise");
    }

    #[test]
    fn respects_limit_and_dedupes() {
        let found = candidates("Planner: Planner Pro", "planner planner planner", 2);
        assert_eq!(found, vec!["planner", "planner pro"]);
    }

    #[test]
    fn handles_non_latin_titles() {
        let found = candidates("待办事项 - 计划", "", 5);
        assert_eq!(found, vec!["待办事项", "计划"]);
    }
}
