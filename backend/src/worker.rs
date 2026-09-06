use crate::{AppState, auth::{lock_user, audit}, crypto, error::{AppError, Result}, model::TemplateInput, notifications::aad, planner, providers, template};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn heartbeat(s: &AppState, role: &str) -> Result<()> {
    sqlx::query("INSERT INTO worker_heartbeats(role,last_success_at) VALUES($1,now()) ON CONFLICT(role) DO UPDATE SET last_success_at=now()")
        .bind(role).execute(&s.db).await?;
    Ok(())
}
pub async fn plan_all(s: AppState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(e) = planning_tick(&s).await { tracing::error!(code=e.code, "planner tick failed"); }
    }
}
async fn planning_tick(s: &AppState) -> Result<()> {
    let users: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM users ORDER BY id").fetch_all(&s.db).await?;
    for user in users {
        let mut tx = s.db.begin().await?;
        let tz: Option<String> = sqlx::query_scalar("SELECT timezone FROM users WHERE id=$1 FOR UPDATE SKIP LOCKED").bind(user).fetch_optional(&mut *tx).await?;
        if tz.is_none() { continue; }
        if let Err(e) = planner::reconcile(&mut tx, user, &s.config).await {
            tracing::error!(user_id=%user, code=e.code, "user planning failed");
            continue;
        }
        tx.commit().await?;
    }
    sqlx::query("DELETE FROM sessions WHERE expires_at<now()").execute(&s.db).await?;
    sqlx::query("DELETE FROM rate_limits WHERE bucket<$1").bind(Utc::now().timestamp().div_euclid(60)-5).execute(&s.db).await?;
    sqlx::query("DELETE FROM delivery_attempts WHERE finished_at<now()-interval '90 days'").execute(&s.db).await?;
    heartbeat(s, "planner").await
}
pub async fn run(s: AppState) {
    let owner = Uuid::new_v4();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(e) = tick(&s, owner).await { tracing::error!(code=e.code, "worker tick failed"); }
    }
}
pub async fn tick(s: &AppState, owner: Uuid) -> Result<()> {
    sqlx::query("UPDATE notification_jobs SET status='retry',lease_owner=NULL,lease_until=NULL,next_attempt_at=now() WHERE status='processing' AND lease_until<now()").execute(&s.db).await?;
    sqlx::query("UPDATE notification_jobs SET status='expired',lease_owner=NULL,lease_until=NULL WHERE status IN ('pending','retry') AND expires_at<=now()").execute(&s.db).await?;
    let rows = sqlx::query("WITH ready AS (SELECT id FROM notification_jobs WHERE status IN ('pending','retry') AND next_attempt_at<=now() AND expires_at>now() ORDER BY next_attempt_at,id FOR UPDATE SKIP LOCKED LIMIT 4) UPDATE notification_jobs j SET status='processing',lease_owner=$1,lease_until=now()+interval '3 minutes',attempt_count=attempt_count+1 FROM ready WHERE j.id=ready.id RETURNING j.id,j.user_id")
        .bind(owner).fetch_all(&s.db).await?;
    let mut tasks = tokio::task::JoinSet::new();
    for row in rows {
        let id: Uuid = row.try_get("id")?;
        let user: Uuid = row.try_get("user_id")?;
        let state = s.clone();
        tasks.spawn(async move { deliver(&state, user, id, owner).await });
    }
    let mut failed = false;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => (),
            Ok(Err(e)) => { failed=true; tracing::error!(code=e.code, "delivery database failure"); },
            Err(_) => { failed=true; tracing::error!("delivery task stopped"); },
        }
    }
    if failed { return Err(AppError::internal()); }
    heartbeat(s, "worker").await
}
fn terminal_error(code: &'static str) -> providers::Delivery {
    providers::Delivery { status:None, retryable:false, invalid_subscription:false, code, retry_after:None }
}
pub async fn deliver(s: &AppState, user: Uuid, id: Uuid, owner: Uuid) -> Result<()> {
    let mut tx = s.db.begin().await?;
    let timezone = lock_user(&mut tx, user).await?;
    let job = sqlx::query("SELECT * FROM notification_jobs WHERE id=$1 AND user_id=$2 AND status='processing' AND lease_owner=$3 AND lease_until>now() FOR UPDATE")
        .bind(id).bind(user).bind(owner).fetch_optional(&mut *tx).await?;
    let Some(job) = job else { return Ok(()); };
    let scheduled: DateTime<Utc> = job.try_get("scheduled_at")?;
    if scheduled > Utc::now() {
        // A date or timezone may change after claim but before the user lock is held.
        // Recheck the current schedule instead of sending a now-premature reminder.
        sqlx::query("UPDATE notification_jobs SET status='pending',next_attempt_at=scheduled_at,lease_owner=NULL,lease_until=NULL,attempt_count=GREATEST(attempt_count-1,0) WHERE id=$1 AND lease_owner=$2")
            .bind(id).bind(owner).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(());
    }
    let expires: DateTime<Utc> = job.try_get("expires_at")?;
    let connection: Uuid = job.try_get("connection_id")?;
    let c = sqlx::query("SELECT kind,encrypted_config,enabled FROM notification_connections WHERE id=$1 AND user_id=$2").bind(connection).bind(user).fetch_one(&mut *tx).await?;
    let mut allowed = c.try_get::<bool,_>("enabled")? && expires>Utc::now();
    let mut context: Value = job.try_get("context")?;
    if let Some(event) = job.try_get::<Option<Uuid>,_>("event_id")? {
        let e = sqlx::query("SELECT active,paid,event_date FROM calendar_events WHERE id=$1 AND user_id=$2").bind(event).bind(user).fetch_one(&mut *tx).await?;
        allowed &= e.try_get::<bool,_>("active")? && !e.try_get::<bool,_>("paid")?;
        let date: chrono::NaiveDate = e.try_get("event_date")?;
        let tz: chrono_tz::Tz = timezone.parse().map_err(|_|AppError::internal())?;
        context["event"]["days_until"] = json!((date-Utc::now().with_timezone(&tz).date_naive()).num_days());
    }
    if let Some(rule) = job.try_get::<Option<Uuid>,_>("rule_id")? {
        let enabled: bool = sqlx::query_scalar("SELECT COALESCE((data->>'enabled')::boolean,true) FROM notification_rules WHERE id=$1 AND user_id=$2").bind(rule).bind(user).fetch_one(&mut *tx).await?;
        allowed &= enabled;
    }
    if !allowed {
        sqlx::query("UPDATE notification_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE id=$1").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(());
    }
    let started = Utc::now();
    let kind: String = c.try_get("kind")?;
    let encrypted: String = c.try_get("encrypted_config")?;
    let t: TemplateInput = serde_json::from_value(job.try_get("template_snapshot")?)?;
    let rendered = template::render(&t, &context, &s.config.base_url);
    let mut stored = Value::Null;
    let outcome = match (rendered, crypto::decrypt(&s.config.encryption_key, &aad(user,connection,&kind), &encrypted)) {
        (Ok(rendered), Ok(bytes)) => {
            stored=json!(rendered);
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(config) => providers::send(&kind, &config, &rendered, id, &s.config).await,
                Err(_) => Err(terminal_error("CREDENTIAL_FORMAT")),
            }
        },
        (Err(_), _) => Err(terminal_error("TEMPLATE_RENDER_FAILED")),
        (_, Err(_)) => Err(terminal_error("CREDENTIAL_DECRYPT_FAILED")),
    };
    let attempt: i32 = job.try_get("attempt_count")?;
    let (status,response,error,next,invalid) = match &outcome {
        Ok(code) => ("sent", code.map(i32::from), None, Utc::now(), false),
        Err(e) => {
            let base = match attempt { 1=>60, 2=>300, 3=>1800, _=>7200 };
            let jitter = rand::thread_rng().gen_range(0..=15);
            let delay = e.retry_after.unwrap_or(base).max(base)+jitter;
            let next = Utc::now()+Duration::seconds(delay as i64);
            (if e.retryable && attempt<5 && next<expires {"retry"} else {"dead"}, e.status.map(i32::from), Some(e.code), next, e.invalid_subscription)
        },
    };
    sqlx::query("INSERT INTO delivery_attempts(job_id,started_at,finished_at,success,response_status,error_code,rendered) VALUES($1,$2,now(),$3,$4,$5,$6)")
        .bind(id).bind(started).bind(outcome.is_ok()).bind(response).bind(error).bind(stored).execute(&mut *tx).await?;
    sqlx::query("UPDATE notification_jobs SET status=$1,next_attempt_at=$2,last_error=$3,sent_at=CASE WHEN $1='sent' THEN now() ELSE NULL END,lease_owner=NULL,lease_until=NULL WHERE id=$4 AND lease_owner=$5")
        .bind(status).bind(next).bind(error).bind(id).bind(owner).execute(&mut *tx).await?;
    if invalid {
        sqlx::query("UPDATE notification_connections SET enabled=false,updated_at=now() WHERE id=$1 AND user_id=$2").bind(connection).bind(user).execute(&mut *tx).await?;
        sqlx::query("UPDATE notification_jobs SET status='cancelled' WHERE connection_id=$1 AND user_id=$2 AND status IN ('pending','retry')").bind(connection).bind(user).execute(&mut *tx).await?;
        audit(&mut tx, user, "webpush_subscription_expired", Some(connection)).await?;
    }
    // Deliberately hold the per-user lock across the bounded provider call. Committed
    // mark-paid actions cannot race a later send; an in-flight message cannot be recalled.
    // Acceptance followed by a crash before this commit still permits at-least-once duplicates.
    tx.commit().await?;
    Ok(())
}
