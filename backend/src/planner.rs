use crate::{config::Config, dates, error::{AppError, Result}, model::{CardInput, CycleView, Event, MilestoneInput, RuleInput, TemplateInput}};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, Utc};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use std::collections::HashMap;
use uuid::Uuid;

pub const CYCLES: &str = "SELECT id,card_id,cycle_month,statement_date,due_date,statement_overridden,due_overridden,amount::text AS amount,minimum_payment::text AS minimum_payment,paid_at,note,revision FROM card_cycles WHERE user_id=$1 AND card_id=$2 ORDER BY cycle_month DESC";
pub fn stable(key: &str) -> Uuid { Uuid::new_v5(&Uuid::NAMESPACE_OID, key.as_bytes()) }
struct EventDraft<'a> {
    id: Uuid,
    card: Uuid,
    cycle: Option<Uuid>,
    kind: &'a str,
    title: &'a str,
    date: NaiveDate,
    paid: bool,
}
async fn event(conn: &mut PgConnection, user: Uuid, e: EventDraft<'_>) -> Result<()> {
    sqlx::query("INSERT INTO calendar_events(id,user_id,card_id,cycle_id,kind,title,event_date,paid) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(id) DO UPDATE SET card_id=EXCLUDED.card_id,cycle_id=EXCLUDED.cycle_id,kind=EXCLUDED.kind,title=EXCLUDED.title,event_date=EXCLUDED.event_date,paid=EXCLUDED.paid,active=true,revision=calendar_events.revision+1,updated_at=now() WHERE (calendar_events.card_id,calendar_events.cycle_id,calendar_events.kind,calendar_events.title,calendar_events.event_date,calendar_events.paid,calendar_events.active) IS DISTINCT FROM (EXCLUDED.card_id,EXCLUDED.cycle_id,EXCLUDED.kind,EXCLUDED.title,EXCLUDED.event_date,EXCLUDED.paid,true)")
        .bind(e.id).bind(user).bind(e.card).bind(e.cycle).bind(e.kind).bind(e.title).bind(e.date).bind(e.paid).execute(conn).await?;
    Ok(())
}
/// The caller holds the user's row lock. Dates, events and jobs commit atomically.
pub async fn reconcile(conn: &mut PgConnection, user: Uuid, cfg: &Config) -> Result<()> {
    let timezone: String = sqlx::query_scalar("SELECT timezone FROM users WHERE id=$1").bind(user).fetch_one(&mut *conn).await?;
    let tz: chrono_tz::Tz = timezone.parse().map_err(|_|AppError::internal())?;
    let now = Utc::now();
    let today = now.with_timezone(&tz).date_naive();
    let current = dates::month(today, 0);
    let start = dates::month(today, -3);
    let end = dates::month(today, 25);
    let rows = sqlx::query("SELECT id,data FROM cards WHERE user_id=$1 AND active=true ORDER BY id").bind(user).fetch_all(&mut *conn).await?;
    let mut cards: HashMap<Uuid, CardInput> = HashMap::new();
    let mut cycles: HashMap<Uuid, CycleView> = HashMap::new();
    let mut ids = Vec::new();
    for row in rows {
        let card: Uuid = row.try_get("id")?;
        let c: CardInput = serde_json::from_value(row.try_get("data")?)?;
        for offset in -3..=24 {
            let m = dates::month(today, offset);
            let id = stable(&format!("{card}/cycle/{m}"));
            let (statement, due) = dates::cycle_dates(&c, m)?;
            sqlx::query("INSERT INTO card_cycles(id,user_id,card_id,cycle_month,statement_date,due_date) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(card_id,cycle_month) DO NOTHING")
                .bind(id).bind(user).bind(card).bind(m).bind(statement).bind(due).execute(&mut *conn).await?;
        }
        let mut card_cycles = sqlx::query_as::<_, CycleView>(CYCLES).bind(user).bind(card).fetch_all(&mut *conn).await?;
        for cycle in &mut card_cycles {
            if cycle.cycle_month < start || cycle.cycle_month >= end { continue; }
            if cycle.paid_at.is_none() && cycle.cycle_month >= current {
                let (statement, _) = dates::cycle_dates(&c, cycle.cycle_month)?;
                let statement = if cycle.statement_overridden { cycle.statement_date } else { statement };
                let due = if cycle.due_overridden { cycle.due_date } else { dates::due_for_statement(&c, cycle.cycle_month, statement)? };
                if due < statement { return Err(AppError::bad("An overridden due date conflicts with the new statement rule")); }
                if statement != cycle.statement_date || due != cycle.due_date {
                    sqlx::query("UPDATE card_cycles SET statement_date=$1,due_date=$2,revision=revision+1,updated_at=now() WHERE id=$3 AND user_id=$4")
                        .bind(statement).bind(due).bind(cycle.id).bind(user).execute(&mut *conn).await?;
                    cycle.statement_date = statement;
                    cycle.due_date = due;
                }
            }
            for (kind, date) in [("statement",cycle.statement_date), ("payment_due",cycle.due_date)] {
                let id = stable(&format!("{}/{kind}", cycle.id));
                ids.push(id);
                event(conn, user, EventDraft { id, card, cycle:Some(cycle.id), kind, title:dates::label(kind), date, paid:kind=="payment_due" && cycle.paid_at.is_some() }).await?;
            }
            cycles.insert(cycle.id, cycle.clone());
        }
        if let (Some(m), Some(d)) = (c.annual_fee_month, c.annual_fee_day) {
            for year in start.year()..=end.year() {
                let date = dates::day(year, m, d);
                if date < start || date >= end { continue; }
                let id = stable(&format!("{card}/annual/{year}"));
                ids.push(id);
                event(conn, user, EventDraft { id, card, cycle:None, kind:"annual_fee", title:"年费日", date, paid:false }).await?;
            }
        }
        cards.insert(card, c);
    }
    let milestones = sqlx::query("SELECT id,data FROM card_milestones WHERE user_id=$1 ORDER BY id").bind(user).fetch_all(&mut *conn).await?;
    for row in milestones {
        let milestone: Uuid = row.try_get("id")?;
        let m: MilestoneInput = serde_json::from_value(row.try_get("data")?)?;
        if !cards.contains_key(&m.card_id) { continue; }
        let mut occurrences = Vec::new();
        match m.recurrence.as_str() {
            "one_time" => occurrences.push(("once".to_string(), m.start_date)),
            "monthly" => for offset in -3..=24 {
                let month = dates::month(today, offset);
                occurrences.push((month.format("%Y-%m").to_string(), dates::day(month.year(), month.month(), m.start_date.day())));
            },
            "yearly" => for year in start.year()..=end.year() {
                occurrences.push((year.to_string(), dates::day(year, m.start_date.month(), m.start_date.day())));
            },
            _ => return Err(AppError::internal()),
        }
        for (key, date) in occurrences {
            if date < m.start_date || date < start || date >= end { continue; }
            let id = stable(&format!("{milestone}/{key}"));
            ids.push(id);
            event(conn, user, EventDraft { id, card:m.card_id, cycle:None, kind:&m.kind, title:&m.title, date, paid:false }).await?;
        }
    }
    // Never recycle event IDs. Calendar publication records retain cancellation identities.
    sqlx::query("UPDATE calendar_events SET active=false,revision=revision+1,updated_at=now() WHERE user_id=$1 AND active=true AND NOT(id=ANY($2))")
        .bind(user).bind(&ids).execute(&mut *conn).await?;
    let events = sqlx::query_as::<_, Event>("SELECT * FROM calendar_events WHERE user_id=$1 AND active=true AND paid=false ORDER BY event_date,id").bind(user).fetch_all(&mut *conn).await?;
    let rules = sqlx::query("SELECT id,data,created_at FROM notification_rules WHERE user_id=$1").bind(user).fetch_all(&mut *conn).await?;
    let connections: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM notification_connections WHERE user_id=$1 AND enabled=true").bind(user).fetch_all(&mut *conn).await?;
    let template_rows = sqlx::query("SELECT id,data FROM notification_templates WHERE user_id=$1").bind(user).fetch_all(&mut *conn).await?;
    let mut templates: HashMap<Uuid, Value> = HashMap::new();
    for row in template_rows { templates.insert(row.try_get("id")?, row.try_get("data")?); }
    let plan = Uuid::new_v4();
    let mut planned = 0;
    for row in rules {
        let rule_id: Uuid = row.try_get("id")?;
        let created: DateTime<Utc> = row.try_get("created_at")?;
        let rule: RuleInput = serde_json::from_value(row.try_get("data")?)?;
        if !rule.enabled { continue; }
        let Some(template) = templates.get(&rule.template_id) else { continue; };
        let _: TemplateInput = serde_json::from_value(template.clone())?;
        let time = NaiveTime::parse_from_str(&rule.local_time, "%H:%M").map_err(|_|AppError::internal())?;
        for e in &events {
            if e.kind != rule.event_kind || !rule.card_ids.is_empty() && !rule.card_ids.contains(&e.card_id) { continue; }
            let Some(card) = cards.get(&e.card_id) else { continue; };
            let cycle = e.cycle_id.and_then(|id|cycles.get(&id));
            let context = json!({"card":{"id":e.card_id,"name":card.name,"issuer":card.issuer,"last4":card.last4,"currency":card.currency},"event":{"type":e.kind,"label":e.title,"date":e.event_date,"days_until":(e.event_date-today).num_days()},"cycle":{"statement_date":cycle.map(|c|c.statement_date),"due_date":cycle.map(|c|c.due_date),"amount":cycle.and_then(|c|c.amount.clone()),"minimum_payment":cycle.and_then(|c|c.minimum_payment.clone())},"user":{"timezone":timezone},"app":{"url":cfg.base_url}});
            for offset in &rule.offsets {
                let when = dates::local_instant(e.event_date-Duration::days(i64::from(*offset)), time, tz)?;
                if when < created || when < now-Duration::hours(24) { continue; }
                for connection in &rule.connection_ids {
                    if !connections.contains(connection) { continue; }
                    planned += 1;
                    if planned > 20000 { return Err(AppError::bad("Too many scheduled reminders; narrow card, channel or offset selections")); }
                    sqlx::query("INSERT INTO notification_jobs(id,user_id,rule_id,event_id,connection_id,offset_days,scheduled_at,next_attempt_at,expires_at,template_snapshot,context,plan_token) VALUES($1,$2,$3,$4,$5,$6,$7,$7,$8,$9,$10,$11) ON CONFLICT(rule_id,event_id,connection_id,offset_days) DO UPDATE SET scheduled_at=EXCLUDED.scheduled_at,expires_at=EXCLUDED.expires_at,template_snapshot=EXCLUDED.template_snapshot,context=EXCLUDED.context,plan_token=EXCLUDED.plan_token,status=CASE WHEN notification_jobs.status IN ('cancelled','expired') THEN 'pending' ELSE notification_jobs.status END,next_attempt_at=CASE WHEN notification_jobs.scheduled_at IS DISTINCT FROM EXCLUDED.scheduled_at OR notification_jobs.status IN ('cancelled','expired') THEN EXCLUDED.scheduled_at ELSE notification_jobs.next_attempt_at END,attempt_count=CASE WHEN notification_jobs.status IN ('cancelled','expired') THEN 0 ELSE notification_jobs.attempt_count END WHERE notification_jobs.status NOT IN ('sent','dead')")
                        .bind(Uuid::new_v4()).bind(user).bind(rule_id).bind(e.id).bind(connection).bind(offset).bind(when).bind(when+Duration::hours(24)).bind(template).bind(&context).bind(plan).execute(&mut *conn).await?;
                }
            }
        }
    }
    sqlx::query("UPDATE notification_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE user_id=$1 AND rule_id IS NOT NULL AND status IN ('pending','retry','processing') AND plan_token IS DISTINCT FROM $2")
        .bind(user).bind(plan).execute(&mut *conn).await?;
    Ok(())
}
