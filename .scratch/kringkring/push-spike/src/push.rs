use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::ORIGIN},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::{SecretKey, elliptic_curve::rand_core::OsRng};
use url::Url;
use web_push::{
    ContentEncoding, IsahcWebPushClient, PartialVapidSignatureBuilder, SubscriptionInfo,
    VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder,
};

#[derive(Clone)]
pub struct Push {
    origin: String,
    contact: String,
    vapid: PartialVapidSignatureBuilder,
    client: IsahcWebPushClient,
    subscriptions: Arc<Mutex<HashMap<String, SubscriptionInfo>>>,
}

impl Push {
    pub fn new(origin: String, contact: String) -> Result<Self, web_push::WebPushError> {
        let key = SecretKey::random(&mut OsRng);
        Ok(Self {
            origin,
            contact,
            vapid: VapidSignatureBuilder::from_base64_no_sub(
                &URL_SAFE_NO_PAD.encode(key.to_bytes()),
            )?,
            client: IsahcWebPushClient::new()?,
            subscriptions: Arc::default(),
        })
    }

    fn check_origin(&self, headers: &HeaderMap) -> Result<(), (StatusCode, &'static str)> {
        if headers.get(ORIGIN).and_then(|value| value.to_str().ok()) == Some(&self.origin) {
            Ok(())
        } else {
            Err((StatusCode::FORBIDDEN, "Use the spike's HTTPS origin."))
        }
    }
}

pub async fn public_key(State(push): State<Push>) -> Json<String> {
    Json(URL_SAFE_NO_PAD.encode(push.vapid.get_public_key()))
}

pub async fn subscribe(
    State(push): State<Push>,
    headers: HeaderMap,
    Json(subscription): Json<SubscriptionInfo>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    push.check_origin(&headers)?;
    let endpoint = Url::parse(&subscription.endpoint)
        .ok()
        .filter(|url| {
            url.scheme() == "https"
                && url.port().is_none()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.host_str().is_some_and(|host| {
                    host == "fcm.googleapis.com"
                        || host == "web.push.apple.com"
                        || host.ends_with(".web.push.apple.com")
                })
        })
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Expected an Apple or FCM HTTPS push endpoint.",
        ))?;
    let mut subscription = subscription;
    subscription.endpoint = endpoint.into();
    push.subscriptions
        .lock()
        .unwrap()
        .insert(subscription.endpoint.clone(), subscription);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn send(
    State(push): State<Push>,
    headers: HeaderMap,
) -> Result<String, (StatusCode, &'static str)> {
    push.check_origin(&headers)?;
    let subscriptions: Vec<_> = push
        .subscriptions
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect();
    if subscriptions.is_empty() {
        return Err((StatusCode::CONFLICT, "Subscribe on a phone first."));
    }

    let mut accepted = 0;
    let mut failed = 0;
    for subscription in &subscriptions {
        let mut vapid = push.vapid.clone().add_sub_info(subscription);
        vapid.add_claim("sub", push.contact.as_str());
        let message = vapid.build().and_then(|signature| {
            let mut message = WebPushMessageBuilder::new(subscription);
            message.set_vapid_signature(signature);
            message.set_ttl(60);
            message.set_payload(
                ContentEncoding::Aes128Gcm,
                b"Kringkring closed-app delivery test",
            );
            message.build()
        });
        match message {
            Ok(message) => {
                match tokio::time::timeout(Duration::from_secs(10), push.client.send(message)).await
                {
                    Ok(Ok(())) => {
                        accepted += 1;
                        continue;
                    }
                    Ok(Err(err)) => eprintln!(
                        "Push service rejected the request ({:?})",
                        std::mem::discriminant(&err)
                    ),
                    Err(_) => eprintln!("Push service request timed out"),
                }
            }
            Err(_) => eprintln!("Could not build push: check the subscription keys"),
        }
        failed += 1;
    }
    let report = format!(
        "Push services accepted: {accepted}; failed: {failed}. Check the phones for delivery."
    );
    if failed > 0 {
        // Keep partial failures visible without logging subscription URLs or keys.
        Ok(format!("Send incomplete. {report}"))
    } else {
        Ok(report)
    }
}
