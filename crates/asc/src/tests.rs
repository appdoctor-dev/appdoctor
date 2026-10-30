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

fn plan() -> SubmissionPlan {
    SubmissionPlan {
        app_id: "904237743".into(),
        app_name: "Things 3".into(),
        platform: Platform::Ios,
        version_id: "ver-1".into(),
        version: "3.22".into(),
        version_state: "PREPARE_FOR_SUBMISSION".into(),
        build_id: "build-1".into(),
        build: "3.22 (1234)".into(),
        release_type: ReleaseType::AfterApproval,
        earliest_release_date: None,
        uses_non_exempt_encryption: Some(false),
    }
}

#[tokio::test]
async fn outward_actions_need_confirmation_and_send_nothing_without_it() {
    let (credentials, _) = credentials();
    // An unroutable base: a request that got sent would fail as Http, not NotConfirmed.
    let client = Client::new(credentials).with_base_url("http://127.0.0.1:9");
    assert!(matches!(
        client.submit_for_review(&plan(), false).await,
        Err(Error::NotConfirmed)
    ));
    assert!(matches!(
        client.cancel_submission("sub-1", false).await,
        Err(Error::NotConfirmed)
    ));
    let file = upload::BuildFile {
        path: "/nonexistent/App.ipa".into(),
        platform: Platform::Ios,
        version: "3.22".into(),
        build_number: "1234".into(),
    };
    assert!(matches!(
        client.upload_build("1", &file, false, |_, _| {}).await,
        Err(Error::NotConfirmed)
    ));
    // Confirmed calls do go out (and fail here only because nothing listens).
    assert!(matches!(
        client.cancel_submission("sub-1", true).await,
        Err(Error::Http(_))
    ));
}

#[test]
fn plan_summary_names_everything_the_user_must_see() {
    let summary = plan().summary();
    for needle in [
        "Things 3",
        "904237743",
        "3.22 (PREPARE_FOR_SUBMISSION)",
        "3.22 (1234)",
        "Release automatically when approved",
        "No non-exempt encryption",
    ] {
        assert!(summary.contains(needle), "summary is missing {needle:?}:\n{summary}");
    }
    let mut scheduled = plan();
    scheduled.release_type = ReleaseType::Scheduled;
    scheduled.earliest_release_date = Some("2026-11-20T08:00:00Z".into());
    assert!(scheduled.summary().contains("Release date: 2026-11-20T08:00:00Z"));
}

#[test]
fn submission_request_bodies() {
    use serde_json::json;
    use submission::*;
    assert_eq!(
        create_submission_body("904237743", Platform::Ios),
        json!({"data": {"type": "reviewSubmissions", "attributes": {"platform": "IOS"},
               "relationships": {"app": {"data": {"type": "apps", "id": "904237743"}}}}})
    );
    assert_eq!(
        submission_item_body("sub-1", "ver-1"),
        json!({"data": {"type": "reviewSubmissionItems", "relationships": {
            "reviewSubmission": {"data": {"type": "reviewSubmissions", "id": "sub-1"}},
            "appStoreVersion": {"data": {"type": "appStoreVersions", "id": "ver-1"}}}}})
    );
    assert_eq!(
        submit_body("sub-1"),
        json!({"data": {"type": "reviewSubmissions", "id": "sub-1", "attributes": {"submitted": true}}})
    );
    assert_eq!(
        cancel_body("sub-1"),
        json!({"data": {"type": "reviewSubmissions", "id": "sub-1", "attributes": {"canceled": true}}})
    );
    assert_eq!(
        build_linkage_body("build-1"),
        json!({"data": {"type": "builds", "id": "build-1"}})
    );
    assert_eq!(
        release_type_body("ver-1", ReleaseType::Scheduled, Some("2026-11-20T08:00:00Z")),
        json!({"data": {"type": "appStoreVersions", "id": "ver-1",
               "attributes": {"releaseType": "SCHEDULED", "earliestReleaseDate": "2026-11-20T08:00:00Z"}}})
    );
}

#[test]
fn parses_builds_with_their_versions() {
    let body = r#"{
      "data": [
        {"type": "builds", "id": "b2", "attributes": {"version": "1235", "processingState": "PROCESSING", "expired": false},
         "relationships": {"preReleaseVersion": {"data": {"type": "preReleaseVersions", "id": "p1"}}}},
        {"type": "builds", "id": "b1", "attributes": {"version": "1234", "processingState": "VALID", "expired": false,
          "usesNonExemptEncryption": false, "uploadedDate": "2026-10-20T10:00:00-07:00"},
         "relationships": {"preReleaseVersion": {"data": {"type": "preReleaseVersions", "id": "p1"}}}},
        {"type": "builds", "id": "b0", "attributes": {"version": "1200", "processingState": "VALID", "expired": true}}
      ],
      "included": [{"type": "preReleaseVersions", "id": "p1", "attributes": {"version": "3.22", "platform": "IOS"}}],
      "links": {"next": "https://api.appstoreconnect.apple.com/v1/builds?cursor=x"}
    }"#;
    let (builds, next) = submission::parse_builds(body).unwrap();
    assert!(next.is_some());
    assert_eq!(
        builds.iter().map(|b| b.label()).collect::<Vec<_>>(),
        vec!["3.22 (1235)", "3.22 (1234)", "1200"]
    );
    assert_eq!(
        builds.iter().map(|b| b.is_ready()).collect::<Vec<_>>(),
        vec![false, true, false]
    );
    assert_eq!(builds[1].uses_non_exempt_encryption, Some(false));
    assert_eq!(submission::parse_build(r#"{"data": null}"#).unwrap(), None);
}

#[test]
fn parses_review_submissions_and_items() {
    let body = r#"{"data": {"type": "reviewSubmissions", "id": "sub-1",
        "attributes": {"platform": "IOS", "state": "WAITING_FOR_REVIEW", "submittedDate": "2026-10-29T12:00:00Z"}}}"#;
    let document: Document<Resource<submission::ReviewSubmissionAttributes>> = serde_json::from_str(body).unwrap();
    let submission = ReviewSubmission::from(document.data);
    assert_eq!(submission.state, ReviewSubmissionState::WaitingForReview);
    assert!(submission.state.can_cancel());
    assert!(!ReviewSubmissionState::Complete.can_cancel());

    let items = r#"{"data": [
        {"type": "reviewSubmissionItems", "id": "i1", "attributes": {"state": "READY_FOR_REVIEW"},
         "relationships": {"appStoreVersion": {"data": {"type": "appStoreVersions", "id": "ver-1"}}}},
        {"type": "reviewSubmissionItems", "id": "i2", "relationships": {"appEvent": {"data": {"type": "appEvents", "id": "e1"}}}}
    ]}"#;
    assert_eq!(submission::parse_item_versions(items).unwrap(), vec!["ver-1"]);
}

#[test]
fn build_upload_bodies_and_checksum() {
    use serde_json::json;
    let file = upload::BuildFile {
        path: "Things.ipa".into(),
        platform: Platform::Ios,
        version: "3.22".into(),
        build_number: "1234".into(),
    };
    assert_eq!(
        upload::build_upload_body("904237743", &file)["data"]["attributes"],
        json!({"cfBundleShortVersionString": "3.22", "cfBundleVersion": "1234", "platform": "IOS"})
    );
    assert_eq!(upload::uti_for(std::path::Path::new("A.IPA")).unwrap(), "com.apple.ipa");
    assert_eq!(upload::uti_for(std::path::Path::new("A.pkg")).unwrap(), "com.apple.pkg");
    assert!(upload::uti_for(std::path::Path::new("A.zip")).is_err());
    assert_eq!(
        upload::upload_file_body("u1", "Things.ipa", 42, "com.apple.ipa")["data"]["attributes"],
        json!({"assetType": "ASSET", "fileName": "Things.ipa", "fileSize": 42, "uti": "com.apple.ipa"})
    );

    let path = std::env::temp_dir().join(format!("appdoctor-asc-{}.ipa", std::process::id()));
    std::fs::write(&path, b"abc").unwrap();
    assert_eq!(
        upload::sha256_hex(&path).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let _ = std::fs::remove_file(&path);

    let operations = r#"{"data": {"type": "buildUploadFiles", "id": "f1", "attributes": {"uploadOperations": [
        {"method": "PUT", "url": "https://upload.example/part1", "offset": 0, "length": 10485760,
         "requestHeaders": [{"name": "Content-Type", "value": "application/octet-stream"}]}]}}}"#;
    let document: Document<Resource<upload::BuildUploadFileAttributes>> = serde_json::from_str(operations).unwrap();
    assert_eq!(document.data.attributes.upload_operations()[0].length, 10_485_760);
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
