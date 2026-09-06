use crate::{AppState, auth::{Auth, audit, lock_user, rate}, cards::owned_cards, crypto, dates, error::{AppError, Result}, model::{CardInput, Event, FeedInput}, web::ApiJson};
use axum::{extract::{State, Path, ConnectInfo}, http::{HeaderMap, HeaderValue, StatusCode}, response::{Response, IntoResponse}, Json};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde_json::{Value, json};
use sqlx::Row;
use std::{collections::{HashMap, HashSet}, net::SocketAddr};
use uuid::Uuid;

pub fn url(s: &AppState, id: Uuid, version: i32) -> String {
    format!("{}/cal/v1/{}/{}.ics", s.config.base_url, id, crypto::sign(&s.config.signing_key, "feed", &format!("{id}:{version}")))
}
pub async fn list(State(s): State<AppState>, auth: Auth) -> Result<Json<Value>> {
    let rows = sqlx::query("SELECT id,data,token_version,last_accessed_at FROM calendar_feeds WHERE user_id=$1 ORDER BY updated_at DESC").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut feeds = Vec::new();
    for r in rows {
        let id = r.try_get::<Uuid, _>("id")?;
        feeds.push(json!({"id":id,"data":r.try_get::<Value,_>("data")?,"url":url(&s,id,r.try_get("token_version")?),"last_accessed_at":r.try_get::<Option<DateTime<Utc>>,_>("last_accessed_at")?}));
    }
    Ok(Json(json!(feeds)))
}
async fn save(s: AppState, auth: Auth, id: Option<Uuid>, f: FeedInput) -> Result<Json<Value>> {
    f.validate()?;
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    owned_cards(&mut tx, auth.user.id, &f.card_ids).await?;
    let key = id.unwrap_or_else(Uuid::new_v4);
    let version: i32 = if id.is_some() {
        sqlx::query_scalar("UPDATE calendar_feeds SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3 RETURNING token_version").bind(json!(f)).bind(key).bind(auth.user.id).fetch_one(&mut *tx).await?
    } else {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_feeds WHERE user_id=$1").bind(auth.user.id).fetch_one(&mut *tx).await?;
        if count >= 20 { return Err(AppError::bad("Calendar feed limit reached")); }
        sqlx::query("INSERT INTO calendar_feeds(id,user_id,data) VALUES($1,$2,$3)").bind(key).bind(auth.user.id).bind(json!(f)).execute(&mut *tx).await?;
        1
    };
    audit(&mut tx, auth.user.id, "feed_saved", Some(key)).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":key,"url":url(&s,key,version)})))
}
pub async fn create(State(s): State<AppState>, auth: Auth, ApiJson(f): ApiJson<FeedInput>) -> Result<Json<Value>> { save(s, auth, None, f).await }
pub async fn update(State(s): State<AppState>, auth: Auth, Path(id): Path<Uuid>, ApiJson(f): ApiJson<FeedInput>) -> Result<Json<Value>> { save(s, auth, Some(id), f).await }
pub async fn delete(State(s): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let result = sqlx::query("DELETE FROM calendar_feeds WHERE id=$1 AND user_id=$2").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    if result.rows_affected() != 1 { return Err(AppError::missing()); }
    audit(&mut tx, auth.user.id, "feed_deleted", Some(id)).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn rotate(State(s): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    let mut tx = s.db.begin().await?;
    lock_user(&mut tx, auth.user.id).await?;
    let version: i32 = sqlx::query_scalar("UPDATE calendar_feeds SET token_version=token_version+1,updated_at=now() WHERE id=$1 AND user_id=$2 RETURNING token_version").bind(id).bind(auth.user.id).fetch_one(&mut *tx).await?;
    audit(&mut tx, auth.user.id, "feed_token_rotated", Some(id)).await?;
    tx.commit().await?;
    Ok(Json(json!({"url":url(&s,id,version)})))
}
pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\\n").replace(';', "\\;").replace(',', "\\,")
}
fn line(out: &mut String, s: &str) {
    let mut bytes = 0;
    for ch in s.chars() {
        if bytes + ch.len_utf8() > 75 { out.push_str("\r\n "); bytes = 1; }
        out.push(ch);
        bytes += ch.len_utf8();
    }
    out.push_str("\r\n");
}
#[derive(Clone)]
pub struct Published {
    pub id: Uuid,
    pub date: NaiveDate,
    pub title: String,
    pub cancelled: bool,
    pub alarms: Vec<i32>,
    pub sequence: i32,
    pub modified: DateTime<Utc>,
}
pub fn render(name: &str, events: &[Published]) -> String {
    let mut out = String::new();
    for text in ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//CardDue//Calendar//ZH", "CALSCALE:GREGORIAN", "METHOD:PUBLISH", "REFRESH-INTERVAL;VALUE=DURATION:PT6H"] { line(&mut out, text); }
    line(&mut out, &format!("NAME:{}", escape(name)));
    line(&mut out, &format!("X-WR-CALNAME:{}", escape(name)));
    for e in events {
        line(&mut out, "BEGIN:VEVENT");
        line(&mut out, &format!("UID:{}@carddue", e.id));
        line(&mut out, &format!("DTSTAMP:{}", e.modified.format("%Y%m%dT%H%M%SZ")));
        line(&mut out, &format!("LAST-MODIFIED:{}", e.modified.format("%Y%m%dT%H%M%SZ")));
        line(&mut out, &format!("SEQUENCE:{}", e.sequence));
        line(&mut out, &format!("DTSTART;VALUE=DATE:{}", e.date.format("%Y%m%d")));
        line(&mut out, &format!("DTEND;VALUE=DATE:{}", e.date.succ_opt().expect("bounded supported date").format("%Y%m%d")));
        line(&mut out, &format!("SUMMARY:{}", escape(&e.title)));
        line(&mut out, "TRANSP:TRANSPARENT");
        line(&mut out, "CLASS:PRIVATE");
        line(&mut out, if e.cancelled { "STATUS:CANCELLED" } else { "STATUS:CONFIRMED" });
        if !e.cancelled {
            for day in &e.alarms {
                line(&mut out, "BEGIN:VALARM");
                line(&mut out, "ACTION:DISPLAY");
                line(&mut out, &if *day == 0 { "TRIGGER:PT0S".to_string() } else { format!("TRIGGER:-P{day}D") });
                line(&mut out, "DESCRIPTION:CardDue reminder");
                line(&mut out, "END:VALARM");
            }
        }
        line(&mut out, "END:VEVENT");
    }
    line(&mut out, "END:VCALENDAR");
    out
}
pub fn included(feed: &FeedInput, event: &Event) -> bool {
    event.active && feed.kinds.contains(&event.kind) && (feed.card_ids.is_empty() || feed.card_ids.contains(&event.card_id)) && !(feed.hide_paid && event.paid)
}
fn title(feed: &FeedInput, event: &Event, card: &CardInput) -> String {
    let value = match feed.privacy.as_str() {
        "private" => dates::label(&event.kind).to_string(),
        "detailed" => format!("{} {} · {}", card.name, if card.last4.is_empty() { String::new() } else { format!("•••• {}", card.last4) }, event.title),
        _ => format!("{} · {}", card.name, event.title),
    };
    if event.paid { format!("✓ 已还款 · {value}") } else { value }
}
pub async fn subscribe(State(s): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, Path((id, token)): Path<(Uuid, String)>, headers: HeaderMap) -> Result<Response> {
    rate(&s, &format!("feed:{}", peer.ip()), 120).await?;
    // Authenticate before taking locks or performing publication work.
    let initial = sqlx::query("SELECT user_id,token_version FROM calendar_feeds WHERE id=$1").bind(id).fetch_optional(&s.db).await?.ok_or_else(AppError::missing)?;
    let version: i32 = initial.try_get("token_version")?;
    let supplied = token.strip_suffix(".ics").unwrap_or("");
    if !crypto::equal(supplied, &crypto::sign(&s.config.signing_key, "feed", &format!("{id}:{version}"))) { return Err(AppError::missing()); }
    let user: Uuid = initial.try_get("user_id")?;
    let mut tx = s.db.begin().await?;
    // Consistent lock order with all mutations: user, then feed. This also ensures a
    // single feed response cannot mix old and new card data across a transaction.
    let timezone: String = sqlx::query_scalar("SELECT timezone FROM users WHERE id=$1 FOR SHARE").bind(user).fetch_one(&mut *tx).await?;
    let row = sqlx::query("SELECT data,token_version,updated_at FROM calendar_feeds WHERE id=$1 AND user_id=$2 FOR UPDATE").bind(id).bind(user).fetch_one(&mut *tx).await?;
    let current_version: i32 = row.try_get("token_version")?;
    if current_version != version { return Err(AppError::missing()); }
    let feed: FeedInput = serde_json::from_value(row.try_get("data")?)?;
    if !feed.enabled { return Err(AppError::missing()); }
    let feed_updated: DateTime<Utc> = row.try_get("updated_at")?;
    let tz: chrono_tz::Tz = timezone.parse().map_err(|_| AppError::internal())?;
    let today = Utc::now().with_timezone(&tz).date_naive();
    let start = dates::month(today, -3);
    let end = dates::month(today, 25);
    let events = sqlx::query_as::<_, Event>("SELECT * FROM calendar_events WHERE user_id=$1 AND event_date >= $2 AND event_date < $3 ORDER BY id").bind(user).bind(start).bind(end).fetch_all(&mut *tx).await?;
    let rows = sqlx::query("SELECT id,data FROM cards WHERE user_id=$1").bind(user).fetch_all(&mut *tx).await?;
    let mut cards: HashMap<Uuid, CardInput> = HashMap::new();
    for r in rows { cards.insert(r.try_get("id")?, serde_json::from_value(r.try_get("data")?)?); }
    let mut published = Vec::new();
    let mut selected = HashSet::new();
    for e in events.iter().filter(|e| included(&feed, e)) {
        let Some(card) = cards.get(&e.card_id) else { continue; };
        selected.insert(e.id);
        let alarms = if e.paid { Vec::new() } else { feed.alarms_days_before.clone() };
        let title = title(&feed, e, card);
        let hash = crypto::hash(&json!({"date":e.event_date,"title":title,"alarms":alarms,"cancelled":false}).to_string());
        sqlx::query("INSERT INTO calendar_publications(feed_id,event_id,representation_hash,last_date) VALUES($1,$2,$3,$4) ON CONFLICT(feed_id,event_id) DO UPDATE SET representation_hash=EXCLUDED.representation_hash,last_date=EXCLUDED.last_date,cancelled=false,sequence=calendar_publications.sequence+1,modified_at=now() WHERE calendar_publications.representation_hash IS DISTINCT FROM EXCLUDED.representation_hash OR calendar_publications.cancelled=true")
            .bind(id).bind(e.id).bind(hash).bind(e.event_date).execute(&mut *tx).await?;
        published.push(Published { id:e.id, date:e.event_date, title, cancelled:false, alarms, sequence:0, modified:feed_updated });
    }
    let previous = sqlx::query("SELECT event_id,last_date,cancelled FROM calendar_publications WHERE feed_id=$1").bind(id).fetch_all(&mut *tx).await?;
    for old in previous {
        let event_id: Uuid = old.try_get("event_id")?;
        let last_date: NaiveDate = old.try_get("last_date")?;
        if selected.contains(&event_id) { continue; }
        if last_date < start {
            sqlx::query("DELETE FROM calendar_publications WHERE feed_id=$1 AND event_id=$2").bind(id).bind(event_id).execute(&mut *tx).await?;
            continue;
        }
        sqlx::query("UPDATE calendar_publications SET cancelled=true,sequence=sequence+1,modified_at=now() WHERE feed_id=$1 AND event_id=$2 AND cancelled=false").bind(id).bind(event_id).execute(&mut *tx).await?;
        // Never expose a card's current title or date after it has been excluded.
        published.push(Published { id:event_id, date:last_date, title:"CardDue event removed".into(), cancelled:true, alarms:Vec::new(), sequence:0, modified:feed_updated });
    }
    let states = sqlx::query("SELECT event_id,sequence,modified_at FROM calendar_publications WHERE feed_id=$1").bind(id).fetch_all(&mut *tx).await?;
    let mut versions: HashMap<Uuid, (i32, DateTime<Utc>)> = HashMap::new();
    for state in states { versions.insert(state.try_get("event_id")?, (state.try_get("sequence")?, state.try_get("modified_at")?)); }
    for event in &mut published {
        if let Some((sequence, modified)) = versions.get(&event.id) { event.sequence=*sequence; event.modified=*modified; }
    }
    published.sort_by_key(|e| e.id);
    let body = render(&feed.name, &published);
    let etag = format!("\"{}\"", crypto::hash(&body));
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("valid month").and_hms_opt(0,0,0).expect("midnight").and_utc();
    let modified = published.iter().map(|e|e.modified).max().unwrap_or(feed_updated).max(feed_updated).max(month_start);
    let not_modified = headers.get("if-none-match").and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(',').any(|part|part.trim().trim_start_matches("W/")==etag || part.trim()=="*"));
    sqlx::query("UPDATE calendar_feeds SET last_accessed_at=now() WHERE id=$1 AND (last_accessed_at IS NULL OR last_accessed_at<now()-interval '5 minutes')").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    let mut response = if not_modified { StatusCode::NOT_MODIFIED.into_response() } else { body.into_response() };
    response.headers_mut().insert("content-type", HeaderValue::from_static("text/calendar; charset=utf-8"));
    response.headers_mut().insert("cache-control", HeaderValue::from_static("private, no-cache"));
    response.headers_mut().insert("etag", HeaderValue::from_str(&etag).map_err(|_|AppError::internal())?);
    response.headers_mut().insert("last-modified", HeaderValue::from_str(&httpdate::fmt_http_date(modified.into())).map_err(|_|AppError::internal())?);
    Ok(response)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_folding_and_escape() {
        let mut out=String::new(); line(&mut out, &format!("SUMMARY:{}", "💳中文".repeat(40)));
        for l in out.split("\r\n") { assert!(l.len()<=75); }
        assert_eq!(out.replace("\r\n ", "").trim_end(), format!("SUMMARY:{}", "💳中文".repeat(40)));
        assert_eq!(escape("a,b;c\\d\r\nEND:VEVENT"), "a\\,b\\;c\\\\d\\nEND:VEVENT");
    }
    #[test]
    fn all_day_event_and_stable_uid() {
        let e=Published {id:Uuid::new_v4(),date:dates::day(2028,2,29),title:"示例".into(),cancelled:false,alarms:vec![0,1],sequence:4,modified:Utc::now()};
        let body=render("My feed",std::slice::from_ref(&e));
        assert!(body.contains("DTEND;VALUE=DATE:20280301\r\n"));
        assert!(body.contains("SEQUENCE:4\r\n"));
        assert!(body.contains("TRIGGER:PT0S\r\n"));
        let mut changed=e.clone();changed.date=dates::day(2028,3,1);changed.sequence+=1;
        assert!(render("My feed",&[changed]).contains(&format!("UID:{}@carddue",e.id)));
    }
}
