use std::time::{Duration, UNIX_EPOCH};

use appdoctor_jwt::EcKey;
use serde_json::Value;

use super::*;

fn credentials() -> (Credentials, EcKey) {
    let key = EcKey::generate();
    let credentials = Credentials::new(
        " 57246542-96fe-1a63-e053-0824d011072a ",
        "2X9R4HXF34",
        &key.to_pkcs8_pem().unwrap(),
    )
    .unwrap();
    (credentials, key)
}

#[test]
fn token_has_apple_header_and_claims() {
    let (credentials, key) = credentials();
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let token = credentials.token(now, Duration::from_secs(15 * 60)).unwrap();
    let (header, claims): (Value, Value) = key.public_key().verify(&token).unwrap();
    assert_eq!(header["alg"], "ES256");
    assert_eq!(header["kid"], "2X9R4HXF34");
    assert_eq!(header["typ"], "JWT");
    assert_eq!(claims["iss"], "57246542-96fe-1a63-e053-0824d011072a");
    assert_eq!(claims["aud"], "appstoreconnect-v1");
    assert_eq!(claims["iat"], 1_800_000_000u64);
    assert_eq!(claims["exp"], 1_800_000_000u64 + 900);
}

#[test]
fn token_lifetime_is_capped_at_twenty_minutes() {
    let (credentials, _) = credentials();
    let now = UNIX_EPOCH + Duration::from_secs(1_000);
    let token = credentials.token(now, Duration::from_secs(3600)).unwrap();
    let (_, claims): (Value, Value) = appdoctor_jwt::decode_unverified(&token).unwrap();
    assert_eq!(claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(), 1200);
}

#[test]
fn rejects_bad_p8() {
    assert!(matches!(Credentials::new("i", "k", "not a key"), Err(Error::Key(_))));
}

#[tokio::test]
async fn refuses_review_submissions_without_sending() {
    let (credentials, _) = credentials();
    // An unroutable base: if the guard failed, this would be a network error instead.
    let client = Client::new(credentials).with_base_url("http://127.0.0.1:9");
    for path in [
        "/v1/reviewSubmissions",
        "/v1/reviewSubmissionItems",
        "/v1/appStoreVersionSubmissions",
    ] {
        let result: Result<Value> = client.request(Method::POST, path, None).await;
        assert!(matches!(result, Err(Error::ReviewSubmissionRefused(_))), "{path}");
    }
}

#[test]
fn parses_apps_page() {
    let body = r#"{
      "data": [{"type": "apps", "id": "904237743",
                "attributes": {"name": "Things 3", "bundleId": "com.culturedcode.ThingsiPhone", "sku": "T3", "primaryLocale": "en-US"}}],
      "links": {"self": "https://api.appstoreconnect.apple.com/v1/apps",
                "next": "https://api.appstoreconnect.apple.com/v1/apps?cursor=Mg"}
    }"#;
    let document: Document<Vec<Resource<AppAttributes>>> = serde_json::from_str(body).unwrap();
    assert_eq!(
        document.links.unwrap().next.as_deref(),
        Some("https://api.appstoreconnect.apple.com/v1/apps?cursor=Mg")
    );
    let app = App::from(document.data.into_iter().next().unwrap());
    assert_eq!(app.id, "904237743");
    assert_eq!(app.bundle_id, "com.culturedcode.ThingsiPhone");
    assert_eq!(app.primary_locale.as_deref(), Some("en-US"));
}

#[test]
fn picks_editable_versions_and_infos() {
    let body = r#"{"data": [
      {"type": "appStoreVersions", "id": "live", "attributes": {"platform": "IOS", "versionString": "3.21", "appVersionState": "READY_FOR_DISTRIBUTION"}},
      {"type": "appStoreVersions", "id": "next", "attributes": {"platform": "IOS", "versionString": "3.22", "appStoreState": "PREPARE_FOR_SUBMISSION"}},
      {"type": "appStoreVersions", "id": "mac", "attributes": {"platform": "MAC_OS", "versionString": "3.22", "appVersionState": "PREPARE_FOR_SUBMISSION"}},
      {"type": "appStoreVersions", "id": "vision", "attributes": {"platform": "SOMETHING_NEW"}}
    ]}"#;
    let document: Document<Vec<Resource<AppStoreVersionAttributes>>> = serde_json::from_str(body).unwrap();
    let versions: Vec<AppStoreVersion> = document.data.into_iter().map(Into::into).collect();
    let editable: Vec<_> = versions
        .iter()
        .filter(|v| v.is_editable())
        .map(|v| (v.id.as_str(), v.platform))
        .collect();
    assert_eq!(
        editable,
        vec![("next", Some(Platform::Ios)), ("mac", Some(Platform::MacOs))]
    );
    assert_eq!(versions[3].platform, Some(Platform::Other));

    let info = AppInfo {
        id: "1".into(),
        state: Some("READY_FOR_DISTRIBUTION".into()),
    };
    assert!(!info.is_editable());
    let info = AppInfo {
        id: "2".into(),
        state: Some("PREPARE_FOR_SUBMISSION".into()),
    };
    assert!(info.is_editable());
}

#[test]
fn parses_localizations() {
    let body = r#"{"data": [{"type": "appStoreVersionLocalizations", "id": "loc1",
        "attributes": {"locale": "en-US", "keywords": "todo,tasks,gtd", "promotionalText": null,
                       "description": "Get things done.", "whatsNew": "Bug fixes", "marketingUrl": null}}]}"#;
    let document: Document<Vec<Resource<VersionLocalizationAttributes>>> = serde_json::from_str(body).unwrap();
    let localization = VersionLocalization::from(document.data.into_iter().next().unwrap());
    assert_eq!(localization.locale, "en-US");
    assert_eq!(localization.keywords.as_deref(), Some("todo,tasks,gtd"));
    assert_eq!(localization.promotional_text, None);
}

#[test]
fn updates_serialize_only_set_fields() {
    let update = VersionLocalizationUpdate {
        keywords: Some("todo,tasks".into()),
        promotional_text: Some("New!".into()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(&update).unwrap(),
        serde_json::json!({"keywords": "todo,tasks", "promotionalText": "New!"})
    );
    let update = AppInfoLocalizationUpdate {
        subtitle: Some("To-do lists".into()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(&update).unwrap(),
        serde_json::json!({"subtitle": "To-do lists"})
    );
}

#[test]
fn api_errors_surface_details() {
    let body = r#"{"errors": [{"status": "409", "code": "ENTITY_ERROR.ATTRIBUTE.INVALID",
        "title": "An attribute value is invalid.", "detail": "keywords is too long"}]}"#;
    match api_error(StatusCode::CONFLICT, body) {
        Error::Api(message) => assert_eq!(message, "keywords is too long"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        api_error(StatusCode::BAD_GATEWAY, "<html>"),
        Error::Status(502)
    ));
}
