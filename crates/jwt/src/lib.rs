//! ES256 JSON Web Tokens signed on the user's machine.
//!
//! Both of Apple's developer APIs AppDoctor talks to authenticate with an
//! ES256 JWT signed by a P-256 private key that never leaves the device:
//! App Store Connect with the `.p8` key the user downloads, Apple Ads with a
//! key pair AppDoctor generates. This crate holds the shared pieces: key
//! parsing and generation, signing, and (for tests and diagnostics) decoding.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::ecdsa::signature::{Signer as _, Verifier as _};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::Generate as _;
use p256::pkcs8::{
    DecodePrivateKey as _, DecodePublicKey as _, EncodePrivateKey as _, EncodePublicKey as _, LineEnding,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "not a P-256 private key in PEM form (expected a .p8 / \"BEGIN PRIVATE KEY\" or \"BEGIN EC PRIVATE KEY\" file)"
    )]
    InvalidPrivateKey,
    #[error("not a P-256 public key in PEM form")]
    InvalidPublicKey,
    #[error("malformed token")]
    Malformed,
    #[error("token signature does not verify")]
    BadSignature,
    #[error("could not encode key: {0}")]
    Encode(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A P-256 private key used to sign ES256 tokens.
#[derive(Clone)]
pub struct EcKey {
    signing: SigningKey,
}

impl std::fmt::Debug for EcKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EcKey(..)")
    }
}

impl EcKey {
    /// Parses a PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`, App Store
    /// Connect's `.p8` format) or SEC1 PEM (`-----BEGIN EC PRIVATE KEY-----`,
    /// what `openssl ecparam -genkey` writes).
    pub fn from_pem(pem: &str) -> Result<Self> {
        let pem = pem.trim();
        let signing = SigningKey::from_pkcs8_pem(pem)
            .ok()
            .or_else(|| p256::SecretKey::from_sec1_pem(pem).ok().map(SigningKey::from))
            .ok_or(Error::InvalidPrivateKey)?;
        Ok(Self { signing })
    }

    /// A new random key from the OS's secure RNG.
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate_from_rng(&mut rand::rng()),
        }
    }

    /// The private key as PKCS#8 PEM, for storing in the keychain.
    pub fn to_pkcs8_pem(&self) -> Result<String> {
        self.signing
            .to_pkcs8_pem(LineEnding::LF)
            .map(|pem| pem.to_string())
            .map_err(|error| Error::Encode(error.to_string()))
    }

    /// The public key as SPKI PEM (`-----BEGIN PUBLIC KEY-----`), the form
    /// `openssl ec -pubout` prints and Apple Ads asks the user to paste.
    pub fn public_key_pem(&self) -> Result<String> {
        self.signing
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|error| Error::Encode(error.to_string()))
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey {
            verifying: *self.signing.verifying_key(),
        }
    }

    /// Signs `header.claims` as a compact JWS. `alg` is always ES256; the
    /// signature is the raw 64-byte `r || s` form JWS requires.
    pub fn sign<H: Serialize, C: Serialize>(&self, header: &H, claims: &C) -> Result<String> {
        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(header)?),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims)?)
        );
        let signature: Signature = self.signing.sign(signing_input.as_bytes());
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        ))
    }
}

/// A P-256 public key, for verifying tokens.
#[derive(Debug, Clone)]
pub struct PublicKey {
    verifying: VerifyingKey,
}

impl PublicKey {
    pub fn from_pem(pem: &str) -> Result<Self> {
        VerifyingKey::from_public_key_pem(pem.trim())
            .map(|verifying| Self { verifying })
            .map_err(|_| Error::InvalidPublicKey)
    }

    /// Verifies an ES256 compact JWS and returns its decoded header and claims.
    pub fn verify<H: DeserializeOwned, C: DeserializeOwned>(&self, token: &str) -> Result<(H, C)> {
        let (signing_input, signature) = token.rsplit_once('.').ok_or(Error::Malformed)?;
        let signature = URL_SAFE_NO_PAD.decode(signature).map_err(|_| Error::Malformed)?;
        let signature = Signature::from_slice(&signature).map_err(|_| Error::Malformed)?;
        self.verifying
            .verify(signing_input.as_bytes(), &signature)
            .map_err(|_| Error::BadSignature)?;
        decode_unverified(token)
    }
}

/// Decodes a JWT's header and claims without checking its signature.
pub fn decode_unverified<H: DeserializeOwned, C: DeserializeOwned>(token: &str) -> Result<(H, C)> {
    let mut parts = token.split('.');
    let (Some(header), Some(claims), Some(_), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(Error::Malformed);
    };
    let decode = |part: &str| URL_SAFE_NO_PAD.decode(part).map_err(|_| Error::Malformed);
    Ok((
        serde_json::from_slice(&decode(header)?)?,
        serde_json::from_slice(&decode(claims)?)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    // A throwaway key from `openssl ecparam -genkey -name prime256v1 -noout`.
    const SEC1: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIEuQ8+KwisSyfYOf1eftXnSET0P1GHaXVgOGLJ4ioax1oAoGCCqGSM49
AwEHoUQDQgAELaKPrGwIwm4+MZ2RY5PYk9IK0Aq93zBZAKaa9J5abSyMqut6x+tZ
yHAga11+dQTMAKvoXtirs4ML6sdinTmtfw==
-----END EC PRIVATE KEY-----";

    #[test]
    fn sign_and_verify_round_trip() {
        let key = EcKey::generate();
        let token = key
            .sign(&json!({"alg": "ES256", "kid": "K1"}), &json!({"iss": "me", "n": 1}))
            .unwrap();
        assert_eq!(token.split('.').count(), 3);
        let (header, claims): (Value, Value) = key.public_key().verify(&token).unwrap();
        assert_eq!(header["kid"], "K1");
        assert_eq!(claims["n"], 1);
    }

    #[test]
    fn rejects_tampered_and_foreign_tokens() {
        let key = EcKey::generate();
        let token = key.sign(&json!({"alg": "ES256"}), &json!({"admin": false})).unwrap();
        let (head, rest) = token.split_once('.').unwrap();
        let (_, signature) = rest.split_once('.').unwrap();
        let forged = format!("{head}.{}.{signature}", URL_SAFE_NO_PAD.encode(br#"{"admin":true}"#));
        assert!(matches!(
            key.public_key().verify::<Value, Value>(&forged),
            Err(Error::BadSignature)
        ));
        let other = EcKey::generate();
        assert!(matches!(
            other.public_key().verify::<Value, Value>(&token),
            Err(Error::BadSignature)
        ));
        assert!(matches!(
            key.public_key().verify::<Value, Value>("a.b"),
            Err(Error::Malformed)
        ));
    }

    #[test]
    fn pem_round_trips_through_pkcs8_and_spki() {
        let key = EcKey::generate();
        let private = key.to_pkcs8_pem().unwrap();
        assert!(private.starts_with("-----BEGIN PRIVATE KEY-----"));
        let public = key.public_key_pem().unwrap();
        assert!(public.starts_with("-----BEGIN PUBLIC KEY-----"));

        let reloaded = EcKey::from_pem(&private).unwrap();
        let token = reloaded.sign(&json!({}), &json!({})).unwrap();
        PublicKey::from_pem(&public)
            .unwrap()
            .verify::<Value, Value>(&token)
            .unwrap();
    }

    #[test]
    fn accepts_openssl_sec1_keys() {
        let key = EcKey::from_pem(SEC1).unwrap();
        let pkcs8 = key.to_pkcs8_pem().unwrap();
        assert_eq!(
            EcKey::from_pem(&pkcs8).unwrap().public_key_pem().unwrap(),
            key.public_key_pem().unwrap()
        );
    }

    #[test]
    fn rejects_garbage_keys() {
        assert!(matches!(EcKey::from_pem("hello"), Err(Error::InvalidPrivateKey)));
        assert!(matches!(PublicKey::from_pem("hello"), Err(Error::InvalidPublicKey)));
    }
}
