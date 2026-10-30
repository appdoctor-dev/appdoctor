//! Entitlement tokens: `base64url(JSON claims) "." base64url(Ed25519
//! signature over the first part)`, exactly as `apps/api` signs them.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Claims {
    pub v: u32,
    /// The user's account ID.
    pub sub: String,
    pub plan: String,
    /// The device ID the token was issued to.
    pub device: String,
    /// Issued at, Unix seconds.
    pub iat: i64,
    /// Expires at, Unix seconds.
    pub exp: i64,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TokenError {
    #[error("the app's licence public key isn't set")]
    NoPublicKey,
    #[error("malformed licence")]
    Malformed,
    #[error("licence signature doesn't verify")]
    BadSignature,
    #[error("unsupported licence version {0}")]
    UnsupportedVersion(u32),
    #[error("licence was issued to another device")]
    WrongDevice,
    #[error("licence expired")]
    Expired { plan: String },
}

/// Parses the public key: base64url (or base64) of the raw 32 bytes, 64 hex
/// digits, or an SPKI PEM.
pub fn parse_public_key(text: &str) -> Result<VerifyingKey, TokenError> {
    let text = text.trim();
    let bytes = if text.starts_with("-----BEGIN PUBLIC KEY-----") {
        let body: String = text.lines().filter(|line| !line.starts_with("-----")).collect();
        let der = STANDARD.decode(body).map_err(|_| TokenError::NoPublicKey)?;
        // Ed25519 SPKI is a fixed 12-byte prefix followed by the raw key.
        der.get(12..).map(<[u8]>::to_vec).ok_or(TokenError::NoPublicKey)?
    } else if text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit()) {
        (0..32)
            .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16))
            .collect::<Result<_, _>>()
            .map_err(|_| TokenError::NoPublicKey)?
    } else {
        URL_SAFE_NO_PAD
            .decode(text.trim_end_matches('='))
            .or_else(|_| STANDARD.decode(text))
            .map_err(|_| TokenError::NoPublicKey)?
    };
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| TokenError::NoPublicKey)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| TokenError::NoPublicKey)
}

/// Verifies a token's signature, version, device and expiry.
pub fn verify(token: &str, key: &VerifyingKey, device_id: &str, now: i64) -> Result<Claims, TokenError> {
    let (body, signature) = token.trim().split_once('.').ok_or(TokenError::Malformed)?;
    let signature = URL_SAFE_NO_PAD.decode(signature).map_err(|_| TokenError::Malformed)?;
    let signature = Signature::from_slice(&signature).map_err(|_| TokenError::Malformed)?;
    key.verify_strict(body.as_bytes(), &signature)
        .map_err(|_| TokenError::BadSignature)?;

    let claims = URL_SAFE_NO_PAD.decode(body).map_err(|_| TokenError::Malformed)?;
    let claims: Claims = serde_json::from_slice(&claims).map_err(|_| TokenError::Malformed)?;
    if claims.v != 1 {
        return Err(TokenError::UnsupportedVersion(claims.v));
    }
    if claims.device != device_id {
        return Err(TokenError::WrongDevice);
    }
    if claims.exp <= now {
        return Err(TokenError::Expired { plan: claims.plan });
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};
    use serde_json::json;

    use super::*;

    const DEVICE: &str = "6f1c1e0a-7d1b-4c5e-9a43-1c2f7b1e9d10";

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    /// Mints a token the way `apps/api/src/lib/entitlement.ts` does.
    fn mint(key: &SigningKey, claims: serde_json::Value) -> String {
        let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let signature = key.sign(body.as_bytes());
        format!("{body}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }

    fn claims(exp: i64) -> serde_json::Value {
        json!({"v": 1, "sub": "user_1", "plan": "pro", "device": DEVICE, "iat": 1_000, "exp": exp})
    }

    #[test]
    fn verifies_a_valid_token() {
        let key = signing_key();
        let token = mint(&key, claims(5_000));
        let verified = verify(&token, &key.verifying_key(), DEVICE, 2_000).unwrap();
        assert_eq!(verified.plan, "pro");
        assert_eq!(verified.sub, "user_1");
        assert_eq!(verified.exp, 5_000);
    }

    #[test]
    fn rejects_expired_wrong_device_and_versions() {
        let key = signing_key();
        let public = key.verifying_key();
        let token = mint(&key, claims(5_000));
        assert_eq!(
            verify(&token, &public, DEVICE, 5_000),
            Err(TokenError::Expired { plan: "pro".into() })
        );
        assert_eq!(
            verify(&token, &public, "another-device", 2_000),
            Err(TokenError::WrongDevice)
        );
        let v2 = mint(
            &key,
            json!({"v": 2, "sub": "u", "plan": "pro", "device": DEVICE, "iat": 0, "exp": 9_999}),
        );
        assert_eq!(verify(&v2, &public, DEVICE, 1), Err(TokenError::UnsupportedVersion(2)));
    }

    #[test]
    fn rejects_forged_and_garbled_tokens() {
        let key = signing_key();
        let public = key.verifying_key();
        let token = mint(&key, claims(5_000));
        let (_, signature) = token.split_once('.').unwrap();
        let mut forged_claims = claims(5_000);
        forged_claims["plan"] = "enterprise".into();
        let forged = format!(
            "{}.{signature}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&forged_claims).unwrap())
        );
        assert_eq!(verify(&forged, &public, DEVICE, 2_000), Err(TokenError::BadSignature));

        let other = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(
            verify(&mint(&other, claims(5_000)), &public, DEVICE, 2_000),
            Err(TokenError::BadSignature)
        );
        assert_eq!(verify("nodot", &public, DEVICE, 0), Err(TokenError::Malformed));
        assert_eq!(verify("a.!!!", &public, DEVICE, 0), Err(TokenError::Malformed));
    }

    #[test]
    fn parses_public_key_formats() {
        let public = signing_key().verifying_key();
        let raw = public.to_bytes();
        let b64url = URL_SAFE_NO_PAD.encode(raw);
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let mut spki = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
        spki.extend_from_slice(&raw);
        let pem = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            STANDARD.encode(&spki)
        );
        for text in [b64url.as_str(), &STANDARD.encode(raw), &hex, &pem] {
            assert_eq!(parse_public_key(text).unwrap(), public, "{text}");
        }
        // The compiled-in production key is a valid one.
        assert!(parse_public_key(crate::PUBLIC_KEY).is_ok());
        assert_eq!(parse_public_key("abc"), Err(TokenError::NoPublicKey));
    }
}
