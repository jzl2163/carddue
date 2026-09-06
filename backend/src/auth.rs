use crate::{
    AppState, crypto,
    error::{AppError, Result},
    model::TemplateInput,
    web::ApiJson,
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json,
    extract::{ConnectInfo, FromRequestParts, State},
    http::{HeaderMap, HeaderValue, StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use std::net::SocketAddr;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Serialize, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub timezone: String,
    pub is_admin: bool,
}
pub struct Auth {
    pub user: User,
    pub token: String,
}
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let mut values = headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .filter_map(|s| s.trim().split_once('='))
        .filter(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned());
    let first = values.next()?;
    if values.next().is_some() || first.len() != 43 {
        return None;
    }
    Some(first)
}
impl FromRequestParts<AppState> for Auth {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let token = cookie(&parts.headers, state.config.cookie_name())
            .ok_or_else(AppError::unauthorized)?;
        let row = sqlx::query("SELECT u.id,u.email,u.timezone,u.is_admin FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.expires_at>now()")
            .bind(crypto::hash(&token)).fetch_optional(&state.db).await?.ok_or_else(AppError::unauthorized)?;
        Ok(Self {
            user: User {
                id: row.try_get("id")?,
                email: row.try_get("email")?,
                timezone: row.try_get("timezone")?,
                is_admin: row.try_get("is_admin")?,
            },
            token,
        })
    }
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetupInput {
    pub email: String,
    pub password: String,
    pub timezone: String,
    pub setup_token: Option<String>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccountInput {
    pub timezone: String,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PasswordInput {
    pub current_password: String,
    pub new_password: String,
}
fn normalize_email(s: &str) -> Result<String> {
    let email = s.trim().to_ascii_lowercase();
    if email.len() > 254
        || !email.contains('@')
        || email.contains(['\r', '\n', ' '])
        || email.parse::<lettre::Address>().is_err()
    {
        return Err(AppError::bad("Invalid email address"));
    }
    Ok(email)
}
fn check_password(s: &str) -> Result<()> {
    if !(12..=256).contains(&s.len()) {
        return Err(AppError::bad("Password must be 12..256 bytes"));
    }
    Ok(())
}
pub async fn hash_password(s: String, state: &AppState) -> Result<String> {
    check_password(&s)?;
    let permit = state
        .passwords
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::internal())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(s.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| AppError::internal())
    })
    .await
    .map_err(|_| AppError::internal())?
}
async fn verify_password(password: String, hash: String, state: &AppState) -> Result<bool> {
    if password.len() > 256 {
        return Ok(false);
    }
    let permit = state
        .passwords
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::internal())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .map_err(|_| AppError::internal())
}
pub async fn rate(state: &AppState, key: &str, limit: i32) -> Result<()> {
    let bucket = chrono::Utc::now().timestamp().div_euclid(60);
    let hits: i32 = sqlx::query_scalar("INSERT INTO rate_limits(key,bucket) VALUES($1,$2) ON CONFLICT(key,bucket) DO UPDATE SET hits=rate_limits.hits+1 RETURNING hits")
        .bind(crypto::hash(key)).bind(bucket).fetch_one(&state.db).await?;
    if hits > limit {
        return Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too many requests; try again in a minute",
        ));
    }
    Ok(())
}
pub async fn lock_user(conn: &mut PgConnection, user: Uuid) -> Result<String> {
    sqlx::query_scalar("SELECT timezone FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .fetch_one(conn)
        .await
        .map_err(Into::into)
}
pub async fn audit(
    conn: &mut PgConnection,
    user: Uuid,
    action: &str,
    entity: Option<Uuid>,
) -> Result<()> {
    sqlx::query("INSERT INTO audit_logs(user_id,action,entity_id) VALUES($1,$2,$3)")
        .bind(user)
        .bind(action)
        .bind(entity)
        .execute(conn)
        .await?;
    Ok(())
}
fn cookie_value(state: &AppState, token: &str, clear: bool) -> String {
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        state.config.cookie_name(),
        token,
        if clear { 0 } else { 1209600 },
        if state.config.secure_cookie {
            "; Secure"
        } else {
            ""
        }
    )
}
async fn session_response(state: &AppState, user: User) -> Result<Response> {
    let token = crypto::random_token();
    sqlx::query("INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '14 days')").bind(crypto::hash(&token)).bind(user.id).execute(&state.db).await?;
    let mut response = Json(
        json!({"user":user,"csrf_token":crypto::sign(&state.config.signing_key,"csrf",&token)}),
    )
    .into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie_value(state, &token, false))
            .map_err(|_| AppError::internal())?,
    );
    Ok(response)
}
pub async fn status(State(s): State<AppState>) -> Result<Json<Value>> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&s.db)
        .await?;
    Ok(Json(
        json!({"setup_required":count==0,"registration_allowed":s.config.allow_registration,"default_timezone":s.config.default_timezone}),
    ))
}
pub async fn setup(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    ApiJson(input): ApiJson<SetupInput>,
) -> Result<Response> {
    rate(&s, &format!("setup:{}", peer.ip()), 5).await?;
    if !crypto::equal(
        input.setup_token.as_deref().unwrap_or(""),
        &s.config.setup_token,
    ) {
        return Err(AppError::new(
            StatusCode::FORBIDDEN,
            "SETUP_TOKEN_INVALID",
            "Invalid bootstrap token",
        ));
    }
    create_user(s, input, true).await
}
pub async fn register(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    ApiJson(input): ApiJson<SetupInput>,
) -> Result<Response> {
    rate(&s, &format!("register:{}", peer.ip()), 3).await?;
    if !s.config.allow_registration {
        return Err(AppError::new(
            StatusCode::FORBIDDEN,
            "REGISTRATION_DISABLED",
            "Registration is disabled",
        ));
    }
    create_user(s, input, false).await
}
async fn create_user(s: AppState, input: SetupInput, setup: bool) -> Result<Response> {
    let email = normalize_email(&input.email)?;
    input
        .timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| AppError::bad("Use an IANA timezone"))?;
    let password = hash_password(input.password, &s).await?;
    let mut tx = s.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(61728199001)")
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&mut *tx)
        .await?;
    if setup && count != 0 || !setup && count == 0 {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "SETUP_STATE_CHANGED",
            "Review setup status and try again",
        ));
    }
    let user = User {
        id: Uuid::new_v4(),
        email,
        timezone: input.timezone,
        is_admin: setup,
    };
    let result = sqlx::query(
        "INSERT INTO users(id,email,password_hash,timezone,is_admin) VALUES($1,$2,$3,$4,$5)",
    )
    .bind(user.id)
    .bind(&user.email)
    .bind(password)
    .bind(&user.timezone)
    .bind(user.is_admin)
    .execute(&mut *tx)
    .await;
    if result.is_err() {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "ACCOUNT_UNAVAILABLE",
            "This account cannot be created",
        ));
    }
    sqlx::query("INSERT INTO notification_templates(id,user_id,data) VALUES($1,$2,$3)")
        .bind(Uuid::new_v4())
        .bind(user.id)
        .bind(json!(TemplateInput::default()))
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, user.id, "account_created", None).await?;
    tx.commit().await?;
    session_response(&s, user).await
}
pub async fn login(
    State(s): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    ApiJson(input): ApiJson<LoginInput>,
) -> Result<Response> {
    rate(&s, &format!("login:{}", peer.ip()), 20).await?;
    let email = normalize_email(&input.email)?;
    rate(&s, &format!("login-email:{email}"), 10).await?;
    let row =
        sqlx::query("SELECT id,email,password_hash,timezone,is_admin FROM users WHERE email=$1")
            .bind(email)
            .fetch_optional(&s.db)
            .await?;
    let Some(row) = row else {
        // Spend the same password-hashing work without retaining or logging the supplied password.
        let _ = hash_password(crypto::random_token(), &s).await?;
        return Err(AppError::unauthorized());
    };
    if !verify_password(input.password, row.try_get("password_hash")?, &s).await? {
        return Err(AppError::unauthorized());
    }
    let user = User {
        id: row.try_get("id")?,
        email: row.try_get("email")?,
        timezone: row.try_get("timezone")?,
        is_admin: row.try_get("is_admin")?,
    };
    let mut conn = s.db.acquire().await?;
    audit(&mut conn, user.id, "login", None).await?;
    drop(conn);
    session_response(&s, user).await
}
pub async fn session(State(s): State<AppState>, auth: Auth) -> Json<Value> {
    Json(
        json!({"user":auth.user,"csrf_token":crypto::sign(&s.config.signing_key,"csrf",&auth.token)}),
    )
}
pub async fn logout(State(s): State<AppState>, auth: Auth) -> Result<Response> {
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1 AND user_id=$2")
        .bind(crypto::hash(&auth.token))
        .bind(auth.user.id)
        .execute(&s.db)
        .await?;
    cleared(&s)
}
fn cleared(s: &AppState) -> Result<Response> {
    let mut r = Json(json!({"ok":true})).into_response();
    r.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie_value(s, "", true)).map_err(|_| AppError::internal())?,
    );
    Ok(r)
}
pub async fn account(
    State(s): State<AppState>,
    auth: Auth,
    ApiJson(input): ApiJson<AccountInput>,
) -> Result<Json<Value>> {
    input
        .timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| AppError::bad("Use an IANA timezone"))?;
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    sqlx::query("UPDATE users SET timezone=$1 WHERE id=$2")
        .bind(&input.timezone)
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    crate::planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "timezone_changed", None).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn password(
    State(s): State<AppState>,
    auth: Auth,
    ApiJson(input): ApiJson<PasswordInput>,
) -> Result<Response> {
    rate(&s, &format!("password:{}", auth.user.id), 5).await?;
    let old: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(auth.user.id)
        .fetch_one(&s.db)
        .await?;
    if !verify_password(input.current_password, old.clone(), &s).await? {
        return Err(AppError::unauthorized());
    }
    let new = hash_password(input.new_password, &s).await?;
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let result = sqlx::query("UPDATE users SET password_hash=$1 WHERE id=$2 AND password_hash=$3")
        .bind(new)
        .bind(auth.user.id)
        .bind(old)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::unauthorized());
    }
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, auth.user.id, "password_changed", None).await?;
    tx.commit().await?;
    cleared(&s)
}
pub async fn logout_all(State(s): State<AppState>, auth: Auth) -> Result<Response> {
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(auth.user.id)
        .execute(&s.db)
        .await?;
    cleared(&s)
}
