use axum::{
    Json, Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
    response::IntoResponse,
    routing::post,
};
use carddue::{AppState, StateInner, config::Config, crypto, dates, worker};
use chrono::{Datelike, Utc};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "http://localhost:8080";
const PASSWORD: &str = "integration-test-password-only";
#[derive(Clone)]
struct Session {
    cookie: String,
    csrf: String,
    user: Uuid,
}
fn state(db: PgPool) -> AppState {
    Arc::new(StateInner {
        db,
        passwords: Arc::new(tokio::sync::Semaphore::new(4)),
        config: Config {
            database_url: String::new(),
            base_url: ORIGIN.into(),
            bind: "127.0.0.1:8080".parse().unwrap(),
            role: "all".into(),
            secure_cookie: false,
            encryption_key: [21; 32],
            signing_key: [22; 32],
            setup_token: "integration-bootstrap-token-32-chars".into(),
            allow_registration: true,
            private_hosts: HashSet::from(["127.0.0.1".into()]),
            allow_insecure_notifications: true,
            vapid_private: None,
            vapid_subject: "mailto:tests@example.invalid".into(),
            frontend_dir: "/nonexistent-test-assets".into(),
            default_timezone: "UTC".into(),
        },
    })
}
async fn request(
    s: &AppState,
    method: &str,
    path: &str,
    body: Option<Value>,
    auth: Option<&Session>,
    extra: &[(&str, &str)],
) -> (StatusCode, HeaderMap, String) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("origin", ORIGIN)
        .extension(ConnectInfo::<SocketAddr>(
            "127.0.0.1:32123".parse().unwrap(),
        ));
    if let Some(auth) = auth {
        req = req
            .header("cookie", &auth.cookie)
            .header("x-csrf-token", &auth.csrf);
    }
    for (key, value) in extra {
        req = req.header(*key, *value);
    }
    let body = if let Some(body) = body {
        req = req.header("content-type", "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = carddue::web::router(s.clone())
        .oneshot(req.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let text = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    (status, headers, text)
}
async fn ok(
    s: &AppState,
    method: &str,
    path: &str,
    body: Option<Value>,
    auth: Option<&Session>,
) -> Value {
    let (status, _, body) = request(s, method, path, body, auth, &[]).await;
    assert_eq!(status, StatusCode::OK, "{method} {path}: {body}");
    serde_json::from_str(&body).unwrap()
}
async fn account(s: &AppState, email: &str, setup: bool) -> Session {
    let body = json!({"email":email,"password":PASSWORD,"timezone":"UTC","setup_token":s.config.setup_token});
    let (status, headers, text) = request(
        s,
        "POST",
        if setup {
            "/api/v1/auth/setup"
        } else {
            "/api/v1/auth/register"
        },
        Some(body),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let data: Value = serde_json::from_str(&text).unwrap();
    let cookie = headers
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    Session {
        cookie,
        csrf: data["csrf_token"].as_str().unwrap().into(),
        user: Uuid::parse_str(data["user"]["id"].as_str().unwrap()).unwrap(),
    }
}
fn card(name: &str) -> Value {
    json!({"name":name,"statement_day":1,"due_mode":"fixed_day","due_day":25,"due_month_offset":0,"last4":"1234","currency":"CNY","color":"#2563eb"})
}
async fn add_card(s: &AppState, a: &Session, name: &str) -> Uuid {
    Uuid::parse_str(
        ok(s, "POST", "/api/v1/cards", Some(card(name)), Some(a)).await["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}
async fn connection(s: &AppState, a: &Session, kind: &str, url: &str) -> Uuid {
    let config = if kind == "bark" {
        json!({"base_url":url,"device_key":"mock-device-key"})
    } else {
        json!({"url":url,"bearer_token":"mock-private-bearer"})
    };
    let value = ok(
        s,
        "POST",
        "/api/v1/notification/connections",
        Some(json!({"name":"Test connection","kind":kind,"enabled":true,"config":config})),
        Some(a),
    )
    .await;
    Uuid::parse_str(value["id"].as_str().unwrap()).unwrap()
}
async fn mock_server(first_limit: bool) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let handler = move |Json(body): Json<Value>| {
        let counter = counter.clone();
        async move {
            assert!(body.is_object());
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if first_limit && n == 0 {
                (StatusCode::TOO_MANY_REQUESTS, Json(json!({"code":429}))).into_response()
            } else {
                (StatusCode::OK, Json(json!({"code":200,"accepted":true}))).into_response()
            }
        }
    };
    let app = Router::new()
        .route("/", post(handler.clone()))
        .route("/push", post(handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), count, task)
}
#[sqlx::test(migrations = "./migrations")]
async fn auth_csrf_and_tenant_isolation(db: PgPool) {
    let s = state(db);
    let alice = account(&s, "alice@example.com", true).await;
    let bob = account(&s, "bob@example.com", false).await;
    let id = add_card(&s, &alice, "Alice private card").await;
    let (status, _, _) = request(
        &s,
        "GET",
        &format!("/api/v1/cards/{id}"),
        None,
        Some(&bob),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = request(&s, "GET", "/api/v1/cards", None, None, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let broken = Session {
        csrf: "invalid".into(),
        ..alice.clone()
    };
    let (status, _, _) = request(
        &s,
        "POST",
        "/api/v1/cards",
        Some(card("CSRF attack")),
        Some(&broken),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = request(
        &s,
        "POST",
        "/api/v1/cards",
        Some(card("foreign origin")),
        Some(&alice),
        &[("origin", "https://attacker.invalid")],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, body) = request(
        &s,
        "POST",
        "/api/v1/cards",
        Some(json!({"name":"invalid","statement_day":0,"due_day":25})),
        Some(&alice),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("VALIDATION_ERROR"));
    ok(&s, "POST", "/api/v1/auth/logout-all", None, Some(&alice)).await;
    let (status, _, _) = request(&s, "GET", "/api/v1/auth/session", None, Some(&alice), &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let rows = ok(&s, "GET", "/api/v1/cards", None, Some(&bob)).await;
    assert!(rows.as_array().unwrap().is_empty());
}
#[sqlx::test(migrations = "./migrations")]
async fn initialization_is_atomic(db: PgPool) {
    let s = state(db);
    let first = json!({"email":"one@example.com","password":PASSWORD,"timezone":"UTC","setup_token":s.config.setup_token});
    let second = json!({"email":"two@example.com","password":PASSWORD,"timezone":"UTC","setup_token":s.config.setup_token});
    let (a, b) = tokio::join!(
        request(&s, "POST", "/api/v1/auth/setup", Some(first), None, &[]),
        request(&s, "POST", "/api/v1/auth/setup", Some(second), None, &[])
    );
    let statuses = [a.0, b.0];
    assert_eq!(statuses.iter().filter(|s| **s == StatusCode::OK).count(), 1);
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
#[sqlx::test(migrations = "./migrations")]
async fn calendar_privacy_stable_uid_etag_and_rotation(db: PgPool) {
    let s = state(db);
    let a = account(&s, "calendar@example.com", true).await;
    let card_a = add_card(&s, &a, "VISIBLE-CARD").await;
    let card_b = add_card(&s, &a, "NEVER-PUBLISH-THIS-CARD").await;
    let feed = json!({"name":"Selected card","privacy":"normal","card_ids":[card_a],"kinds":["payment_due"],"alarms_days_before":[1,0]});
    let f = ok(
        &s,
        "POST",
        "/api/v1/calendar/feeds",
        Some(feed.clone()),
        Some(&a),
    )
    .await;
    let path = f["url"].as_str().unwrap().strip_prefix(ORIGIN).unwrap();
    let (status, headers, body) = request(&s, "GET", path, None, None, &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("VISIBLE-CARD"));
    assert!(!body.contains("NEVER-PUBLISH-THIS-CARD"));
    assert!(!body.contains("1234"));
    assert!(!body.contains("calendar@example.com"));
    assert!(!body.contains("STATUS:CANCELLED"));
    let etag = headers.get("etag").unwrap().to_str().unwrap();
    let (status, _, body) = request(&s, "GET", path, None, None, &[("if-none-match", etag)]).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
    let next = dates::month(Utc::now().date_naive(), 1);
    let cycle: Uuid =
        sqlx::query_scalar("SELECT id FROM card_cycles WHERE card_id=$1 AND cycle_month=$2")
            .bind(card_a)
            .bind(next)
            .fetch_one(&s.db)
            .await
            .unwrap();
    let event: Uuid = sqlx::query_scalar(
        "SELECT id FROM calendar_events WHERE cycle_id=$1 AND kind='payment_due'",
    )
    .bind(cycle)
    .fetch_one(&s.db)
    .await
    .unwrap();
    let new_due = dates::day(next.year(), next.month(), 26);
    ok(
        &s,
        "PATCH",
        &format!("/api/v1/cards/{card_a}/cycles/{cycle}"),
        Some(json!({"due_date":new_due,"amount":"1234.56"})),
        Some(&a),
    )
    .await;
    let (status, new_headers, body) =
        request(&s, "GET", path, None, None, &[("if-none-match", etag)]).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(new_headers.get("etag"), headers.get("etag"));
    assert!(!body.contains("1234.56"));
    let block = body
        .split("BEGIN:VEVENT")
        .find(|b| b.contains(&event.to_string()))
        .unwrap();
    assert!(block.contains("SEQUENCE:1"));
    assert!(block.contains(&format!("DTSTART;VALUE=DATE:{}", new_due.format("%Y%m%d"))));
    let mut restricted = feed;
    restricted["card_ids"] = json!([card_b]);
    restricted["privacy"] = json!("private");
    ok(
        &s,
        "PATCH",
        &format!("/api/v1/calendar/feeds/{}", f["id"].as_str().unwrap()),
        Some(restricted),
        Some(&a),
    )
    .await;
    let (_, _, body) = request(&s, "GET", path, None, None, &[]).await;
    assert!(!body.contains("VISIBLE-CARD"));
    assert!(!body.contains("NEVER-PUBLISH-THIS-CARD"));
    assert!(body.contains("STATUS:CANCELLED"));
    assert!(body.contains("CardDue event removed"));
    let rotated = ok(
        &s,
        "POST",
        &format!(
            "/api/v1/calendar/feeds/{}/rotate-token",
            f["id"].as_str().unwrap()
        ),
        None,
        Some(&a),
    )
    .await;
    let (status, _, _) = request(&s, "GET", path, None, None, &[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let new_path = rotated["url"]
        .as_str()
        .unwrap()
        .strip_prefix(ORIGIN)
        .unwrap();
    assert_eq!(
        request(&s, "GET", new_path, None, None, &[]).await.0,
        StatusCode::OK
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn paid_cancels_claimed_jobs_before_send(db: PgPool) {
    let s = state(db);
    let a = account(&s, "paid@example.com", true).await;
    let card = add_card(&s, &a, "Payment card").await;
    let (url, count, server) = mock_server(false).await;
    let connection = connection(&s, &a, "webhook", &url).await;
    let templates = ok(&s, "GET", "/api/v1/notification/templates", None, Some(&a)).await;
    let rule=ok(&s,"POST","/api/v1/notification/rules",Some(json!({"name":"Pay reminders","event_kind":"payment_due","card_ids":[card],"offsets":[0,1],"local_time":"09:00","connection_ids":[connection],"template_id":templates[0]["id"],"enabled":true})),Some(&a)).await;
    assert!(rule["id"].is_string());
    let next = dates::month(Utc::now().date_naive(), 1);
    let cycle: Uuid =
        sqlx::query_scalar("SELECT id FROM card_cycles WHERE card_id=$1 AND cycle_month=$2")
            .bind(card)
            .bind(next)
            .fetch_one(&s.db)
            .await
            .unwrap();
    let event: Uuid = sqlx::query_scalar(
        "SELECT id FROM calendar_events WHERE cycle_id=$1 AND kind='payment_due'",
    )
    .bind(cycle)
    .fetch_one(&s.db)
    .await
    .unwrap();
    let jobs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM notification_jobs WHERE event_id=$1 AND status='pending'",
    )
    .bind(event)
    .fetch_all(&s.db)
    .await
    .unwrap();
    assert_eq!(jobs.len(), 2);
    let owner = Uuid::new_v4();
    sqlx::query("UPDATE notification_jobs SET status='processing',lease_owner=$1,lease_until=now()+interval '1 minute' WHERE id=$2").bind(owner).bind(jobs[0]).execute(&s.db).await.unwrap();
    ok(
        &s,
        "POST",
        &format!("/api/v1/cards/{card}/cycles/{cycle}/pay"),
        None,
        Some(&a),
    )
    .await;
    worker::deliver(&s, a.user, jobs[0], owner).await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let cancelled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_jobs WHERE event_id=$1 AND status='cancelled'",
    )
    .bind(event)
    .fetch_one(&s.db)
    .await
    .unwrap();
    assert_eq!(cancelled, 2);
    ok(
        &s,
        "DELETE",
        &format!("/api/v1/cards/{card}/cycles/{cycle}/pay"),
        None,
        Some(&a),
    )
    .await;
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_jobs WHERE event_id=$1 AND status='pending'",
    )
    .bind(event)
    .fetch_one(&s.db)
    .await
    .unwrap();
    assert_eq!(pending, 2);
    server.abort();
}
#[sqlx::test(migrations = "./migrations")]
async fn encrypted_connection_real_mock_delivery_and_retry(db: PgPool) {
    let s = state(db);
    let a = account(&s, "delivery@example.com", true).await;
    let (url, count, server) = mock_server(true).await;
    let c = connection(&s, &a, "bark", &url).await;
    let cipher: String =
        sqlx::query_scalar("SELECT encrypted_config FROM notification_connections WHERE id=$1")
            .bind(c)
            .fetch_one(&s.db)
            .await
            .unwrap();
    assert!(!cipher.contains("mock-device-key"));
    let listed = ok(
        &s,
        "GET",
        "/api/v1/notification/connections",
        None,
        Some(&a),
    )
    .await;
    assert!(!listed.to_string().contains("mock-device-key"));
    let result = ok(
        &s,
        "POST",
        &format!("/api/v1/notification/connections/{c}/test"),
        Some(json!({})),
        Some(&a),
    )
    .await;
    let id = Uuid::parse_str(result["id"].as_str().unwrap()).unwrap();
    worker::tick(&s, Uuid::new_v4()).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM notification_jobs WHERE id=$1")
        .bind(id)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(status, "retry");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    sqlx::query("UPDATE notification_jobs SET next_attempt_at=now() WHERE id=$1")
        .bind(id)
        .execute(&s.db)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        worker::tick(&s, Uuid::new_v4()),
        worker::tick(&s, Uuid::new_v4())
    );
    left.unwrap();
    right.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM notification_jobs WHERE id=$1")
        .bind(id)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(status, "sent");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery_attempts WHERE job_id=$1")
            .bind(id)
            .fetch_one(&s.db)
            .await
            .unwrap();
    assert_eq!(attempts, 2);
    let (status, _, _) = request(
        &s,
        "POST",
        &format!("/api/v1/notification/deliveries/{id}/retry"),
        None,
        Some(&a),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        crypto::decrypt(
            &s.config.encryption_key,
            &format!("{}/{c}/bark", a.user),
            &cipher
        )
        .is_ok()
    );
    server.abort();
}
#[sqlx::test(migrations = "./migrations")]
async fn invalid_recurrence_templates_and_cross_tenant_references(db: PgPool) {
    let s = state(db);
    let a = account(&s, "rules-a@example.com", true).await;
    let b = account(&s, "rules-b@example.com", false).await;
    let card = add_card(&s, &a, "Private").await;
    let (status,_,_)=request(&s,"POST","/api/v1/milestones",Some(json!({"card_id":card,"title":"Not yours","kind":"custom","recurrence":"monthly","start_date":"2026-09-01"})),Some(&b),&[]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = request(
        &s,
        "POST",
        "/api/v1/calendar/feeds",
        Some(json!({"name":"foreign","card_ids":[card]})),
        Some(&b),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = request(
        &s,
        "POST",
        "/api/v1/notification/templates",
        Some(json!({"name":"bad","title":"{{ missing }}","body":"body"})),
        Some(&a),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status,_,_)=request(&s,"POST","/api/v1/notification/connections",Some(json!({"name":"metadata","kind":"webhook","config":{"url":"https://169.254.169.254/latest/meta-data/"}})),Some(&a),&[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn registration_switch_duplicate_and_non_admin(db: PgPool) {
    let mut s = state(db);
    account(&s, "owner@example.com", true).await;
    let user = account(&s, "new@example.com", false).await;
    let session = ok(&s, "GET", "/api/v1/auth/session", None, Some(&user)).await;
    assert_eq!(session["user"]["is_admin"], false);
    let input = json!({"email":"new@example.com","password":PASSWORD,"timezone":"UTC"});
    let (status, _, _) = request(
        &s,
        "POST",
        "/api/v1/auth/register",
        Some(input.clone()),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    Arc::get_mut(&mut s).unwrap().config.allow_registration = false;
    let (status, _, _) = request(&s, "POST", "/api/v1/auth/register", Some(input), None, &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn card_delete_preserves_cancellations_and_isolation(db: PgPool) {
    let s = state(db);
    let alice = account(&s, "delete-a@example.com", true).await;
    let bob = account(&s, "delete-b@example.com", false).await;
    let id = add_card(&s, &alice, "TO-DELETE").await;
    let other = add_card(&s, &alice, "DO-NOT-PUBLISH").await;
    let feed = ok(
        &s,
        "POST",
        "/api/v1/calendar/feeds",
        Some(json!({"name":"only target","card_ids":[id]})),
        Some(&alice),
    )
    .await;
    let path = feed["url"].as_str().unwrap().strip_prefix(ORIGIN).unwrap();
    let (_, headers, body) = request(&s, "GET", path, None, None, &[]).await;
    assert!(body.contains("TO-DELETE"));
    let endpoint = format!("/api/v1/cards/{id}/permanent");
    assert_eq!(
        request(&s, "DELETE", &endpoint, None, Some(&bob), &[])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // A claimed task must not survive deletion and later send.
    let conn = connection(&s, &alice, "bark", "http://127.0.0.1:9").await;
    let event: Uuid = sqlx::query_scalar("SELECT id FROM calendar_events WHERE card_id=$1 LIMIT 1")
        .bind(id)
        .fetch_one(&s.db)
        .await
        .unwrap();
    let owner = Uuid::new_v4();
    let job = Uuid::new_v4();
    sqlx::query("INSERT INTO notification_jobs(id,user_id,event_id,connection_id,scheduled_at,next_attempt_at,expires_at,status,lease_owner,lease_until,template_snapshot,context) VALUES($1,$2,$3,$4,now(),now(),now()+interval '1 day','processing',$5,now()+interval '1 hour','{}','{}')")
        .bind(job).bind(alice.user).bind(event).bind(conn).bind(owner).execute(&s.db).await.unwrap();
    ok(&s, "DELETE", &endpoint, None, Some(&alice)).await;
    worker::deliver(&s, alice.user, job, owner).await.unwrap();
    assert_eq!(
        request(
            &s,
            "GET",
            &format!("/api/v1/cards/{id}"),
            None,
            Some(&alice),
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &s,
            "POST",
            &format!("/api/v1/cards/{id}/restore"),
            None,
            Some(&alice),
            &[]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_jobs WHERE id=$1")
        .bind(job)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(n, 0);
    let (_, new_headers, body) = request(&s, "GET", path, None, None, &[]).await;
    assert!(body.contains("STATUS:CANCELLED"));
    assert!(!body.contains("TO-DELETE"));
    assert!(!body.contains("DO-NOT-PUBLISH"));
    assert_ne!(headers["etag"], new_headers["etag"]);
    assert_eq!(
        request(
            &s,
            "GET",
            &format!("/api/v1/cards/{other}"),
            None,
            Some(&alice),
            &[]
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn per_card_timezone_reschedules_claimed_jobs_and_fallback(db: PgPool) {
    let s = state(db);
    let a = account(&s, "timezone@example.com", true).await;
    let mut c = card("NY card");
    c["timezone"] = json!("America/New_York");
    c["region"] = json!("US");
    let ny = Uuid::parse_str(
        ok(&s, "POST", "/api/v1/cards", Some(c.clone()), Some(&a)).await["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let fallback = add_card(&s, &a, "Account timezone").await;
    let invalid =
        json!({"name":"bad zone","statement_day":1,"due_day":25,"timezone":"Mars/Olympus"});
    assert_eq!(
        request(&s, "POST", "/api/v1/cards", Some(invalid), Some(&a), &[])
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let conn = connection(&s, &a, "bark", "http://127.0.0.1:9").await;
    let template: Uuid =
        sqlx::query_scalar("SELECT id FROM notification_templates WHERE user_id=$1")
            .bind(a.user)
            .fetch_one(&s.db)
            .await
            .unwrap();
    ok(&s,"POST","/api/v1/notification/rules",Some(json!({"name":"nine local","event_kind":"payment_due","offsets":[0],"local_time":"09:00","connection_ids":[conn],"template_id":template})),Some(&a)).await;
    let jan = chrono::NaiveDate::from_ymd_opt(Utc::now().year() + 1, 1, 25).unwrap();
    let query = "SELECT j.scheduled_at FROM notification_jobs j JOIN calendar_events e ON e.id=j.event_id WHERE e.card_id=$1 AND e.event_date=$2";
    let ny_time: chrono::DateTime<Utc> = sqlx::query_scalar(query)
        .bind(ny)
        .bind(jan)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(ny_time, jan.and_hms_opt(14, 0, 0).unwrap().and_utc());
    ok(
        &s,
        "PATCH",
        "/api/v1/account",
        Some(json!({"timezone":"Asia/Shanghai"})),
        Some(&a),
    )
    .await;
    let fallback_time: chrono::DateTime<Utc> = sqlx::query_scalar(query)
        .bind(fallback)
        .bind(jan)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(fallback_time, jan.and_hms_opt(1, 0, 0).unwrap().and_utc());
    let ny_still: chrono::DateTime<Utc> = sqlx::query_scalar(query)
        .bind(ny)
        .bind(jan)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(ny_still, ny_time);
    let owner = Uuid::new_v4();
    let job:Uuid=sqlx::query_scalar("UPDATE notification_jobs j SET status='processing',lease_owner=$1,lease_until=now()+interval '1 hour' FROM calendar_events e WHERE e.id=j.event_id AND e.card_id=$2 AND e.event_date=$3 RETURNING j.id")
        .bind(owner).bind(ny).bind(jan).fetch_one(&s.db).await.unwrap();
    c["timezone"] = json!("Asia/Shanghai");
    ok(
        &s,
        "PATCH",
        &format!("/api/v1/cards/{ny}"),
        Some(c),
        Some(&a),
    )
    .await;
    worker::deliver(&s, a.user, job, owner).await.unwrap();
    let new_time: chrono::DateTime<Utc> = sqlx::query_scalar(query)
        .bind(ny)
        .bind(jan)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(new_time, fallback_time);
    let status: String = sqlx::query_scalar("SELECT status FROM notification_jobs WHERE id=$1")
        .bind(job)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert_eq!(status, "pending");
    let rank = ok(&s, "GET", "/api/v1/cards/ranking", None, Some(&a)).await;
    assert_eq!(rank["cards"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn rolling_horizon_and_custom_milestone_updates(db: PgPool) {
    let s = state(db);
    let a = account(&s, "recurrence@example.invalid", true).await;
    let card_id = add_card(&s, &a, "Schedule test").await;
    let today = Utc::now().date_naive();
    let end = dates::month(today, 13);
    let max: chrono::NaiveDate =
        sqlx::query_scalar("SELECT max(cycle_month) FROM card_cycles WHERE card_id=$1")
            .bind(card_id)
            .fetch_one(&s.db)
            .await
            .unwrap();
    assert_eq!(max, dates::month(today, 12));
    // Simulate records generated by the previous two-year planner.
    for (offset, note) in [(15, ""), (16, "Keep my note")] {
        sqlx::query("INSERT INTO card_cycles(id,user_id,card_id,cycle_month,statement_date,due_date,note) VALUES($1,$2,$3,$4,$4,$4,$5)")
            .bind(Uuid::new_v4()).bind(a.user).bind(card_id).bind(dates::month(today,offset)).bind(note).execute(&s.db).await.unwrap();
    }
    let rows = ok(
        &s,
        "GET",
        &format!("/api/v1/cards/{card_id}/cycles"),
        None,
        Some(&a),
    )
    .await;
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .all(
                |r| r["cycle_month"].as_str().unwrap() < end.to_string().as_str()
                    || r["note"] == "Keep my note"
            )
    );
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .any(|r| r["note"] == "Keep my note")
    );
    let first = today + chrono::Duration::days(3);
    let second = today + chrono::Duration::days(10);
    let value=ok(&s,"POST","/api/v1/milestones",Some(json!({"card_id":card_id,"title":"Custom test","kind":"benefit","recurrence":"custom_dates","start_date":today,"dates":[first,second]})),Some(&a)).await;
    let id = value["id"].as_str().unwrap();
    let event_id = carddue::planner::stable(&format!("{id}/date/{first}"));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM calendar_events WHERE card_id=$1 AND kind='benefit' AND active",
    )
    .bind(card_id)
    .fetch_one(&s.db)
    .await
    .unwrap();
    assert_eq!(count, 2);
    ok(&s,"PATCH",&format!("/api/v1/milestones/{id}"),Some(json!({"card_id":card_id,"title":"Custom test","kind":"benefit","recurrence":"custom_dates","start_date":today,"dates":[second]})),Some(&a)).await;
    let active: bool = sqlx::query_scalar("SELECT active FROM calendar_events WHERE id=$1")
        .bind(event_id)
        .fetch_one(&s.db)
        .await
        .unwrap();
    assert!(
        !active,
        "Removed date must retain an inactive cancellation identity"
    );
    let (status,_,_)=request(&s,"PATCH",&format!("/api/v1/milestones/{id}"),Some(json!({"card_id":card_id,"title":"Invalid","kind":"benefit","recurrence":"custom_months","start_date":today,"months":[13]})),Some(&a),&[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
