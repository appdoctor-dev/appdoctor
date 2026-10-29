use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;

use super::*;

fn credentials() -> (Credentials, EcKey) {
    let key = generate_key();
    let credentials = Credentials::new(
        "SEARCHADS.aeb3ef5f-0c5a-4f2a-99c8-fca83f25a9 ",
        " SEARCHADS.hgw3ef3p-0w7a-8a2n-77c8-scv83f25a7",
        "a273d0d3-4d9e-458c-a173-0db8619ca7d7",
        key.clone(),
    );
    (credentials, key)
}

#[test]
fn client_secret_matches_apples_spec() {
    let (credentials, key) = credentials();
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let secret = credentials.client_secret(now, Duration::from_secs(86_400)).unwrap();
    let (header, claims): (Value, Value) = key.public_key().verify(&secret).unwrap();
    assert_eq!(
        header,
        serde_json::json!({"alg": "ES256", "kid": "a273d0d3-4d9e-458c-a173-0db8619ca7d7"})
    );
    assert_eq!(claims["sub"], "SEARCHADS.aeb3ef5f-0c5a-4f2a-99c8-fca83f25a9");
    assert_eq!(claims["iss"], "SEARCHADS.hgw3ef3p-0w7a-8a2n-77c8-scv83f25a7");
    assert_eq!(claims["aud"], "https://appleid.apple.com");
    assert_eq!(claims["iat"], 1_800_000_000u64);
    assert_eq!(claims["exp"], 1_800_086_400u64);
}

#[test]
fn client_secret_stays_under_180_days() {
    let (credentials, _) = credentials();
    let now = UNIX_EPOCH + Duration::from_secs(1_000);
    let secret = credentials
        .client_secret(now, Duration::from_secs(400 * 86_400))
        .unwrap();
    let (_, claims): (Value, Value) = appdoctor_jwt::decode_unverified(&secret).unwrap();
    let lifetime = claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap();
    assert!(lifetime < 180 * 86_400, "{lifetime}");
}

#[test]
fn public_key_is_the_pem_apple_asks_for() {
    let key = generate_key();
    let pem = key.public_key_pem().unwrap();
    assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----\n"));
    assert!(pem.trim_end().ends_with("-----END PUBLIC KEY-----"));
    // The keychain copy of the private key reloads to the same public key.
    let reloaded = EcKey::from_pem(&key.to_pkcs8_pem().unwrap()).unwrap();
    assert_eq!(reloaded.public_key_pem().unwrap(), pem);
}

#[test]
fn parses_acls() {
    let body = r#"{"result": {"acls": [{"adAccount": {"id": 123456789, "name": "AwayFinder", "orgId": 40669820}, "roles": ["Admin"]}]}}"#;
    let envelope: Envelope<AclList> = serde_json::from_str(body).unwrap();
    let acls = envelope.result.unwrap().acls;
    assert_eq!(acls[0].ad_account.id, 123456789);
    assert_eq!(acls[0].ad_account.org_id, Some(40669820));
    assert_eq!(acls[0].roles, vec!["Admin"]);
}

#[test]
fn phrase_search_body_and_response() {
    let phrases = vec!["best productivity apps".to_owned(), "task management tools".to_owned()];
    let body = phrase_search_query(&phrases);
    assert_eq!(
        body["filters"][0],
        serde_json::json!({"field": "queryType", "operator": "EQUALS", "value": ["SEARCH"]})
    );
    assert_eq!(body["filters"][1]["operator"], "IN");
    assert_eq!(body["filters"][1]["value"][1], "task management tools");

    let response = r#"{"result": [{"phrase": "best productivity apps", "popularity": 82},
                                  {"phrase": "task management tools", "popularity": 75}],
                      "pagination": {"offset": 0, "pageSize": 20, "totalCount": 2}}"#;
    let envelope: Envelope<Vec<PhrasePopularity>> = serde_json::from_str(response).unwrap();
    assert_eq!(
        envelope.result.unwrap()[1],
        PhrasePopularity {
            phrase: "task management tools".into(),
            popularity: 75
        }
    );
}

#[test]
fn keyword_suggestion_body() {
    let body = keyword_suggestions_query(123456, &["us".into(), "gb".into()], &[]);
    let filters = body["filters"].as_array().unwrap();
    assert_eq!(filters.len(), 3);
    assert_eq!(filters[0]["value"], serde_json::json!(["123456"]));
    assert_eq!(
        filters[2],
        serde_json::json!({"field": "countriesOrRegions", "operator": "IN", "value": ["US", "GB"]})
    );
    assert_eq!(body["sorting"][0]["field"], "popularity");
}

#[test]
fn api_errors_carry_details() {
    let body = r#"{"error": {"code": "VALIDATION_ERROR", "message": "Validation errors found",
        "details": [{"code": "INVALID", "message": "phrase is required", "info": {"field": "phrase"}}]}}"#;
    match api_error(StatusCode::BAD_REQUEST, body) {
        Error::Api(message) => assert_eq!(message, "Validation errors found: phrase is required"),
        other => panic!("{other:?}"),
    }
}
