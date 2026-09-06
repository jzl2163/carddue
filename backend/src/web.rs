use crate::{AppState,auth,calendar,cards,crypto,error::{AppError,Result},notifications};
use axum::{Router,body::Body,extract::{DefaultBodyLimit,FromRequest,Request,State,MatchedPath},http::{HeaderValue,StatusCode,Method},middleware::{self,Next},response::{IntoResponse,Response},routing::{get,post,patch},Json};
use serde::de::DeserializeOwned;
use serde_json::{Value,json};
use sqlx::Row;
use std::time::Instant;
use tower_http::{compression::CompressionLayer,services::{ServeDir,ServeFile}};

pub struct ApiJson<T>(pub T);
impl<T> FromRequest<AppState> for ApiJson<T> where T:DeserializeOwned+Send {
    type Rejection=AppError;
    async fn from_request(req:Request,state:&AppState)->Result<Self>{
        let Json(value)=Json::<T>::from_request(req,state).await.map_err(|_|AppError::bad("Invalid JSON request; review field names and value types"))?;Ok(Self(value))
    }
}
async fn not_found()->AppError{AppError::missing()}
async fn live()->Json<Value>{Json(json!({"status":"ok"}))}
async fn ready(State(s):State<AppState>)->Response{
    match tokio::time::timeout(std::time::Duration::from_secs(3),sqlx::query_scalar::<_,i32>("SELECT 1").fetch_one(&s.db)).await{
        Ok(Ok(_))=>Json(json!({"status":"ready"})).into_response(),
        _=>(StatusCode::SERVICE_UNAVAILABLE,Json(json!({"status":"database_unavailable"}))).into_response()
    }
}
async fn openapi(_auth:auth::Auth)->Json<Value>{Json(crate::spec::document())}
async fn system(State(s):State<AppState>,auth:auth::Auth)->Result<Json<Value>>{
    if !auth.user.is_admin{return Err(AppError::new(StatusCode::FORBIDDEN,"ADMIN_REQUIRED","Administrator access required"));}
    let pending:i64=sqlx::query_scalar("SELECT count(*) FROM notification_jobs WHERE status IN ('pending','retry','processing')").fetch_one(&s.db).await?;
    Ok(Json(json!({"version":env!("CARGO_PKG_VERSION"),"role":s.config.role,"base_url":s.config.base_url,"secure_cookies":s.config.secure_cookie,"registration_allowed":s.config.allow_registration,"webpush_configured":s.config.vapid_private.is_some(),"pending_jobs":pending,"private_destination_exceptions":s.config.private_hosts,"insecure_notifications_allowed":s.config.allow_insecure_notifications})))
}
async fn audit_log(State(s):State<AppState>,auth:auth::Auth)->Result<Json<Value>>{
    let rows=sqlx::query("SELECT action,entity_id,created_at FROM audit_logs WHERE user_id=$1 ORDER BY id DESC LIMIT 100").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut out=Vec::new();for r in rows{out.push(json!({"action":r.try_get::<String,_>("action")?,"entity_id":r.try_get::<Option<uuid::Uuid>,_>("entity_id")?,"created_at":r.try_get::<chrono::DateTime<chrono::Utc>,_>("created_at")?}));}Ok(Json(json!(out)))
}
pub async fn security(State(s):State<AppState>,req:Request,next:Next)->Response{
    let started=Instant::now();let path=req.uri().path().to_owned();let method=req.method().clone();
    // Only matched route patterns are logged: never query strings, calendar tokens or provider credentials.
    let route=req.extensions().get::<MatchedPath>().map(|v|v.as_str().to_string()).unwrap_or_else(||"unmatched".into());
    let unsafe_method=![Method::GET,Method::HEAD,Method::OPTIONS].contains(&method);
    let rejection=if unsafe_method && path.starts_with("/api/") {
        let origin=req.headers().get("origin").and_then(|v|v.to_str().ok()).unwrap_or("");
        if origin!=s.config.base_url{Some(AppError::new(StatusCode::FORBIDDEN,"ORIGIN_REJECTED","Use the configured application origin"))}
        else if !["/api/v1/auth/login","/api/v1/auth/setup","/api/v1/auth/register"].contains(&path.as_str()){
            let token=auth::cookie(req.headers(),s.config.cookie_name()).unwrap_or_default();
            let csrf=req.headers().get("x-csrf-token").and_then(|v|v.to_str().ok()).unwrap_or("");
            if token.is_empty() || !crypto::equal(csrf,&crypto::sign(&s.config.signing_key,"csrf",&token)){Some(AppError::new(StatusCode::FORBIDDEN,"CSRF_REJECTED","Refresh the page and try again"))}else{None}
        }else{None}
    }else{None};
    let mut response=if let Some(error)=rejection{error.into_response()}else{next.run(req).await};
    let id=response.headers().get("x-request-id").cloned().unwrap_or_else(||HeaderValue::from_str(&uuid::Uuid::new_v4().to_string()).expect("UUID header"));
    response.headers_mut().insert("x-request-id",id.clone());
    response.headers_mut().insert("x-content-type-options",HeaderValue::from_static("nosniff"));
    response.headers_mut().insert("referrer-policy",HeaderValue::from_static("no-referrer"));
    response.headers_mut().insert("x-frame-options",HeaderValue::from_static("DENY"));
    response.headers_mut().insert("permissions-policy",HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"));
    if s.config.secure_cookie{response.headers_mut().insert("strict-transport-security",HeaderValue::from_static("max-age=31536000"));}
    if path.starts_with("/api/") || path.starts_with("/cal/"){
        if !response.headers().contains_key("cache-control"){response.headers_mut().insert("cache-control",HeaderValue::from_static("no-store"));}
    }else if path.starts_with("/_app/immutable/"){
        response.headers_mut().insert("cache-control",HeaderValue::from_static("public, max-age=31536000, immutable"));
    }else{response.headers_mut().insert("cache-control",HeaderValue::from_static("no-cache"));}
    if path=="/sw.js"{response.headers_mut().insert("service-worker-allowed",HeaderValue::from_static("/"));}
    tracing::info!(request_id=?id,method=%method,route=%route,status=response.status().as_u16(),latency_ms=started.elapsed().as_millis(),"http request");
    response
}
pub fn router(s:AppState)->Router{
    let api=Router::new()
        .route("/auth/status",get(auth::status)).route("/auth/setup",post(auth::setup)).route("/auth/register",post(auth::register))
        .route("/auth/login",post(auth::login)).route("/auth/logout",post(auth::logout)).route("/auth/logout-all",post(auth::logout_all)).route("/auth/session",get(auth::session))
        .route("/account",patch(auth::account)).route("/account/password",post(auth::password))
        .route("/dashboard",get(cards::dashboard)).route("/cards",get(cards::list).post(cards::create))
        .route("/cards/{id}",get(cards::get).patch(cards::update).delete(cards::archive)).route("/cards/{id}/restore",post(cards::restore))
        .route("/cards/{card}/cycles",get(cards::cycles)).route("/cards/{card}/cycles/{cycle}",patch(cards::patch_cycle))
        .route("/cards/{card}/cycles/{cycle}/pay",post(cards::pay).delete(cards::unpay))
        .route("/milestones",get(cards::milestones).post(cards::create_milestone)).route("/milestones/{id}",patch(cards::update_milestone).delete(cards::delete_milestone))
        .route("/calendar/feeds",get(calendar::list).post(calendar::create)).route("/calendar/feeds/{id}",patch(calendar::update).delete(calendar::delete))
        .route("/calendar/feeds/{id}/rotate-token",post(calendar::rotate))
        .route("/notification/connections",get(notifications::connections).post(notifications::create_connection))
        .route("/notification/connections/{id}",patch(notifications::update_connection).delete(notifications::delete_connection))
        .route("/notification/connections/{id}/test",post(notifications::test_connection)).route("/notification/webpush/key",get(notifications::vapid))
        .route("/notification/templates",get(notifications::templates).post(notifications::create_template))
        .route("/notification/templates/{id}",patch(notifications::update_template).delete(notifications::delete_template))
        .route("/notification/templates/preview",post(notifications::preview)).route("/notification/templates/validate",post(notifications::preview))
        .route("/notification/rules",get(notifications::rules).post(notifications::create_rule))
        .route("/notification/rules/{id}",patch(notifications::update_rule).delete(notifications::delete_rule))
        .route("/notification/deliveries",get(notifications::history)).route("/notification/deliveries/{id}/attempts",get(notifications::attempts))
        .route("/notification/deliveries/{id}/retry",post(notifications::retry))
        .route("/settings/system",get(system)).route("/settings/audit",get(audit_log)).route("/openapi.json",get(openapi)).fallback(not_found);
    let fallback=ServeDir::new(&s.config.frontend_dir).fallback(ServeFile::new(format!("{}/200.html",s.config.frontend_dir)));
    Router::new().nest("/api/v1",api)
        .nest("/cal",Router::new().route("/v1/{id}/{token}",get(calendar::subscribe)).fallback(not_found))
        .route("/health/live",get(live)).route("/health/ready",get(ready))
        .fallback_service(fallback).layer(DefaultBodyLimit::max(128*1024)).layer(CompressionLayer::new())
        .layer(middleware::from_fn_with_state(s.clone(),security)).with_state(s)
}
