use crate::{
    config::Config,
    error::{AppError, Result},
    model::text,
    network,
    template::Rendered,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug)]
pub struct Delivery {
    pub status: Option<u16>,
    pub retryable: bool,
    pub invalid_subscription: bool,
    pub code: &'static str,
    pub retry_after: Option<u64>,
}
pub type DeliveryResult = std::result::Result<Option<u16>, Delivery>;
fn failure(code: &'static str, retryable: bool) -> Delivery {
    Delivery {
        status: None,
        retryable,
        invalid_subscription: false,
        code,
        retry_after: None,
    }
}
fn field<'a>(c: &'a Value, name: &str) -> Result<&'a str> {
    c.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad(format!("Missing {name}")))
}
fn optional<'a>(c: &'a Value, name: &str) -> &'a str {
    c.get(name).and_then(Value::as_str).unwrap_or("")
}
fn parse_subscription(c: &Value) -> Result<web_push::SubscriptionInfo> {
    let sub: web_push::SubscriptionInfo = serde_json::from_value(c.clone())
        .map_err(|_| AppError::bad("Invalid Web Push subscription"))?;
    let public = URL_SAFE_NO_PAD
        .decode(&sub.keys.p256dh)
        .map_err(|_| AppError::bad("Invalid P-256 subscription key"))?;
    let auth = URL_SAFE_NO_PAD
        .decode(&sub.keys.auth)
        .map_err(|_| AppError::bad("Invalid Web Push authentication secret"))?;
    if public.len() != 65 || public.first() != Some(&4) || auth.len() != 16 {
        return Err(AppError::bad("Invalid Web Push key lengths"));
    }
    Ok(sub)
}
pub async fn validate(kind: &str, c: &Value, cfg: &Config) -> Result<()> {
    if !c.is_object() || c.to_string().len() > 16000 {
        return Err(AppError::bad(
            "Connection config must be a small JSON object",
        ));
    }
    match kind {
        "bark" => {
            text(field(c, "device_key")?, 1, 256)?;
            let u = network::parse_url(field(c, "base_url")?, cfg)?;
            if u.query().is_some() {
                return Err(AppError::bad("Bark base URL cannot contain a query"));
            }
            network::resolve(
                u.host_str().unwrap_or_default(),
                u.port_or_known_default().unwrap_or(443),
                cfg,
            )
            .await?;
        }
        "email" => {
            let host = field(c, "host")?;
            text(host, 1, 253)?;
            let port = c
                .get("port")
                .and_then(Value::as_u64)
                .filter(|p| (1..=65535).contains(p))
                .ok_or_else(|| AppError::bad("Invalid SMTP port"))? as u16;
            network::resolve(host, port, cfg).await?;
            let security = field(c, "security")?;
            if !["tls", "starttls"].contains(&security)
                && !(security == "none"
                    && cfg.allow_insecure_notifications
                    && cfg.private_hosts.contains(host))
            {
                return Err(AppError::bad("SMTP requires TLS or mandatory STARTTLS"));
            }
            field(c, "from")?
                .parse::<Mailbox>()
                .map_err(|_| AppError::bad("Invalid sender mailbox"))?;
            field(c, "to")?
                .parse::<Mailbox>()
                .map_err(|_| AppError::bad("Invalid recipient mailbox"))?;
        }
        "telegram" => {
            let token = field(c, "bot_token")?;
            if token.len() > 200
                || !token.contains(':')
                || !token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b':' || b == b'_' || b == b'-')
            {
                return Err(AppError::bad("Invalid Telegram bot token"));
            }
            text(field(c, "chat_id")?, 1, 100)?;
        }
        "webhook" => {
            let u = network::parse_url(field(c, "url")?, cfg)?;
            network::resolve(
                u.host_str().unwrap_or_default(),
                u.port_or_known_default().unwrap_or(443),
                cfg,
            )
            .await?;
        }
        "webpush" => {
            if cfg.vapid_private.is_none() {
                return Err(AppError::bad(
                    "The server has not configured VAPID_PRIVATE_KEY",
                ));
            }
            let sub = parse_subscription(c)?;
            let u = network::parse_url(&sub.endpoint, cfg)?;
            // Browser push endpoints are HTTPS even when the application runs on localhost.
            if u.scheme() != "https" {
                return Err(AppError::bad("Web Push endpoint must use HTTPS"));
            }
            network::resolve(
                u.host_str().unwrap_or_default(),
                u.port_or_known_default().unwrap_or(443),
                cfg,
            )
            .await?;
        }
        _ => return Err(AppError::bad("Unknown notification provider")),
    }
    Ok(())
}
async fn json_request(
    url: &str,
    payload: Value,
    bearer: &str,
    job: Uuid,
    cfg: &Config,
) -> std::result::Result<(u16, Value), Delivery> {
    let u = network::parse_url(url, cfg).map_err(|_| failure("DESTINATION_BLOCKED", false))?;
    let client = network::client(&u, cfg)
        .await
        .map_err(|_| failure("DESTINATION_UNAVAILABLE", true))?;
    let mut r = client
        .post(u)
        .header("Idempotency-Key", job.to_string())
        .json(&payload);
    if !bearer.is_empty() {
        r = r.bearer_auth(bearer);
    }
    let response = r
        .send()
        .await
        .map_err(|_| failure("HTTP_TRANSPORT", true))?;
    let status = response.status().as_u16();
    let after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|n| n.min(86400));
    let body = network::small_body(response)
        .await
        .map_err(|_| failure("HTTP_RESPONSE_LIMIT", false))?;
    if !(200..300).contains(&status) {
        return Err(Delivery {
            status: Some(status),
            retryable: status == 429 || status >= 500 || status == 408,
            invalid_subscription: false,
            code: "HTTP_REJECTED",
            retry_after: after,
        });
    }
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}
pub async fn send(kind: &str, c: &Value, r: &Rendered, job: Uuid, cfg: &Config) -> DeliveryResult {
    let result =
        tokio::time::timeout(Duration::from_secs(25), send_inner(kind, c, r, job, cfg)).await;
    result.unwrap_or_else(|_| Err(failure("PROVIDER_TIMEOUT", true)))
}
async fn send_inner(
    kind: &str,
    c: &Value,
    r: &Rendered,
    job: Uuid,
    cfg: &Config,
) -> DeliveryResult {
    match kind {
        "bark" => {
            let url = format!("{}/push", optional(c, "base_url").trim_end_matches('/'));
            let (s,body) = json_request(&url,json!({"device_key":optional(c,"device_key"),"title":r.title,"body":r.body,"url":r.url,"group":r.group,"sound":r.sound,"level":if r.level.is_empty(){"active"}else{&r.level},"isArchive":0}),"",job,cfg).await?;
            if body.get("code").and_then(Value::as_i64) != Some(200) {
                return Err(failure("BARK_REJECTED", false));
            }
            Ok(Some(s))
        }
        "telegram" => {
            let text = format!(
                "{}\n\n{}{}",
                r.title,
                r.body,
                if r.url.is_empty() {
                    String::new()
                } else {
                    format!("\n\n{}", r.url)
                }
            );
            if text.encode_utf16().count() > 4096 {
                return Err(failure("TELEGRAM_MESSAGE_TOO_LONG", false));
            }
            let url = format!(
                "https://api.telegram.org/bot{}/sendMessage",
                optional(c, "bot_token")
            );
            let (s,body) = json_request(&url,json!({"chat_id":optional(c,"chat_id"),"text":text,"link_preview_options":{"is_disabled":true}}),"",job,cfg).await?;
            if body.get("ok").and_then(Value::as_bool) != Some(true) {
                return Err(failure("TELEGRAM_REJECTED", false));
            }
            Ok(Some(s))
        }
        "webhook" => {
            let (s, _) = json_request(
                optional(c, "url"),
                json!({"id":job,"source":"CardDue","notification":r}),
                optional(c, "bearer_token"),
                job,
                cfg,
            )
            .await?;
            Ok(Some(s))
        }
        "email" => {
            let host = optional(c, "host");
            let port = c.get("port").and_then(Value::as_u64).unwrap_or(465) as u16;
            let addresses = network::resolve(host, port, cfg)
                .await
                .map_err(|_| failure("SMTP_DESTINATION", false))?;
            let tls = TlsParameters::new(host.to_owned())
                .map_err(|_| failure("SMTP_TLS_CONFIG", false))?;
            let tls = match optional(c, "security") {
                "tls" => Tls::Wrapper(tls),
                "starttls" => Tls::Required(tls),
                "none" if cfg.allow_insecure_notifications && cfg.private_hosts.contains(host) => {
                    Tls::None
                }
                _ => return Err(failure("SMTP_TLS_REQUIRED", false)),
            };
            // Connect to the validated IP, but verify the TLS certificate against the original hostname.
            let mut transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                addresses[0].ip().to_string(),
            )
            .port(port)
            .tls(tls)
            .timeout(Some(Duration::from_secs(18)));
            if !optional(c, "username").is_empty() {
                transport = transport.credentials(Credentials::new(
                    optional(c, "username").into(),
                    optional(c, "password").into(),
                ));
            }
            let from = optional(c, "from")
                .parse::<Mailbox>()
                .map_err(|_| failure("SMTP_MAILBOX", false))?;
            let to = optional(c, "to")
                .parse::<Mailbox>()
                .map_err(|_| failure("SMTP_MAILBOX", false))?;
            let mut parts = MultiPart::alternative().singlepart(SinglePart::plain(r.body.clone()));
            if !r.html.is_empty() {
                parts = parts.singlepart(SinglePart::html(r.html.clone()));
            }
            let message = Message::builder()
                .from(from)
                .to(to)
                .subject(&r.title)
                .message_id(Some(format!("<{job}@carddue.local>")))
                .multipart(parts)
                .map_err(|_| failure("SMTP_MESSAGE", false))?;
            transport.build().send(message).await.map_err(|e| {
                failure(
                    if e.is_timeout() {
                        "SMTP_TIMEOUT"
                    } else {
                        "SMTP_REJECTED"
                    },
                    !e.is_permanent(),
                )
            })?;
            Ok(None)
        }
        "webpush" => {
            let sub = parse_subscription(c).map_err(|_| failure("WEBPUSH_SUBSCRIPTION", false))?;
            let key = cfg
                .vapid_private
                .as_ref()
                .ok_or_else(|| failure("VAPID_NOT_CONFIGURED", false))?;
            let mut signature = web_push::VapidSignatureBuilder::from_base64(key, &sub)
                .map_err(|_| failure("VAPID_KEY", false))?;
            signature.add_claim("sub", cfg.vapid_subject.clone());
            let signature = signature
                .build()
                .map_err(|_| failure("VAPID_SIGNATURE", false))?;
            let payload = serde_json::to_vec(
                &json!({"title":r.title,"body":r.body,"url":r.url,"tag":job.to_string()}),
            )
            .map_err(|_| failure("WEBPUSH_PAYLOAD", false))?;
            if payload.len() > 3000 {
                return Err(failure("WEBPUSH_MESSAGE_TOO_LONG", false));
            }
            let mut builder = web_push::WebPushMessageBuilder::new(&sub);
            builder.set_payload(web_push::ContentEncoding::Aes128Gcm, &payload);
            builder.set_vapid_signature(signature);
            builder.set_ttl(86400);
            let message = builder
                .build()
                .map_err(|_| failure("WEBPUSH_ENCRYPTION", false))?;
            let body = message
                .payload
                .ok_or_else(|| failure("WEBPUSH_PAYLOAD", false))?;
            let u = network::parse_url(&sub.endpoint, cfg)
                .map_err(|_| failure("DESTINATION_BLOCKED", false))?;
            let client = network::client(&u, cfg)
                .await
                .map_err(|_| failure("WEBPUSH_DESTINATION", true))?;
            let mut request = client
                .post(u)
                .header("TTL", "86400")
                .header("Content-Encoding", "aes128gcm")
                .header("Content-Type", "application/octet-stream");
            for (key, value) in body.crypto_headers {
                request = request.header(key, value);
            }
            let response = request
                .body(body.content)
                .send()
                .await
                .map_err(|_| failure("WEBPUSH_TRANSPORT", true))?;
            let s = response.status().as_u16();
            if (200..300).contains(&s) {
                return Ok(Some(s));
            }
            Err(Delivery {
                status: Some(s),
                retryable: s == 429 || s >= 500,
                invalid_subscription: s == 404 || s == 410,
                code: "WEBPUSH_REJECTED",
                retry_after: None,
            })
        }
        _ => Err(failure("UNKNOWN_PROVIDER", false)),
    }
}
