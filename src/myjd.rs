//! Client for the My.JDownloader API (https://api.jdownloader.org).
//!
//! The protocol is small but particular: every request carries an increasing
//! request id, server calls are signed with an HMAC over the query string,
//! device calls are AES-CBC encrypted bodies, and the encryption keys are
//! derived from the account and the session token. This module implements
//! exactly that and nothing else; the JDownloader-specific calls live in
//! `api.rs`.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

const API_URL: &str = "https://api.jdownloader.org";
const APP_KEY: &str = "jdtui";
const API_VERSION: u32 = 1;
const CONTENT_TYPE: &str = "application/aesjson-jd; charset=utf-8";

/// Same set Python's `urllib.parse.quote` leaves untouched, which is what the
/// server signs against.
const QUERY_ENCODE: &AsciiSet = &NON_ALPHANUMERIC.remove(b'_').remove(b'.').remove(b'-').remove(b'~').remove(b'/');

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug)]
pub enum Error {
    /// The server rejected the request; `kind` is its error type, such as
    /// `AUTH_FAILED`, `TOKEN_INVALID`, `OFFLINE` or `OUTDATED`.
    Api { source: String, kind: String },
    /// Transport or decoding failure.
    Transport(String),
    /// A call was made without a session.
    NotConnected,
}

impl Error {
    /// Whether the credentials themselves were refused.
    pub fn is_auth_failure(&self) -> bool {
        matches!(self, Error::Api { kind, .. } if kind == "AUTH_FAILED" || kind == "EMAIL_INVALID")
    }

    /// Whether the session is gone and a reconnect is worth trying.
    pub fn is_session_expired(&self) -> bool {
        matches!(self, Error::Api { kind, .. } if kind == "TOKEN_INVALID" || kind == "SESSION_EXPIRED")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api { source, kind } => write!(f, "{kind} ({source})"),
            Error::Transport(msg) => write!(f, "{msg}"),
            Error::NotConnected => write!(f, "not connected"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        Error::Transport(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Transport(format!("bad json from server: {e}"))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Deserialize)]
struct SessionResponse {
    sessiontoken: String,
    regaintoken: String,
}

#[derive(Deserialize)]
struct DeviceList {
    list: Vec<Device>,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(default)]
    src: String,
    #[serde(rename = "type", default)]
    r#type: String,
}

#[derive(Deserialize)]
struct DeviceResponse<T> {
    rid: i64,
    data: T,
}

/// Session keys. Kept together because they change together on (re)connect.
struct Session {
    session_token: String,
    regain_token: String,
    server_key: [u8; 32],
    device_key: [u8; 32],
}

pub struct MyJd {
    agent: ureq::Agent,
    login_secret: [u8; 32],
    device_secret: [u8; 32],
    email: String,
    session: Option<Session>,
    last_rid: i64,
}

impl MyJd {
    pub fn new(email: &str, password: &str) -> Self {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().new_agent();
        Self {
            agent,
            login_secret: secret(email, password, "server"),
            device_secret: secret(email, password, "device"),
            email: email.to_string(),
            session: None,
            last_rid: 0,
        }
    }

    /// Open a session. The response is encrypted with the login secret, and
    /// from then on the server and device keys derive from the session token.
    pub fn connect(&mut self) -> Result<()> {
        self.session = None;
        let params = [("email", self.email.clone()), ("appkey", APP_KEY.to_string())];
        let resp: SessionResponse = self.server_get("/my/connect", &params)?;
        self.install_session(resp, None);
        Ok(())
    }

    /// Renew an expired session without sending the password again.
    pub fn reconnect(&mut self) -> Result<()> {
        let (session_token, regain_token, server_key) = match &self.session {
            Some(s) => (s.session_token.clone(), s.regain_token.clone(), s.server_key),
            None => return Err(Error::NotConnected),
        };
        let params = [("sessiontoken", session_token), ("regaintoken", regain_token)];
        let resp: SessionResponse = self.server_get("/my/reconnect", &params)?;
        self.install_session(resp, Some(server_key));
        Ok(())
    }

    #[allow(dead_code)]
    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(s) = &self.session {
            let params = [("sessiontoken", s.session_token.clone())];
            let _: serde_json::Value = self.server_get("/my/disconnect", &params)?;
        }
        self.session = None;
        Ok(())
    }

    pub fn list_devices(&mut self) -> Result<Vec<Device>> {
        let token = self.session_token()?;
        let resp: DeviceList = self.server_get("/my/listdevices", &[("sessiontoken", token)])?;
        Ok(resp.list)
    }

    /// Call an endpoint on a device. `params` are serialised one by one, each
    /// as its own JSON string, which is how the server expects them.
    pub fn device_call<T: DeserializeOwned>(
        &mut self,
        device_id: &str,
        path: &str,
        params: &[serde_json::Value],
    ) -> Result<T> {
        self.device_call_with_timeout(device_id, path, params, None)
    }

    /// A device call that may legitimately take longer than the agent's
    /// default, such as `/events/listen`, which blocks until an event comes
    /// or its poll timeout runs out.
    pub fn device_call_with_timeout<T: DeserializeOwned>(
        &mut self,
        device_id: &str,
        path: &str,
        params: &[serde_json::Value],
        timeout: Option<Duration>,
    ) -> Result<T> {
        let (session_token, device_key) = match &self.session {
            Some(s) => (s.session_token.clone(), s.device_key),
            None => return Err(Error::NotConnected),
        };
        let rid = self.next_rid();
        let body = serde_json::json!({
            "apiVer": API_VERSION,
            "url": path,
            "params": params.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            "rid": rid,
        });
        let encrypted = encrypt(&device_key, body.to_string().as_bytes());
        let url = format!("{API_URL}/t_{session_token}_{device_id}{path}");

        let mut request =
            self.agent.post(&url).header("Content-Type", CONTENT_TYPE).config().http_status_as_error(false);
        if let Some(t) = timeout {
            request = request.timeout_global(Some(t));
        }
        let response = request.build().send(encrypted.as_bytes())?;
        let status = response.status().as_u16();
        let text = response.into_body().read_to_string()?;
        if status != 200 {
            return Err(parse_error(&text, Some(&device_key)));
        }
        let plain = decrypt(&device_key, &text)?;
        let parsed: DeviceResponse<T> = serde_json::from_slice(&plain)?;
        if parsed.rid != rid {
            return Err(Error::Transport(format!("request id mismatch: sent {rid}, got {}", parsed.rid)));
        }
        Ok(parsed.data)
    }

    // --- internals -----------------------------------------------------

    fn session_token(&self) -> Result<String> {
        self.session.as_ref().map(|s| s.session_token.clone()).ok_or(Error::NotConnected)
    }

    fn install_session(&mut self, resp: SessionResponse, previous_server_key: Option<[u8; 32]>) {
        let token_bytes = hex_decode(&resp.sessiontoken);
        // The server key chains: on a reconnect it derives from the previous
        // server key rather than from the login secret.
        let base = previous_server_key.unwrap_or(self.login_secret);
        self.session = Some(Session {
            server_key: sha256_concat(&base, &token_bytes),
            device_key: sha256_concat(&self.device_secret, &token_bytes),
            session_token: resp.sessiontoken,
            regain_token: resp.regaintoken,
        });
    }

    /// Request ids must increase within a session; the clock alone repeats
    /// within a millisecond under load.
    fn next_rid(&mut self) -> i64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        self.last_rid = now.max(self.last_rid + 1);
        self.last_rid
    }

    /// A signed GET against the `/my/*` endpoints.
    fn server_get<T: DeserializeOwned>(&mut self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let rid = self.next_rid();
        let mut query = String::from(path);
        query.push('?');
        for (k, v) in params {
            query.push_str(k);
            query.push('=');
            query.push_str(&utf8_percent_encode(v, QUERY_ENCODE).to_string());
            query.push('&');
        }
        query.push_str(&format!("rid={rid}"));

        // Before a session exists the login secret signs and decrypts.
        let key = match &self.session {
            Some(s) => s.server_key,
            None => self.login_secret,
        };
        let signature = hmac_hex(&key, query.as_bytes());
        let url = format!("{API_URL}{query}&signature={signature}");

        let response = self.agent.get(&url).config().http_status_as_error(false).build().call()?;
        let status = response.status().as_u16();
        let text = response.into_body().read_to_string()?;
        if status != 200 {
            return Err(parse_error(&text, None));
        }
        let plain = decrypt(&key, &text)?;
        Ok(serde_json::from_slice(&plain)?)
    }
}

fn parse_error(text: &str, device_key: Option<&[u8; 32]>) -> Error {
    let plain = serde_json::from_str::<ApiError>(text).ok().or_else(|| {
        device_key.and_then(|k| decrypt(k, text).ok()).and_then(|b| serde_json::from_slice::<ApiError>(&b).ok())
    });
    match plain {
        Some(e) => Error::Api { source: e.src, kind: e.r#type },
        None => Error::Transport(format!("server error: {}", text.trim())),
    }
}

// --- primitives --------------------------------------------------------------

/// SHA-256 of lowercase email + password + domain, the root of every key.
pub fn secret(email: &str, password: &str, domain: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(email.to_lowercase().as_bytes());
    h.update(password.as_bytes());
    h.update(domain.to_lowercase().as_bytes());
    h.finalize().into()
}

fn sha256_concat(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(a);
    h.update(b);
    h.finalize().into()
}

pub fn hmac_hex(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    hex_encode(&mac.finalize().into_bytes())
}

/// AES-128-CBC with PKCS#7, base64 encoded. The 32-byte token splits into
/// iv (first half) and key (second half).
pub fn encrypt(token: &[u8; 32], plain: &[u8]) -> String {
    let (iv, key) = token.split_at(16);
    let enc = Aes128CbcEnc::new_from_slices(key, iv).expect("fixed-size key and iv");
    BASE64.encode(enc.encrypt_padded_vec::<Pkcs7>(plain))
}

pub fn decrypt(token: &[u8; 32], data: &str) -> Result<Vec<u8>> {
    let (iv, key) = token.split_at(16);
    let raw = BASE64.decode(data.trim()).map_err(|e| Error::Transport(format!("response is not base64: {e}")))?;
    let dec = Aes128CbcDec::new_from_slices(key, iv).expect("fixed-size key and iv");
    dec.decrypt_padded_vec::<Pkcs7>(&raw)
        .map_err(|_| Error::Transport("response failed to decrypt (wrong key?)".into()))
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len().saturating_sub(1)).step_by(2).filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    //! Expected values were produced with the reference Python client
    //! (myjdapi 1.1.x) for the same inputs, so these pin byte-for-byte
    //! compatibility with the protocol as it is actually spoken.
    use super::*;

    const EMAIL: &str = "Test.User@Example.com";
    const PASSWORD: &str = "pässword";

    #[test]
    fn secrets_match_reference() {
        assert_eq!(
            hex_encode(&secret(EMAIL, PASSWORD, "server")),
            "646068ae4e3c8cc1d9be13609bdb1f18566bcf8ee02002d7aaf426823f899fa8"
        );
        assert_eq!(
            hex_encode(&secret(EMAIL, PASSWORD, "device")),
            "b3648da41f19610f89afbe4c29de3821a5ac73360f8dac2665e47daa7fe9c029"
        );
    }

    #[test]
    fn query_encoding_and_signature_match_reference() {
        let email = utf8_percent_encode(EMAIL, QUERY_ENCODE).to_string();
        let query = format!("/my/connect?email={email}&appkey=jdtui&rid=1700000000000");
        assert_eq!(query, "/my/connect?email=Test.User%40Example.com&appkey=jdtui&rid=1700000000000");
        let key = secret(EMAIL, PASSWORD, "server");
        assert_eq!(
            hmac_hex(&key, query.as_bytes()),
            "7a90bfb7c4a7b234306644c777174c0b5b1ba94177ecf8f77f996c7340c2d950"
        );
    }

    #[test]
    fn aes_cbc_matches_reference() {
        let key = secret(EMAIL, PASSWORD, "server");
        let plain = r#"{"hello":"wörld","n":1}"#;
        let encrypted = encrypt(&key, plain.as_bytes());
        assert_eq!(encrypted, "4ItvyLMo6hhRuxmOgdgwMT0k1cKz2XcWCGfNse1e0cM=");
        assert_eq!(decrypt(&key, &encrypted).unwrap(), plain.as_bytes());
    }

    #[test]
    fn hex_roundtrip() {
        let bytes = [0u8, 1, 0xab, 0xff];
        assert_eq!(hex_encode(&bytes), "0001abff");
        assert_eq!(hex_decode("0001abff"), bytes);
    }
}

#[cfg(test)]
mod live {
    //! Talk to the real service. Run with `cargo test live -- --ignored --nocapture`.
    use super::*;

    #[test]
    #[ignore]
    fn wrong_credentials_are_refused_cleanly() {
        let mut jd = MyJd::new("nobody@example.invalid", "definitely-wrong");
        let err = jd.connect().expect_err("bogus credentials must fail");
        println!("server said: {err}");
        assert!(err.is_auth_failure(), "expected AUTH_FAILED, got {err:?}");
    }

    #[test]
    #[ignore]
    fn real_login_lists_devices() {
        let cfg = crate::config::Config::load().expect("config");
        let (Some(email), Some(password)) = (cfg.email, cfg.password) else {
            eprintln!("no credentials in config, skipping");
            return;
        };
        let mut jd = MyJd::new(&email, &password);
        jd.connect().expect("connect");
        let devices = jd.list_devices().expect("list devices");
        println!("devices: {}", devices.len());
        for d in &devices {
            println!("  {} [{}] {}", d.name, d.kind, d.id);
        }
        assert!(!devices.is_empty());
        let state: String =
            jd.device_call(&devices[0].id, "/downloadcontroller/getCurrentState", &[]).expect("device call");
        println!("state of {}: {state}", devices[0].name);
        jd.disconnect().expect("disconnect");
    }
}
