use url::Url;

use crate::{AppId, Error, Result};

/// An app reference as the user typed it: the ID and, when the URL named a
/// storefront (`/us/`, `/gb/`), that country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRef {
    pub id: AppId,
    pub country: Option<String>,
}

/// Parses an App Store URL, `id123…`, or a bare numeric ID.
///
/// Accepts `apps.apple.com` and legacy `itunes.apple.com` links, with or
/// without a scheme, `itms-apps://` links, and old `viewSoftware?id=` links.
pub fn parse_app_ref(input: &str) -> Result<AppRef> {
    let input = input.trim();
    let invalid = || Error::InvalidAppRef(input.to_owned());

    if let Some(id) = parse_id(input) {
        return Ok(AppRef { id, country: None });
    }

    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| invalid())?;
    let host = url.host_str().ok_or_else(invalid)?;
    if !(host.ends_with("apple.com")) {
        return Err(invalid());
    }

    let segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
    let id = segments
        .iter()
        .rev()
        .find_map(|segment| segment.strip_prefix("id").and_then(parse_digits))
        .or_else(|| {
            url.query_pairs()
                .find(|(key, _)| key == "id")
                .and_then(|(_, value)| parse_digits(&value))
        })
        .ok_or_else(invalid)?;

    let country = segments
        .first()
        .filter(|segment| segment.len() == 2 && segment.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|segment| segment.to_ascii_lowercase());

    Ok(AppRef { id, country })
}

fn parse_id(input: &str) -> Option<AppId> {
    parse_digits(input.strip_prefix("id").unwrap_or(input))
}

fn parse_digits(text: &str) -> Option<AppId> {
    if text.is_empty() || text.len() > 12 || !text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().map(AppId)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(input: &str) -> (u64, Option<&'static str>) {
        let app = parse_app_ref(input).unwrap_or_else(|e| panic!("{input}: {e}"));
        (app.id.0, app.country.map(|c| &*c.leak()))
    }

    #[test]
    fn bare_ids() {
        assert_eq!(parsed("284882215"), (284882215, None));
        assert_eq!(parsed("  id284882215 "), (284882215, None));
    }

    #[test]
    fn app_store_urls() {
        assert_eq!(
            parsed("https://apps.apple.com/us/app/facebook/id284882215"),
            (284882215, Some("us"))
        );
        assert_eq!(
            parsed("https://apps.apple.com/GB/app/things-3/id904237743?l=en&mt=8"),
            (904237743, Some("gb"))
        );
        assert_eq!(parsed("apps.apple.com/app/id904237743"), (904237743, None));
        assert_eq!(
            parsed("https://itunes.apple.com/de/app/name/id904237743?mt=8"),
            (904237743, Some("de"))
        );
        assert_eq!(
            parsed("itms-apps://itunes.apple.com/app/id904237743"),
            (904237743, None)
        );
        assert_eq!(
            parsed("https://itunes.apple.com/WebObjects/MZStore.woa/wa/viewSoftware?id=904237743"),
            (904237743, None)
        );
    }

    #[test]
    fn rejects_other_input() {
        for input in [
            "",
            "hello",
            "id",
            "https://example.com/us/app/x/id123",
            "https://apps.apple.com/us/app/no-id",
            "12345678901234567890",
        ] {
            assert!(parse_app_ref(input).is_err(), "{input:?} should be rejected");
        }
    }
}
