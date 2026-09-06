use crate::{AppState, auth, calendar, cards, crypto, error::{AppError, Result}, notifications};
use axum::{Router, extract::{DefaultBodyLimit, FromRequest, Request, State, MatchedPath}, http::{HeaderMap, HeaderValue, StatusCode, Method}, middleware::{self, Next}, response::{IntoResponse, Response}, routing::{get, post, patch}, Json};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sqlx::Row;
use std::time::Instant;
use tower_http::{compression::CompressionLayer, services::{ServeDir, ServeFile}};

pub struct ApiJson<T>(pub T);
impl<T> FromRequest<AppState> for ApiJson<T> where T: DeserializeOwned + Send {
    type Rejection = AppError;
    async fn from_request(req: Request, state: &AppState) -> Result<Self> {
        let Json(value) = Json::<T>::from_request(req, state).await.map_err(|rejection| {
            AppError::new(rejection.status(), "VALIDATION_ERROR", "Invalid JSON request; review field names, body size and value types")
        })?;
        Ok(Self(value))
    }
}
async fn not_found() -> AppError { AppError::missing() }
async fn live() -> Json<Value> { Json(json!({"status":"ok"})) }
async fn ready(State(s): State<AppState>) -> Response {
    match tokio::time::timeout(std::time::Duration::from_secs(3), sqlx::query_scalar::<_,i32>("SELECT 1").fetch_one(&s.db)).await {
        Ok(Ok(_)) => Json(json!({"status":"ready"})).into_response(),
        _ => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status":"database_unavailable"}))).into_response(),
    }
}
async fn openapi(_auth: auth::Auth) -> Json<Value> { Json(crate::spec::document()) }
async fn system(State(s): State<AppState>, auth: auth::Auth) -> Result<Json<Value>> {
    if !auth.user.is_admin { return Err(AppError::new(StatusCode::FORBIDDEN, "ADMIN_REQUIRED", "Administrator access required")); }
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_jobs WHERE status IN ('pending','retry','processing')").fetch_one(&s.db).await?;
    let rows = sqlx::query("SELECT role,last_success_at FROM worker_heartbeats ORDER BY role").fetch_all(&s.db).await?;
    let mut heartbeats = serde_json::Map::new();
    for row in rows {
        let role: String = row.try_get("role")?;
        let success: chrono::DateTime<chrono::Utc> = row.try_get("last_success_at")?;
        heartbeats.insert(role, json!({"last_success_at":success,"seconds_since_success":(chrono::Utc::now()-success).num_seconds()}));
    }
    Ok(Json(json!({"version":env!("CARGO_PKG_VERSION"),"role":s.config.role,"base_url":s.config.base_url,"secure_cookies":s.config.secure_cookie,"registration_allowed":s.config.allow_registration,"webpush_configured":s.config.vapid_private.is_some(),"pending_jobs":pending,"worker_heartbeats":heartbeats,"private_destination_exceptions":s.config.private_hosts,"insecure_notifications_allowed":s.config.allow_insecure_notifications})))
}
async fn audit_log(State(s): State<AppState>, auth: auth::Auth) -> Result<Json<Value>> {
    let rows = sqlx::query("SELECT action,entity_id,created_at FROM audit_logs WHERE user_id=$1 ORDER BY id DESC LIMIT 100").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut out = Vec::new();
    for r in rows {
        out.push(json!({"action":r.try_get::<String,_>("action")?,"entity_id":r.try_get::<Option<uuid::Uuid>,_>("entity_id")?,"created_at":r.try_get::<chrono::DateTime<chrono::Utc>,_>("created_at")?}));
    }
    Ok(Json(json!(out)))
}
fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let first = values.next()?.to_str().ok()?;
    if values.next().is_some() { return None; }
    Some(first)
}
pub async fn security(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let started = Instant::now();
    let path = req.uri().path().to_owned();
    let method = req.method().clone();
    // Log matched patterns only, never the raw URI, calendar token or provider secret.
    let route = req.extensions().get::<MatchedPath>().map(|v|v.as_str().to_string()).unwrap_or_else(||"unmatched".into());
    let unsafe_method = ![Method::GET, Method::HEAD, Method::OPTIONS].contains(&method);
    let rejection = if unsafe_method && path.starts_with("/api/") {
        let origin = single_header(req.headers(), "origin");
        if origin != Some(s.config.base_url.as_str()) {
            Some(AppError::new(StatusCode::FORBIDDEN, "ORIGIN_REJECTED", "Use exactly one Origin header matching the configured application origin"))
        } else if !["/api/v1/auth/login", "/api/v1/auth/setup", "/api/v1/auth/register"].contains(&path.as_str()) {
            let token = auth::cookie(req.headers(), s.config.cookie_name()).unwrap_or_default();
            let csrf = single_header(req.headers(), "x-csrf-token").unwrap_or("");
            if token.is_empty() || !crypto::equal(csrf, &crypto::sign(&s.config.signing_key, "csrf", &token)) {
                Some(AppError::new(StatusCode::FORBIDDEN, "CSRF_REJECTED", "Refresh the page and try again"))
            } else { None }
        } else { None }
    } else { None };
    let mut response = if let Some(error) = rejection { error.into_response() } else { next.run(req).await };
    let json_response = response.headers().get("content-type").and_then(|h|h.to_str().ok()).is_some_and(|h|h.starts_with("application/json"));
    if path.starts_with("/api/") && response.status().is_client_error() && !json_response {
        let status = response.status();
        let code = match status {
            StatusCode::NOT_FOUND => "NOT_FOUND",
            StatusCode::METHOD_NOT_ALLOWED => "METHOD_NOT_ALLOWED",
            StatusCode::PAYLOAD_TOO_LARGE => "PAYLOAD_TOO_LARGE",
            _ => "VALIDATION_ERROR",
        };
        response = AppError::new(status, code, "Request path, method or body is invalid").into_response();
    }
    let id = response.headers().get("x-request-id").cloned().unwrap_or_else(||HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).expect("UUID header"));
    response.headers_mut().insert("x-request-id", id.clone());
    response.headers_mut().insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    response.headers_mut().insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response.headers_mut().insert("x-frame-options", HeaderValue::from_static("DENY"));
    response.headers_mut().insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"));
    if s.config.secure_cookie { response.headers_mut().insert("strict-transport-security", HeaderValue::from_static("max-age=31536000")); }
    if path.starts_with("/api/") || path.starts_with("/cal/") {
        if !response.headers().contains_key("cache-control") { response.headers_mut().insert("cache-control", HeaderValue::from_static("no-store")); }
    } else if path.starts_with("/_app/immutable/") {
        response.headers_mut().insert("cache-control", HeaderValue::from_static("public, max-age=31536000, immutable"));
    } else { response.headers_mut().insert("cache-control", HeaderValue::from_static("no-cache")); }
    if path == "/sw.js" { response.headers_mut().insert("service-worker-allowed", HeaderValue::from_static("/")); }
    tracing::info!(request_id=?id, method=%method, route=%route, status=response.status().as_u16(), latency_ms=started.elapsed().as_millis(), "http request");
    response
}
pub fn router(s: AppState) -> Router {
    let api = Router::new()
        .route("/auth/status", get(auth::status)).route("/auth/setup", post(auth::setup)).route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login)).route("/auth/logout", post(auth::logout)).route("/auth/logout-all", post(auth::logout_all)).route("/auth/session", get(auth::session))
        .route("/account", patch(auth::account)).route("/account/password", post(auth::password))
        .route("/dashboard", get(cards::dashboard)).route("/cards", get(cards::list).post(cards::create))
        .route("/cards/{id}", get(cards::get).patch(cards::update).delete(cards::archive)).route("/cards/{id}/restore", post(cards::restore))
        .route("/cards/{card}/cycles", get(cards::cycles)).route("/cards/{card}/cycles/{cycle}", patch(cards::patch_cycle))
        .route("/cards/{card}/cycles/{cycle}/pay", post(cards::pay).delete(cards::unpay))
        .route("/milestones", get(cards::milestones).post(cards::create_milestone)).route("/milestones/{id}", patch(cards::update_milestone).delete(cards::delete_milestone))
        .route("/calendar/feeds", get(calendar::list).post(calendar::create)).route("/calendar/feeds/{id}", patch(calendar::update).delete(calendar::delete))
        .route("/calendar/feeds/{id}/rotate-token", post(calendar::rotate))
        .route("/notification/connections", get(notifications::connections).post(notifications::create_connection))
        .route("/notification/connections/{id}", patch(notifications::update_connection).delete(notifications::delete_connection))
        .route("/notification/connections/{id}/test", post(notifications::test_connection)).route("/notification/webpush/key", get(notifications::vapid))
        .route("/notification/templates", get(notifications::templates).post(notifications::create_template))
        .route("/notification/templates/{id}", patch(notifications::update_template).delete(notifications::delete_template))
        .route("/notification/templates/preview", post(notifications::preview)).route("/notification/templates/validate", post(notifications::preview))
        .route("/notification/rules", get(notifications::rules).post(notifications::create_rule))
        .route("/notification/rules/{id}", patch(notifications::update_rule).delete(notifications::delete_rule))
        .route("/notification/deliveries", get(notifications::history)).route("/notification/deliveries/{id}/attempts", get(notifications::attempts))
        .route("/notification/deliveries/{id}/retry", post(notifications::retry))
        .route("/settings/system", get(system)).route("/settings/audit", get(audit_log)).route("/openapi.json", get(openapi)).fallback(not_found);
    let fallback = ServeDir::new(&s.config.frontend_dir).fallback(ServeFile::new(format!("{}/200.html", s.config.frontend_dir)));
    Router::new().nest("/api/v1", api)
        .nest("/cal", Router::new().route("/v1/{id}/{token}", get(calendar::subscribe)).fallback(not_found))
        .route("/health/live", get(live)).route("/health/ready", get(ready))
        .fallback_service(fallback).layer(DefaultBodyLimit::max(128*1024)).layer(CompressionLayer::new())
        .layer(middleware::from_fn_with_state(s.clone(), security)).with_state(s)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_security_headers_are_rejected() {
        let mut headers=HeaderMap::new();
        headers.append("origin",HeaderValue::from_static("https://card.example"));
        assert_eq!(single_header(&headers,"origin"),Some("https://card.example"));
        headers.append("origin",HeaderValue::from_static("https://attacker.example"));
        assert_eq!(single_header(&headers,"origin"),None);
    }
}
