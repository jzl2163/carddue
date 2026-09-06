use crate::{AppState, auth::{Auth, audit, lock_user, rate}, cards::owned_cards, crypto, dates, error::{AppError, Result}, model::{CardInput, Event, FeedInput}, web::ApiJson};
use axum::{extract::{State,Path,ConnectInfo}, http::{HeaderMap,HeaderValue,StatusCode}, response::{Response,IntoResponse}, Json};
use chrono::{DateTime,Datelike,NaiveDate,Utc};
use serde_json::{Value,json};
use sqlx::Row;
use std::{collections::HashMap,net::SocketAddr};
use uuid::Uuid;

pub fn url(s:&AppState,id:Uuid,version:i32) -> String {
    format!("{}/cal/v1/{}/{}.ics",s.config.base_url,id,crypto::sign(&s.config.signing_key,"feed",&format!("{id}:{version}")))
}
pub async fn list(State(s):State<AppState>,auth:Auth) -> Result<Json<Value>> {
    let rows=sqlx::query("SELECT id,data,token_version,last_accessed_at FROM calendar_feeds WHERE user_id=$1 ORDER BY updated_at DESC").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut feeds=Vec::new();for r in rows {let id=r.try_get::<Uuid,_>("id")?;feeds.push(json!({"id":id,"data":r.try_get::<Value,_>("data")?,"url":url(&s,id,r.try_get("token_version")?),"last_accessed_at":r.try_get::<Option<DateTime<Utc>>,_>("last_accessed_at")?}));}
    Ok(Json(json!(feeds)))
}
async fn save(s:AppState,auth:Auth,id:Option<Uuid>,f:FeedInput) -> Result<Json<Value>> {
    f.validate()?;let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;owned_cards(&mut tx,auth.user.id,&f.card_ids).await?;
    let key=id.unwrap_or_else(Uuid::new_v4);
    let version:i32=if id.is_some() {
        sqlx::query_scalar("UPDATE calendar_feeds SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3 RETURNING token_version").bind(json!(f)).bind(key).bind(auth.user.id).fetch_one(&mut *tx).await?
    } else {
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM calendar_feeds WHERE user_id=$1").bind(auth.user.id).fetch_one(&mut *tx).await?;
        if count>=20 {return Err(AppError::bad("Calendar feed limit reached"));}
        sqlx::query("INSERT INTO calendar_feeds(id,user_id,data) VALUES($1,$2,$3)").bind(key).bind(auth.user.id).bind(json!(f)).execute(&mut *tx).await?;1
    };
    audit(&mut tx,auth.user.id,"feed_saved",Some(key)).await?;tx.commit().await?;Ok(Json(json!({"id":key,"url":url(&s,key,version)})))
}
pub async fn create(State(s):State<AppState>,auth:Auth,ApiJson(f):ApiJson<FeedInput>) -> Result<Json<Value>> {save(s,auth,None,f).await}
pub async fn update(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>,ApiJson(f):ApiJson<FeedInput>) -> Result<Json<Value>> {save(s,auth,Some(id),f).await}
pub async fn delete(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>> {
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let r=sqlx::query("DELETE FROM calendar_feeds WHERE id=$1 AND user_id=$2").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    if r.rows_affected()!=1 {return Err(AppError::missing());}
    audit(&mut tx,auth.user.id,"feed_deleted",Some(id)).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))
}
pub async fn rotate(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>> {
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let version:i32=sqlx::query_scalar("UPDATE calendar_feeds SET token_version=token_version+1,updated_at=now() WHERE id=$1 AND user_id=$2 RETURNING token_version").bind(id).bind(auth.user.id).fetch_one(&mut *tx).await?;
    audit(&mut tx,auth.user.id,"feed_token_rotated",Some(id)).await?;tx.commit().await?;Ok(Json(json!({"url":url(&s,id,version)})))
}
pub fn escape(s:&str) -> String {s.replace('\\',"\\\\").replace("\r\n","\n").replace('\r',"\n").replace('\n',"\\n").replace(';',"\\;").replace(',',"\\,")}
fn line(out:&mut String,s:&str) {
    // RFC 5545 folding is in octets, not Unicode code points.
    let mut n=0;
    for ch in s.chars() {let bytes=ch.len_utf8();if n+bytes>75 {out.push_str("\r\n ");n=1;}out.push(ch);n+=bytes;}
    out.push_str("\r\n");
}
pub fn render(f:&FeedInput,events:&[Event],cards:&HashMap<Uuid,CardInput>,feed_updated:DateTime<Utc>) -> String {
    let mut out=String::new();
    for s in ["BEGIN:VCALENDAR","VERSION:2.0","PRODID:-//CardDue//Calendar//ZH","CALSCALE:GREGORIAN","METHOD:PUBLISH","REFRESH-INTERVAL;VALUE=DURATION:PT6H"] {line(&mut out,s);}
    line(&mut out,&format!("NAME:{}",escape(&f.name)));line(&mut out,&format!("X-WR-CALNAME:{}",escape(&f.name)));
    for e in events {
        let Some(c)=cards.get(&e.card_id) else {continue;};
        let visible=e.active && f.kinds.contains(&e.kind) && (f.card_ids.is_empty() || f.card_ids.contains(&e.card_id)) && !(f.hide_paid && e.paid);
        let title=match f.privacy.as_str(){"private"=>dates::label(&e.kind).to_string(),"detailed"=>format!("{} {} · {}",c.name,if c.last4.is_empty(){String::new()}else{format!("•••• {}",c.last4)},e.title),_=>format!("{} · {}",c.name,e.title)};
        let title=if e.paid && visible {format!("✓ 已还款 · {title}")}else{title};
        line(&mut out,"BEGIN:VEVENT");line(&mut out,&format!("UID:{}@carddue",e.id));
        let updated=e.updated_at.max(feed_updated);
        line(&mut out,&format!("DTSTAMP:{}",updated.format("%Y%m%dT%H%M%SZ")));
        line(&mut out,&format!("LAST-MODIFIED:{}",updated.format("%Y%m%dT%H%M%SZ")));
        line(&mut out,&format!("SEQUENCE:{}",e.revision));
        line(&mut out,&format!("DTSTART;VALUE=DATE:{}",e.event_date.format("%Y%m%d")));
        line(&mut out,&format!("DTEND;VALUE=DATE:{}",e.event_date.succ_opt().expect("bounded date").format("%Y%m%d")));
        line(&mut out,&format!("SUMMARY:{}",escape(&title)));
        line(&mut out,"TRANSP:TRANSPARENT");line(&mut out,"CLASS:PRIVATE");
        line(&mut out,if visible{"STATUS:CONFIRMED"}else{"STATUS:CANCELLED"});
        // No amounts, notes, email addresses, application secrets or repayment links in ICS.
        if visible && !e.paid {for days in &f.alarms_days_before {
            line(&mut out,"BEGIN:VALARM");line(&mut out,"ACTION:DISPLAY");
            line(&mut out,&format!("TRIGGER:-P{days}D"));line(&mut out,"DESCRIPTION:CardDue reminder");line(&mut out,"END:VALARM");
        }}
        line(&mut out,"END:VEVENT");
    }
    line(&mut out,"END:VCALENDAR");out
}
pub async fn subscribe(State(s):State<AppState>,ConnectInfo(peer):ConnectInfo<SocketAddr>,Path((id,token)):Path<(Uuid,String)>,headers:HeaderMap) -> Result<Response> {
    rate(&s,&format!("feed:{}",peer.ip()),120).await?;
    let row=sqlx::query("SELECT f.user_id,f.data,f.token_version,f.updated_at,u.timezone FROM calendar_feeds f JOIN users u ON u.id=f.user_id WHERE f.id=$1").bind(id).fetch_optional(&s.db).await?.ok_or_else(AppError::missing)?;
    let version:i32=row.try_get("token_version")?;
    let expected=crypto::sign(&s.config.signing_key,"feed",&format!("{id}:{version}"));
    if !crypto::equal(token.strip_suffix(".ics").unwrap_or(""),&expected) {return Err(AppError::missing());}
    let feed:FeedInput=serde_json::from_value(row.try_get("data")?)?;
    if !feed.enabled {return Err(AppError::missing());}
    let user:Uuid=row.try_get("user_id")?;let updated:DateTime<Utc>=row.try_get("updated_at")?;
    let timezone:String=row.try_get("timezone")?;let tz:chrono_tz::Tz=timezone.parse().map_err(|_| AppError::internal())?;
    let today=Utc::now().with_timezone(&tz).date_naive();let start=dates::month(today,-3);let end=dates::month(today,25);
    let events=sqlx::query_as::<_,Event>("SELECT * FROM calendar_events WHERE user_id=$1 AND event_date >= $2 AND event_date < $3 ORDER BY id").bind(user).bind(start).bind(end).fetch_all(&s.db).await?;
    let rows=sqlx::query("SELECT id,data FROM cards WHERE user_id=$1").bind(user).fetch_all(&s.db).await?;
    let mut cards=HashMap::new();for r in rows {cards.insert(r.try_get("id")?,serde_json::from_value(r.try_get("data")?)?);}
    let body=render(&feed,&events,&cards,updated);let etag=format!("\"{}\"",crypto::hash(&body));
    let month_start=NaiveDate::from_ymd_opt(today.year(),today.month(),1).expect("valid month").and_hms_opt(0,0,0).expect("midnight").and_utc();
    let modified=events.iter().map(|e|e.updated_at).max().unwrap_or(updated).max(updated).max(month_start);
    let not_modified=headers.get("if-none-match").and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(',').any(|part|part.trim().trim_start_matches("W/")==etag || part.trim()=="*"));
    sqlx::query("UPDATE calendar_feeds SET last_accessed_at=now() WHERE id=$1 AND (last_accessed_at IS NULL OR last_accessed_at<now()-interval '5 minutes')").bind(id).execute(&s.db).await?;
    let mut response=if not_modified {StatusCode::NOT_MODIFIED.into_response()}else{body.into_response()};
    response.headers_mut().insert("content-type",HeaderValue::from_static("text/calendar; charset=utf-8"));
    response.headers_mut().insert("cache-control",HeaderValue::from_static("private, no-cache"));
    response.headers_mut().insert("etag",HeaderValue::from_str(&etag).map_err(|_| AppError::internal())?);
    response.headers_mut().insert("last-modified",HeaderValue::from_str(&httpdate::fmt_http_date(modified.into())).map_err(|_| AppError::internal())?);
    Ok(response)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_folding_and_escape() {
        let mut out=String::new();line(&mut out,&format!("SUMMARY:{}", "💳中文".repeat(40)));
        for l in out.split("\r\n") {assert!(l.len()<=75);}
        assert_eq!(out.replace("\r\n ","").trim_end(),format!("SUMMARY:{}","💳中文".repeat(40)));
        assert_eq!(escape("a,b;c\\d\r\nEND:VEVENT"),"a\\,b\\;c\\\\d\\nEND:VEVENT");
    }
}
