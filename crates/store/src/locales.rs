//! The languages an App Store listing can have, and which storefront each
//! one is written for.

/// App Store Connect's metadata locales, with their names.
pub const LOCALES: &[(&str, &str)] = &[
    ("ar-SA", "Arabic"),
    ("ca", "Catalan"),
    ("zh-Hans", "Chinese (Simplified)"),
    ("zh-Hant", "Chinese (Traditional)"),
    ("hr", "Croatian"),
    ("cs", "Czech"),
    ("da", "Danish"),
    ("nl-NL", "Dutch"),
    ("en-AU", "English (Australia)"),
    ("en-CA", "English (Canada)"),
    ("en-GB", "English (U.K.)"),
    ("en-US", "English (U.S.)"),
    ("fi", "Finnish"),
    ("fr-FR", "French"),
    ("fr-CA", "French (Canada)"),
    ("de-DE", "German"),
    ("el", "Greek"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ms", "Malay"),
    ("no", "Norwegian"),
    ("pl", "Polish"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sk", "Slovak"),
    ("es-MX", "Spanish (Mexico)"),
    ("es-ES", "Spanish (Spain)"),
    ("sv", "Swedish"),
    ("th", "Thai"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("vi", "Vietnamese"),
];

/// A locale's name ("English (U.K.)"), or the code itself.
pub fn locale_name(code: &str) -> String {
    LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| (*name).to_owned())
        .unwrap_or_else(|| code.to_owned())
}

/// The storefront (country code) a locale is mainly read in, for matching
/// a listing with the keywords tracked there.
pub fn locale_storefront(code: &str) -> Option<&'static str> {
    Some(match code {
        "ar-SA" => "sa",
        "ca" | "es-ES" => "es",
        "zh-Hans" => "cn",
        "zh-Hant" => "tw",
        "hr" => "hr",
        "cs" => "cz",
        "da" => "dk",
        "nl-NL" => "nl",
        "en-AU" => "au",
        "en-CA" | "fr-CA" => "ca",
        "en-GB" => "gb",
        "en-US" => "us",
        "fi" => "fi",
        "fr-FR" => "fr",
        "de-DE" => "de",
        "el" => "gr",
        "he" => "il",
        "hi" => "in",
        "hu" => "hu",
        "id" => "id",
        "it" => "it",
        "ja" => "jp",
        "ko" => "kr",
        "ms" => "my",
        "no" => "no",
        "pl" => "pl",
        "pt-BR" => "br",
        "pt-PT" => "pt",
        "ro" => "ro",
        "ru" => "ru",
        "sk" => "sk",
        "es-MX" => "mx",
        "sv" => "se",
        "th" => "th",
        "tr" => "tr",
        "uk" => "ua",
        "vi" => "vn",
        _ => return None,
    })
}

/// The locale written for a storefront, when there's one obvious one.
pub fn storefront_locale(country: &str) -> Option<&'static str> {
    LOCALES
        .iter()
        .map(|(code, _)| *code)
        .find(|code| locale_storefront(code) == Some(country) && !matches!(*code, "ca" | "fr-CA"))
}
