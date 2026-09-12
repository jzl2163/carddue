use crate::{
    AppState,
    auth::{Auth, audit, lock_user},
    error::{AppError, Result},
    model::{CardInput, CardView, CyclePatch, CycleView, MilestoneInput, date_range, money, text},
    planner,
    web::ApiJson,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub async fn list(State(s): State<AppState>, auth: Auth) -> Result<Json<Vec<CardView>>> {
    let rows = sqlx::query(
        "SELECT id,data,active FROM cards WHERE user_id=$1 ORDER BY active DESC,created_at DESC",
    )
    .bind(auth.user.id)
    .fetch_all(&s.db)
    .await?;
    let mut result = Vec::new();
    for r in rows {
        result.push(CardView {
            id: r.try_get("id")?,
            data: serde_json::from_value(r.try_get("data")?)?,
            active: r.try_get("active")?,
        });
    }
    Ok(Json(result))
}
pub async fn get(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<CardView>> {
    let r = sqlx::query("SELECT id,data,active FROM cards WHERE user_id=$1 AND id=$2")
        .bind(auth.user.id)
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(CardView {
        id: r.try_get("id")?,
        data: serde_json::from_value(r.try_get("data")?)?,
        active: r.try_get("active")?,
    }))
}
pub async fn create(
    State(s): State<AppState>,
    auth: Auth,
    ApiJson(c): ApiJson<CardInput>,
) -> Result<Json<Value>> {
    c.validate()?;
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM cards WHERE user_id=$1")
        .bind(auth.user.id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= 200 {
        return Err(AppError::bad(
            "Card limit reached (200 including archived cards)",
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO cards(id,user_id,data) VALUES($1,$2,$3)")
        .bind(id)
        .bind(auth.user.id)
        .bind(json!(c))
        .execute(&mut *tx)
        .await?;
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "card_created", Some(id)).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id})))
}
pub async fn update(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    ApiJson(c): ApiJson<CardInput>,
) -> Result<Json<Value>> {
    c.validate()?;
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let r = sqlx::query("UPDATE cards SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3")
        .bind(json!(c))
        .bind(id)
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() != 1 {
        return Err(AppError::missing());
    }
    sqlx::query("UPDATE calendar_events SET revision=revision+1,updated_at=now() WHERE card_id=$1 AND user_id=$2").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "card_updated", Some(id)).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
async fn set_active(s: AppState, auth: Auth, id: Uuid, active: bool) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let r = sqlx::query("UPDATE cards SET active=$1,updated_at=now() WHERE id=$2 AND user_id=$3")
        .bind(active)
        .bind(id)
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() != 1 {
        return Err(AppError::missing());
    }
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(
        &mut tx,
        auth.user.id,
        if active {
            "card_restored"
        } else {
            "card_archived"
        },
        Some(id),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn archive(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    set_active(s, auth, id, false).await
}
pub async fn restore(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    set_active(s, auth, id, true).await
}
pub async fn cycles(
    State(s): State<AppState>,
    auth: Auth,
    Path(card): Path<Uuid>,
) -> Result<Json<Vec<CycleView>>> {
    let data: Option<sqlx::types::Json<CardInput>> =
        sqlx::query_scalar("SELECT data FROM cards WHERE id=$1 AND user_id=$2")
            .bind(card)
            .bind(auth.user.id)
            .fetch_optional(&s.db)
            .await?;
    let data = data.ok_or_else(AppError::missing)?;
    let timezone = data.effective_timezone(&auth.user.timezone)?;
    let end = crate::dates::month(chrono::Utc::now().with_timezone(&timezone).date_naive(), 13);
    let mut rows = sqlx::query_as::<_, CycleView>(planner::CYCLES)
        .bind(auth.user.id)
        .bind(card)
        .fetch_all(&s.db)
        .await?;
    // Retain previously entered information even outside the new rolling horizon.
    rows.retain(|c| {
        c.cycle_month < end
            || c.statement_overridden
            || c.due_overridden
            || c.amount.is_some()
            || c.minimum_payment.is_some()
            || c.paid_at.is_some()
            || !c.note.is_empty()
    });
    Ok(Json(rows))
}
pub async fn patch_cycle(
    State(s): State<AppState>,
    auth: Auth,
    Path((card, cycle)): Path<(Uuid, Uuid)>,
    ApiJson(p): ApiJson<CyclePatch>,
) -> Result<Json<Value>> {
    money(p.amount.as_deref())?;
    money(p.minimum_payment.as_deref())?;
    if let Some(d) = p.statement_date {
        date_range(d)?;
    }
    if let Some(d) = p.due_date {
        date_range(d)?;
    }
    if let Some(n) = &p.note {
        text(n, 0, 4000)?;
    }
    if p.reset_dates && (p.statement_date.is_some() || p.due_date.is_some()) {
        return Err(AppError::bad(
            "Do not combine reset_dates and explicit dates",
        ));
    }
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let row=sqlx::query("SELECT c.data,cy.cycle_month,cy.statement_date,cy.due_date,cy.statement_overridden,cy.due_overridden FROM card_cycles cy JOIN cards c ON c.id=cy.card_id WHERE cy.id=$1 AND cy.card_id=$2 AND cy.user_id=$3")
        .bind(cycle).bind(card).bind(auth.user.id).fetch_one(&mut *tx).await?;
    let c: CardInput = serde_json::from_value(row.try_get("data")?)?;
    let month = row.try_get("cycle_month")?;
    let mut statement = p.statement_date.unwrap_or(row.try_get("statement_date")?);
    let mut due = p.due_date.unwrap_or(row.try_get("due_date")?);
    let mut so = p.statement_date.is_some() || row.try_get::<bool, _>("statement_overridden")?;
    let mut do_ = p.due_date.is_some() || row.try_get::<bool, _>("due_overridden")?;
    if p.reset_dates {
        (statement, due) = crate::dates::cycle_dates(&c, month)?;
        so = false;
        do_ = false;
    } else if p.statement_date.is_some() && !do_ {
        due = crate::dates::due_for_statement(&c, month, statement)?;
    }
    if due < statement {
        return Err(AppError::bad("Due date cannot precede statement date"));
    }
    sqlx::query("UPDATE card_cycles SET statement_date=$1,due_date=$2,statement_overridden=$3,due_overridden=$4,amount=CASE WHEN $5 THEN NULL ELSE COALESCE(($6::text)::numeric,amount) END,minimum_payment=CASE WHEN $7 THEN NULL ELSE COALESCE(($8::text)::numeric,minimum_payment) END,note=COALESCE($9,note),revision=revision+1,updated_at=now() WHERE id=$10 AND user_id=$11")
        .bind(statement).bind(due).bind(so).bind(do_).bind(p.clear_amount).bind(&p.amount).bind(p.clear_minimum_payment).bind(&p.minimum_payment).bind(&p.note).bind(cycle).bind(auth.user.id).execute(&mut *tx).await?;
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "cycle_overridden", Some(cycle)).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
async fn payment(
    s: AppState,
    auth: Auth,
    card: Uuid,
    cycle: Uuid,
    paid: bool,
) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let r=sqlx::query("UPDATE card_cycles SET paid_at=CASE WHEN $1 THEN COALESCE(paid_at,now()) ELSE NULL END,revision=revision+1,updated_at=now() WHERE id=$2 AND card_id=$3 AND user_id=$4")
        .bind(paid).bind(cycle).bind(card).bind(auth.user.id).execute(&mut *tx).await?;
    if r.rows_affected() != 1 {
        return Err(AppError::missing());
    }
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(
        &mut tx,
        auth.user.id,
        if paid { "marked_paid" } else { "unmarked_paid" },
        Some(cycle),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn pay(
    State(s): State<AppState>,
    auth: Auth,
    Path((card, cycle)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    payment(s, auth, card, cycle, true).await
}
pub async fn unpay(
    State(s): State<AppState>,
    auth: Auth,
    Path((card, cycle)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>> {
    payment(s, auth, card, cycle, false).await
}
pub async fn dashboard(State(s): State<AppState>, auth: Auth) -> Result<Json<Value>> {
    let tz: chrono_tz::Tz = auth
        .user
        .timezone
        .parse()
        .map_err(|_| AppError::internal())?;
    let today = chrono::Utc::now().with_timezone(&tz).date_naive();
    let rows=sqlx::query("SELECT e.id,e.card_id,e.cycle_id,e.kind,e.title,e.event_date,e.paid,c.data->>'name' AS card_name,c.data->>'color' AS color,c.data->>'timezone' AS card_timezone,cy.amount::text AS amount FROM calendar_events e JOIN cards c ON c.id=e.card_id LEFT JOIN card_cycles cy ON cy.id=e.cycle_id WHERE e.user_id=$1 AND e.active=true AND e.event_date BETWEEN $2 AND $3 ORDER BY e.event_date,e.kind LIMIT 500")
        .bind(auth.user.id).bind(today-chrono::Duration::days(30)).bind(today+chrono::Duration::days(90)).fetch_all(&s.db).await?;
    let mut events = Vec::new();
    for r in rows {
        let card_timezone = r
            .try_get::<Option<String>, _>("card_timezone")?
            .unwrap_or_else(|| auth.user.timezone.clone());
        let card_tz: chrono_tz::Tz = card_timezone.parse().map_err(|_| AppError::internal())?;
        let local_today = chrono::Utc::now().with_timezone(&card_tz).date_naive();
        let event_date: chrono::NaiveDate = r.try_get("event_date")?;
        events.push(json!({"timezone":card_timezone,"local_today":local_today,"days_until":(event_date-local_today).num_days(),"id":r.try_get::<Uuid,_>("id")?,"card_id":r.try_get::<Uuid,_>("card_id")?,"cycle_id":r.try_get::<Option<Uuid>,_>("cycle_id")?,"kind":r.try_get::<String,_>("kind")?,"title":r.try_get::<String,_>("title")?,"date":r.try_get::<chrono::NaiveDate,_>("event_date")?,"paid":r.try_get::<bool,_>("paid")?,"card_name":r.try_get::<String,_>("card_name")?,"color":r.try_get::<String,_>("color")?,"amount":r.try_get::<Option<String>,_>("amount")?}));
    }
    Ok(Json(
        json!({"today":today,"timezone":auth.user.timezone,"events":events}),
    ))
}
pub async fn milestones(State(s): State<AppState>, auth: Auth) -> Result<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id,data FROM card_milestones WHERE user_id=$1 ORDER BY updated_at DESC",
    )
    .bind(auth.user.id)
    .fetch_all(&s.db)
    .await?;
    let mut out = Vec::new();
    for r in rows {
        out.push(json!({"id":r.try_get::<Uuid,_>("id")?,"data":r.try_get::<Value,_>("data")?}));
    }
    Ok(Json(json!(out)))
}
pub async fn owned_cards(conn: &mut PgConnection, user: Uuid, ids: &[Uuid]) -> Result<()> {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cards WHERE user_id=$1 AND id=ANY($2)")
            .bind(user)
            .bind(ids)
            .fetch_one(conn)
            .await?;
    if count as usize != ids.iter().collect::<std::collections::HashSet<_>>().len() {
        return Err(AppError::missing());
    }
    Ok(())
}
async fn save_milestone(
    s: AppState,
    auth: Auth,
    id: Option<Uuid>,
    m: MilestoneInput,
) -> Result<Json<Value>> {
    m.validate()?;
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    owned_cards(&mut tx, auth.user.id, &[m.card_id]).await?;
    let key = id.unwrap_or_else(Uuid::new_v4);
    if id.is_some() {
        let r=sqlx::query("UPDATE card_milestones SET data=$1,card_id=$2,updated_at=now() WHERE id=$3 AND user_id=$4").bind(json!(m)).bind(m.card_id).bind(key).bind(auth.user.id).execute(&mut *tx).await?;
        if r.rows_affected() != 1 {
            return Err(AppError::missing());
        }
    } else {
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM card_milestones WHERE user_id=$1")
            .bind(auth.user.id)
            .fetch_one(&mut *tx)
            .await?;
        if n >= 400 {
            return Err(AppError::bad("Milestone limit reached"));
        }
        sqlx::query("INSERT INTO card_milestones(id,user_id,card_id,data) VALUES($1,$2,$3,$4)")
            .bind(key)
            .bind(auth.user.id)
            .bind(m.card_id)
            .bind(json!(m))
            .execute(&mut *tx)
            .await?;
    }
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "milestone_saved", Some(key)).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":key})))
}
pub async fn create_milestone(
    State(s): State<AppState>,
    auth: Auth,
    ApiJson(m): ApiJson<MilestoneInput>,
) -> Result<Json<Value>> {
    save_milestone(s, auth, None, m).await
}
pub async fn update_milestone(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    ApiJson(m): ApiJson<MilestoneInput>,
) -> Result<Json<Value>> {
    save_milestone(s, auth, Some(id), m).await
}
pub async fn delete_milestone(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let r = sqlx::query("DELETE FROM card_milestones WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    if r.rows_affected() != 1 {
        return Err(AppError::missing());
    }
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "milestone_deleted", Some(id)).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}

pub async fn delete(
    State(s): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    owned_cards(&mut tx, auth.user.id, &[id]).await?;
    sqlx::query("DELETE FROM delivery_attempts WHERE job_id IN (SELECT j.id FROM notification_jobs j JOIN calendar_events e ON e.id=j.event_id WHERE e.card_id=$1 AND j.user_id=$2)")
        .bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM notification_jobs WHERE user_id=$2 AND event_id IN (SELECT id FROM calendar_events WHERE card_id=$1 AND user_id=$2)")
        .bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    for query in [
        "DELETE FROM calendar_events WHERE card_id=$1 AND user_id=$2",
        "DELETE FROM card_cycles WHERE card_id=$1 AND user_id=$2",
        "DELETE FROM card_milestones WHERE card_id=$1 AND user_id=$2",
    ] {
        sqlx::query(query)
            .bind(id)
            .bind(auth.user.id)
            .execute(&mut *tx)
            .await?;
    }
    let deleted = id.to_string();
    for (is_feed, select, update) in [
        (
            true,
            "SELECT id,data FROM calendar_feeds WHERE user_id=$1",
            "UPDATE calendar_feeds SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3",
        ),
        (
            false,
            "SELECT id,data FROM notification_rules WHERE user_id=$1",
            "UPDATE notification_rules SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3",
        ),
    ] {
        let rows = sqlx::query(select)
            .bind(auth.user.id)
            .fetch_all(&mut *tx)
            .await?;
        for row in rows {
            let mut data: Value = row.try_get("data")?;
            if let Some(ids) = data["card_ids"].as_array_mut() {
                let before = ids.len();
                ids.retain(|v| v.as_str() != Some(deleted.as_str()));
                if ids.len() != before {
                    if ids.is_empty() {
                        // Do not silently broaden an explicit card selection to all cards.
                        // An empty-kind feed can still deliver cancellation tombstones.
                        if is_feed {
                            data["kinds"] = json!([]);
                        } else {
                            data["enabled"] = json!(false);
                        }
                    }
                    sqlx::query(update)
                        .bind(data)
                        .bind(row.try_get::<Uuid, _>("id")?)
                        .bind(auth.user.id)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
    }
    sqlx::query("DELETE FROM cards WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(auth.user.id)
        .execute(&mut *tx)
        .await?;
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    audit(&mut tx, auth.user.id, "card_deleted", Some(id)).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}

pub async fn ranking(State(s): State<AppState>, auth: Auth) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    let account_timezone = lock_user(&mut tx, auth.user.id).await?;
    planner::reconcile(&mut tx, auth.user.id, &s.config).await?;
    let rows = sqlx::query("SELECT id,data FROM cards WHERE user_id=$1 AND active=true")
        .bind(auth.user.id)
        .fetch_all(&mut *tx)
        .await?;
    let now = chrono::Utc::now();
    let account_today = now
        .with_timezone(
            &account_timezone
                .parse::<chrono_tz::Tz>()
                .map_err(|_| AppError::internal())?,
        )
        .date_naive();
    let mut result = Vec::new();
    for row in rows {
        let id: Uuid = row.try_get("id")?;
        let c: CardInput = serde_json::from_value(row.try_get("data")?)?;
        let tz = c.effective_timezone(&account_timezone)?;
        let today = now.with_timezone(&tz).date_naive();
        let cycles = sqlx::query_as::<_, CycleView>(planner::CYCLES)
            .bind(auth.user.id)
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
        let estimate = crate::dates::interest_estimate(&cycles, today);
        if let Some(crate::dates::InterestEstimate {
            statement,
            due,
            days,
            longest,
        }) = estimate
        {
            result.push(json!({"id":id,"name":c.name,"issuer":c.issuer,"network":c.network,"color":c.color,
                "region":c.region,"timezone":tz.name(),"different_timezone":tz.name()!=account_timezone,
                "different_date":today!=account_today,"local_today":today,"statement_date":statement,
                "due_date":due,"days":days,"longest_days":longest}));
        }
    }
    result.sort_by(|a, b| {
        b["days"]
            .as_i64()
            .cmp(&a["days"].as_i64())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    tx.commit().await?;
    Ok(Json(
        json!({"account_timezone":account_timezone,"as_of":now,"cards":result}),
    ))
}
