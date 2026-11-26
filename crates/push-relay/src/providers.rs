use std::{future::Future, path::PathBuf, time::Duration};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use companion_proto::push::Kind;
use reqwest::{Client, StatusCode};
use ring::{rand::SystemRandom, signature};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::{now_ms, Platform};

const ALERT: &str = "A session needs you";
const TOPIC: &str = "net.abushanab.harnessmanager";
const OAUTH: &str = "https://oauth2.googleapis.com/token";

pub(super) enum Delivery {
    Sent,
    InvalidToken,
}

pub(super) trait Provider: Send + Sync + 'static {
    fn send(
        &self,
        platform: Platform,
        token: &str,
        kind: Kind,
        collapse: &str,
    ) -> impl Future<Output = Result<Delivery>> + Send;
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ApnsConfig {
    key_file: PathBuf,
    key_id: String,
    team_id: String,
    #[serde(default)]
    sandbox: bool,
}

#[derive(Deserialize)]
struct ServiceAccount {
    project_id: String,
    client_email: String,
    private_key: String,
}

struct Fcm {
    project: String,
    email: String,
    key: signature::RsaKeyPair,
    bearer: Mutex<Option<(u64, String)>>,
}

struct Apns {
    cfg: ApnsConfig,
    key: signature::EcdsaKeyPair,
    bearer: Mutex<Option<(u64, String)>>,
}

pub(super) struct Providers {
    http: Client,
    fcm: Option<Fcm>,
    apns: Option<Apns>,
}

impl Providers {
    pub(super) fn new(fcm_file: Option<PathBuf>, apns: Option<ApnsConfig>) -> Result<Self> {
        let fcm = fcm_file
            .map(|path| -> Result<Fcm> {
                let a: ServiceAccount = serde_json::from_slice(&std::fs::read(path)?)?;
                let der = pem::parse(a.private_key)?;
                let key = signature::RsaKeyPair::from_pkcs8(der.contents())
                    .map_err(|_| anyhow!("invalid Firebase RSA key"))?;
                Ok(Fcm { project: a.project_id, email: a.client_email, key, bearer: Mutex::default() })
            })
            .transpose()?;
        let apns = apns
            .map(|cfg| -> Result<Apns> {
                let der = pem::parse(std::fs::read(&cfg.key_file)?)?;
                let key = signature::EcdsaKeyPair::from_pkcs8(
                    &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                    der.contents(),
                    &SystemRandom::new(),
                )
                .map_err(|_| anyhow!("invalid APNs P-256 key"))?;
                Ok(Apns { cfg, key, bearer: Mutex::default() })
            })
            .transpose()?;
        let http =
            Client::builder().timeout(Duration::from_secs(5)).redirect(reqwest::redirect::Policy::none()).build()?;
        Ok(Self { http, fcm, apns })
    }
}

fn jwt_input(header: Value, claims: Value) -> String {
    format!("{}.{}", URL_SAFE_NO_PAD.encode(header.to_string()), URL_SAFE_NO_PAD.encode(claims.to_string()))
}

async fn forget_rejected_login(cache: &Mutex<Option<(u64, String)>>, used: &str, status: StatusCode) {
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        let mut cached = cache.lock().await;
        if cached.as_ref().is_some_and(|(_, token)| token == used) {
            *cached = None;
        }
    }
}

fn fcm_message(token: &str, kind: Kind, collapse: &str) -> Value {
    let mut msg = json!({"token":token, "android":{"collapse_key":collapse,"priority":"HIGH","ttl":"60s"}});
    match kind {
        Kind::Needs => msg["notification"] = json!({"body": ALERT}),
        Kind::Live => msg["data"] = json!({"kind":"live"}),
    }
    json!({"message":msg})
}

fn apns_message(kind: Kind, now: u64) -> (&'static str, &'static str, Value) {
    match kind {
        Kind::Needs => (TOPIC, "alert", json!({"aps":{"alert":ALERT}})),
        Kind::Live => (
            "net.abushanab.harnessmanager.push-type.liveactivity",
            "liveactivity",
            json!({"aps":{"timestamp":now,"event":"update","content-state":{}}}),
        ),
    }
}

impl Fcm {
    async fn bearer(&self, http: &Client) -> Result<String> {
        let mut cached = self.bearer.lock().await;
        let now = now_ms() / 1000;
        if let Some((_, token)) = cached.as_ref().filter(|(until, _)| now < *until) {
            return Ok(token.clone());
        }
        let input = jwt_input(
            json!({"alg":"RS256","typ":"JWT"}),
            json!({"iss":self.email, "aud":OAUTH,
            "scope":"https://www.googleapis.com/auth/firebase.messaging", "iat":now, "exp":now+3600}),
        );
        let mut sig = vec![0; self.key.public().modulus_len()];
        self.key
            .sign(&signature::RSA_PKCS1_SHA256, &SystemRandom::new(), input.as_bytes(), &mut sig)
            .map_err(|_| anyhow!("sign Firebase assertion"))?;
        let assertion = format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig));
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
            expires_in: u64,
        }
        let t: Token = http
            .post(OAUTH)
            .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &assertion)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        *cached = Some((now.saturating_add(t.expires_in.saturating_sub(60)), t.access_token.clone()));
        Ok(t.access_token)
    }

    async fn send(&self, http: &Client, token: &str, kind: Kind, collapse: &str) -> Result<Delivery> {
        let bearer = self.bearer(http).await?;
        let response = http
            .post(format!("https://fcm.googleapis.com/v1/projects/{}/messages:send", self.project))
            .bearer_auth(&bearer)
            .json(&fcm_message(token, kind, collapse))
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(Delivery::Sent);
        }
        forget_rejected_login(&self.bearer, &bearer, status).await;
        let error: Value = response.json().await?;
        let invalid = error["error"]["details"].as_array().into_iter().flatten().any(|d| {
            d["@type"] == "type.googleapis.com/google.firebase.fcm.v1.FcmError"
                && matches!(
                    (status, d["errorCode"].as_str()),
                    (StatusCode::NOT_FOUND, Some("UNREGISTERED")) | (StatusCode::BAD_REQUEST, Some("INVALID_ARGUMENT"))
                )
        });
        if invalid {
            Ok(Delivery::InvalidToken)
        } else {
            Err(anyhow!("FCM rejected the request"))
        }
    }
}

impl Apns {
    async fn bearer(&self, now: u64) -> Result<String> {
        let mut cached = self.bearer.lock().await;
        if let Some((_, token)) = cached.as_ref().filter(|(until, _)| now < *until) {
            return Ok(token.clone());
        }
        let input = jwt_input(json!({"alg":"ES256","kid":self.cfg.key_id}), json!({"iss":self.cfg.team_id,"iat":now}));
        let sig = self.key.sign(&SystemRandom::new(), input.as_bytes()).map_err(|_| anyhow!("sign APNs token"))?;
        let token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()));
        *cached = Some((now + 50 * 60, token.clone()));
        Ok(token)
    }

    async fn send(&self, http: &Client, token: &str, kind: Kind, collapse: &str) -> Result<Delivery> {
        let now = now_ms() / 1000;
        let (topic, push_type, payload) = apns_message(kind, now);
        let host = if self.cfg.sandbox { "api.sandbox.push.apple.com" } else { "api.push.apple.com" };
        let bearer = self.bearer(now).await?;
        let response = http
            .post(format!("https://{host}/3/device/{token}"))
            .version(reqwest::Version::HTTP_2)
            .bearer_auth(&bearer)
            .header("apns-topic", topic)
            .header("apns-push-type", push_type)
            .header("apns-collapse-id", collapse)
            .header("apns-expiration", (now + 60).to_string())
            .json(&payload)
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(Delivery::Sent);
        }
        forget_rejected_login(&self.bearer, &bearer, status).await;
        let error: Value = response.json().await?;
        match (status, error["reason"].as_str()) {
            (StatusCode::GONE, Some("Unregistered" | "ExpiredToken"))
            | (StatusCode::BAD_REQUEST, Some("BadDeviceToken")) => Ok(Delivery::InvalidToken),
            _ => Err(anyhow!("APNs rejected the request")),
        }
    }
}

impl Provider for Providers {
    async fn send(&self, platform: Platform, token: &str, kind: Kind, collapse: &str) -> Result<Delivery> {
        match platform {
            Platform::Android => {
                self.fcm.as_ref().context("Firebase not configured")?.send(&self.http, token, kind, collapse).await
            }
            Platform::Ios => {
                self.apns.as_ref().context("APNs not configured")?.send(&self.http, token, kind, collapse).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn only_a_rejection_of_the_current_login_clears_the_cache() {
        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let cache = Mutex::new(Some((123, "current".into())));
            forget_rejected_login(&cache, "superseded", status).await;
            forget_rejected_login(&cache, "current", StatusCode::SERVICE_UNAVAILABLE).await;
            assert!(cache.lock().await.is_some());
            forget_rejected_login(&cache, "current", status).await;
            assert!(cache.lock().await.is_none());
        }
    }

    #[test]
    fn provider_payloads_only_announce_a_request_or_an_update() {
        let android = json!({"collapse_key":"opaque","priority":"HIGH","ttl":"60s"});
        assert_eq!(
            fcm_message("device", Kind::Needs, "opaque"),
            json!({"message":{"token":"device","android":android,"notification":{"body":"A session needs you"}}})
        );
        assert_eq!(
            fcm_message("device", Kind::Live, "opaque"),
            json!({"message":{"token":"device","android":android,"data":{"kind":"live"}}})
        );
        assert_eq!(apns_message(Kind::Needs, 123), (TOPIC, "alert", json!({"aps":{"alert":"A session needs you"}})));
        assert_eq!(
            apns_message(Kind::Live, 123),
            (
                "net.abushanab.harnessmanager.push-type.liveactivity",
                "liveactivity",
                json!({"aps":{"timestamp":123,"event":"update","content-state":{}}})
            )
        );
    }
}
