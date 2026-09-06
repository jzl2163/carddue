use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::{collections::HashSet, env, fs, net::SocketAddr};
use url::Url;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub base_url: String,
    pub bind: SocketAddr,
    pub role: String,
    pub secure_cookie: bool,
    pub encryption_key: [u8; 32],
    pub signing_key: [u8; 32],
    pub setup_token: String,
    pub allow_registration: bool,
    pub private_hosts: HashSet<String>,
    pub allow_insecure_notifications: bool,
    pub vapid_private: Option<String>,
    pub vapid_subject: String,
    pub frontend_dir: String,
    pub default_timezone: String,
}
fn value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.is_empty())
}
fn secret(name: &str) -> anyhow::Result<Option<String>> {
    match (value(name), value(&format!("{name}_FILE"))) {
        (Some(_), Some(_)) => bail!("Set only {name} or {name}_FILE"),
        (Some(s), None) => Ok(Some(s)),
        (None, Some(path)) => Ok(Some(
            fs::read_to_string(path)
                .with_context(|| format!("Cannot read {name}_FILE"))?
                .trim()
                .to_owned(),
        )),
        _ => Ok(None),
    }
}
fn key(name: &str) -> anyhow::Result<[u8; 32]> {
    let s = secret(name)?.with_context(|| format!("Missing {name}"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(s)
        .with_context(|| format!("{name} must be base64url without padding"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("{name} must encode exactly 32 random bytes"))
}
impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let u = Url::parse(
            &value("APP_BASE_URL").unwrap_or_else(|| "https://carddue.example.com".into()),
        )?;
        if !u.username().is_empty()
            || u.password().is_some()
            || u.path() != "/"
            || u.query().is_some()
            || u.fragment().is_some()
        {
            bail!("APP_BASE_URL must be an origin without credentials, path, query or fragment");
        }
        let development = value("APP_ENV").as_deref() == Some("development");
        if u.scheme() != "https"
            && !(development
                && u.scheme() == "http"
                && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
        {
            bail!("HTTPS is required; loopback HTTP is permitted only in development");
        }
        let role = value("APP_ROLE").unwrap_or_else(|| "all".into());
        if !["all", "web", "worker"].contains(&role.as_str()) {
            bail!("Invalid APP_ROLE");
        }
        let tz = value("DEFAULT_TIMEZONE").unwrap_or_else(|| "Asia/Shanghai".into());
        tz.parse::<chrono_tz::Tz>()
            .context("Invalid DEFAULT_TIMEZONE")?;
        let setup_token = secret("SETUP_TOKEN")?.context("SETUP_TOKEN is required")?;
        if setup_token.len() < 32 {
            bail!("SETUP_TOKEN must contain at least 32 random characters");
        }
        let vapid_private = secret("VAPID_PRIVATE_KEY")?;
        if let Some(k) = &vapid_private {
            web_push::VapidSignatureBuilder::from_base64_no_sub(k)
                .context("Invalid VAPID_PRIVATE_KEY")?;
        }
        let vapid_subject =
            value("VAPID_SUBJECT").unwrap_or_else(|| "mailto:admin@example.com".into());
        if !vapid_subject.starts_with("mailto:") && !vapid_subject.starts_with("https://") {
            bail!("Invalid VAPID_SUBJECT");
        }
        Ok(Self {
            database_url: secret("DATABASE_URL")?.context("DATABASE_URL is required")?,
            base_url: u.as_str().trim_end_matches('/').into(),
            bind: value("APP_BIND")
                .unwrap_or_else(|| "0.0.0.0:8080".into())
                .parse()?,
            role,
            secure_cookie: u.scheme() == "https",
            encryption_key: key("APP_ENCRYPTION_KEY")?,
            signing_key: key("ICS_SIGNING_KEY")?,
            setup_token,
            allow_registration: value("ALLOW_REGISTRATION").as_deref() == Some("true"),
            private_hosts: value("NOTIFICATION_PRIVATE_HOSTS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            allow_insecure_notifications: value("ALLOW_INSECURE_NOTIFICATIONS").as_deref()
                == Some("true"),
            vapid_private,
            vapid_subject,
            frontend_dir: value("FRONTEND_DIR").unwrap_or_else(|| "frontend/build".into()),
            default_timezone: tz,
        })
    }
    pub fn cookie_name(&self) -> &'static str {
        if self.secure_cookie {
            "__Host-carddue"
        } else {
            "carddue"
        }
    }
}
