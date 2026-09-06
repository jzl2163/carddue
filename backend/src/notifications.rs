use crate::{AppState, auth::{Auth,audit,lock_user,rate}, cards::owned_cards, crypto, error::{AppError,Result}, model::{ConnectionInput,ConnectionView,RuleInput,TemplateInput,text}, planner, providers, template, web::ApiJson};
use axum::{extract::{State,Path,Query},Json};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value,json};
use sqlx::Row;
use uuid::Uuid;

pub fn aad(user:Uuid,id:Uuid,kind:&str) -> String {format!("{user}/{id}/{kind}")}
pub async fn connections(State(s):State<AppState>,auth:Auth) -> Result<Json<Vec<ConnectionView>>> {
    let rows=sqlx::query("SELECT id,name,kind,enabled FROM notification_connections WHERE user_id=$1 ORDER BY created_at DESC").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut out=Vec::new();for r in rows {out.push(ConnectionView{id:r.try_get("id")?,name:r.try_get("name")?,kind:r.try_get("kind")?,enabled:r.try_get("enabled")?});}Ok(Json(out))
}
async fn save_connection(s:AppState,auth:Auth,id:Option<Uuid>,input:ConnectionInput) -> Result<Json<Value>> {
    rate(&s,&format!("connection:{}",auth.user.id),20).await?;text(&input.name,1,100)?;
    if !["bark","email","telegram","webpush","webhook"].contains(&input.kind.as_str()) {return Err(AppError::bad("Unknown channel"));}
    if let Some(config)=&input.config {providers::validate(&input.kind,config,&s.config).await?;}
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let key=id.unwrap_or_else(Uuid::new_v4);
    let encrypted=if let Some(config)=&input.config {Some(crypto::encrypt(&s.config.encryption_key,&aad(auth.user.id,key,&input.kind),&serde_json::to_vec(config)?)?)}else{None};
    if id.is_some() {
        let kind:String=sqlx::query_scalar("SELECT kind FROM notification_connections WHERE id=$1 AND user_id=$2").bind(key).bind(auth.user.id).fetch_one(&mut *tx).await?;
        if kind!=input.kind {return Err(AppError::bad("Connection type cannot be changed; create another connection"));}
        sqlx::query("UPDATE notification_connections SET name=$1,enabled=$2,encrypted_config=COALESCE($3,encrypted_config),updated_at=now() WHERE id=$4 AND user_id=$5")
            .bind(&input.name).bind(input.enabled).bind(encrypted).bind(key).bind(auth.user.id).execute(&mut *tx).await?;
    }else{
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM notification_connections WHERE user_id=$1").bind(auth.user.id).fetch_one(&mut *tx).await?;
        if count>=50 {return Err(AppError::bad("Connection limit reached"));}
        let encrypted=encrypted.ok_or_else(||AppError::bad("Connection config is required"))?;
        sqlx::query("INSERT INTO notification_connections(id,user_id,name,kind,encrypted_config,enabled) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(key).bind(auth.user.id).bind(&input.name).bind(&input.kind).bind(encrypted).bind(input.enabled).execute(&mut *tx).await?;
    }
    planner::reconcile(&mut tx,auth.user.id,&s.config).await?;audit(&mut tx,auth.user.id,"connection_saved",Some(key)).await?;tx.commit().await?;Ok(Json(json!({"id":key})))
}
pub async fn create_connection(State(s):State<AppState>,auth:Auth,ApiJson(i):ApiJson<ConnectionInput>) -> Result<Json<Value>> {save_connection(s,auth,None,i).await}
pub async fn update_connection(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>,ApiJson(i):ApiJson<ConnectionInput>) -> Result<Json<Value>> {save_connection(s,auth,Some(id),i).await}
pub async fn delete_connection(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>> {
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let result=sqlx::query("UPDATE notification_connections SET enabled=false,updated_at=now() WHERE id=$1 AND user_id=$2").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    if result.rows_affected()!=1 {return Err(AppError::missing());}
    sqlx::query("UPDATE notification_jobs SET status='cancelled' WHERE connection_id=$1 AND user_id=$2 AND status IN ('pending','retry','processing')").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    audit(&mut tx,auth.user.id,"connection_disabled",Some(id)).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestInput {pub template_id:Option<Uuid>}
pub async fn test_connection(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>,ApiJson(input):ApiJson<TestInput>) -> Result<Json<Value>> {
    rate(&s,&format!("test:{}",auth.user.id),5).await?;
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let enabled:bool=sqlx::query_scalar("SELECT enabled FROM notification_connections WHERE id=$1 AND user_id=$2").bind(id).bind(auth.user.id).fetch_one(&mut *tx).await?;
    if !enabled {return Err(AppError::bad("Enable the connection before testing"));}
    let t:TemplateInput=if let Some(tid)=input.template_id {let value:Value=sqlx::query_scalar("SELECT data FROM notification_templates WHERE id=$1 AND user_id=$2").bind(tid).bind(auth.user.id).fetch_one(&mut *tx).await?;serde_json::from_value(value)?}else{TemplateInput::default()};
    let job=Uuid::new_v4();
    sqlx::query("INSERT INTO notification_jobs(id,user_id,connection_id,scheduled_at,next_attempt_at,expires_at,template_snapshot,context) VALUES($1,$2,$3,now(),now(),now()+interval '1 hour',$4,$5)")
        .bind(job).bind(auth.user.id).bind(id).bind(json!(t)).bind(template::sample(&s.config.base_url)).execute(&mut *tx).await?;
    audit(&mut tx,auth.user.id,"test_notification_queued",Some(job)).await?;tx.commit().await?;Ok(Json(json!({"id":job,"status":"pending"})))
}
pub async fn vapid(State(s):State<AppState>,_auth:Auth) -> Result<Json<Value>> {
    let Some(key)=&s.config.vapid_private else{return Ok(Json(json!({"enabled":false,"public_key":null})));};
    let builder=web_push::VapidSignatureBuilder::from_base64_no_sub(key).map_err(|_|AppError::internal())?;
    Ok(Json(json!({"enabled":true,"public_key":URL_SAFE_NO_PAD.encode(builder.get_public_key())})))
}
pub async fn templates(State(s):State<AppState>,auth:Auth) -> Result<Json<Value>> {
    let rows=sqlx::query("SELECT id,data FROM notification_templates WHERE user_id=$1 ORDER BY updated_at DESC").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut out=Vec::new();for r in rows{out.push(json!({"id":r.try_get::<Uuid,_>("id")?,"data":r.try_get::<Value,_>("data")?}));}Ok(Json(json!(out)))
}
async fn save_template(s:AppState,auth:Auth,id:Option<Uuid>,t:TemplateInput) -> Result<Json<Value>> {
    template::validate(&t,&s.config.base_url)?;
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;let key=id.unwrap_or_else(Uuid::new_v4);
    if id.is_some(){
        let result=sqlx::query("UPDATE notification_templates SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3").bind(json!(t)).bind(key).bind(auth.user.id).execute(&mut *tx).await?;
        if result.rows_affected()!=1{return Err(AppError::missing());}
    }else{
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM notification_templates WHERE user_id=$1").bind(auth.user.id).fetch_one(&mut *tx).await?;
        if count>=100{return Err(AppError::bad("Template limit reached"));}
        sqlx::query("INSERT INTO notification_templates(id,user_id,data) VALUES($1,$2,$3)").bind(key).bind(auth.user.id).bind(json!(t)).execute(&mut *tx).await?;
    }
    planner::reconcile(&mut tx,auth.user.id,&s.config).await?;audit(&mut tx,auth.user.id,"template_saved",Some(key)).await?;tx.commit().await?;Ok(Json(json!({"id":key})))
}
pub async fn create_template(State(s):State<AppState>,auth:Auth,ApiJson(t):ApiJson<TemplateInput>) -> Result<Json<Value>>{save_template(s,auth,None,t).await}
pub async fn update_template(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>,ApiJson(t):ApiJson<TemplateInput>) -> Result<Json<Value>>{save_template(s,auth,Some(id),t).await}
pub async fn preview(State(s):State<AppState>,auth:Auth,ApiJson(t):ApiJson<TemplateInput>) -> Result<Json<Value>>{
    rate(&s,&format!("preview:{}",auth.user.id),60).await?;template::validate(&t,&s.config.base_url)?;
    Ok(Json(json!(template::render(&t,&template::sample(&s.config.base_url),&s.config.base_url)?)))
}
pub async fn delete_template(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>>{
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let used:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_rules WHERE user_id=$1 AND data->>'template_id'=$2)").bind(auth.user.id).bind(id.to_string()).fetch_one(&mut *tx).await?;
    if used{return Err(AppError::bad("This template is referenced by a reminder rule"));}
    let result=sqlx::query("DELETE FROM notification_templates WHERE id=$1 AND user_id=$2").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    if result.rows_affected()!=1{return Err(AppError::missing());}
    audit(&mut tx,auth.user.id,"template_deleted",Some(id)).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))
}
pub async fn rules(State(s):State<AppState>,auth:Auth) -> Result<Json<Value>>{
    let rows=sqlx::query("SELECT id,data FROM notification_rules WHERE user_id=$1 ORDER BY created_at DESC").bind(auth.user.id).fetch_all(&s.db).await?;
    let mut out=Vec::new();for r in rows{out.push(json!({"id":r.try_get::<Uuid,_>("id")?,"data":r.try_get::<Value,_>("data")?}));}Ok(Json(json!(out)))
}
async fn save_rule(s:AppState,auth:Auth,id:Option<Uuid>,r:RuleInput) -> Result<Json<Value>>{
    r.validate()?;let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;owned_cards(&mut tx,auth.user.id,&r.card_ids).await?;
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM notification_connections WHERE user_id=$1 AND id=ANY($2)").bind(auth.user.id).bind(&r.connection_ids).fetch_one(&mut *tx).await?;
    if count as usize!=r.connection_ids.len(){return Err(AppError::missing());}
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_templates WHERE user_id=$1 AND id=$2)").bind(auth.user.id).bind(r.template_id).fetch_one(&mut *tx).await?;
    if !exists{return Err(AppError::missing());}
    let key=id.unwrap_or_else(Uuid::new_v4);
    if id.is_some(){
        let result=sqlx::query("UPDATE notification_rules SET data=$1,updated_at=now() WHERE id=$2 AND user_id=$3").bind(json!(r)).bind(key).bind(auth.user.id).execute(&mut *tx).await?;
        if result.rows_affected()!=1{return Err(AppError::missing());}
    }else{
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM notification_rules WHERE user_id=$1").bind(auth.user.id).fetch_one(&mut *tx).await?;
        if count>=100{return Err(AppError::bad("Rule limit reached"));}
        sqlx::query("INSERT INTO notification_rules(id,user_id,data) VALUES($1,$2,$3)").bind(key).bind(auth.user.id).bind(json!(r)).execute(&mut *tx).await?;
    }
    planner::reconcile(&mut tx,auth.user.id,&s.config).await?;audit(&mut tx,auth.user.id,"rule_saved",Some(key)).await?;tx.commit().await?;Ok(Json(json!({"id":key})))
}
pub async fn create_rule(State(s):State<AppState>,auth:Auth,ApiJson(r):ApiJson<RuleInput>) -> Result<Json<Value>>{save_rule(s,auth,None,r).await}
pub async fn update_rule(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>,ApiJson(r):ApiJson<RuleInput>) -> Result<Json<Value>>{save_rule(s,auth,Some(id),r).await}
pub async fn delete_rule(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>>{
    let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let result=sqlx::query("UPDATE notification_rules SET data=jsonb_set(data,'{enabled}','false'),updated_at=now() WHERE id=$1 AND user_id=$2").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    if result.rows_affected()!=1{return Err(AppError::missing());}
    planner::reconcile(&mut tx,auth.user.id,&s.config).await?;audit(&mut tx,auth.user.id,"rule_disabled",Some(id)).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct Page {pub offset:Option<i64>}
pub async fn history(State(s):State<AppState>,auth:Auth,Query(p):Query<Page>) -> Result<Json<Value>>{
    let offset=p.offset.unwrap_or(0).clamp(0,100000);
    let rows=sqlx::query("SELECT j.id,j.status,j.scheduled_at,j.sent_at,j.attempt_count,j.last_error,j.context,c.name,c.kind FROM notification_jobs j JOIN notification_connections c ON c.id=j.connection_id WHERE j.user_id=$1 ORDER BY j.created_at DESC,j.id LIMIT 100 OFFSET $2").bind(auth.user.id).bind(offset).fetch_all(&s.db).await?;
    let mut out=Vec::new();for r in rows{out.push(json!({"id":r.try_get::<Uuid,_>("id")?,"status":r.try_get::<String,_>("status")?,"scheduled_at":r.try_get::<chrono::DateTime<chrono::Utc>,_>("scheduled_at")?,"sent_at":r.try_get::<Option<chrono::DateTime<chrono::Utc>>,_>("sent_at")?,"attempt_count":r.try_get::<i32,_>("attempt_count")?,"error":r.try_get::<Option<String>,_>("last_error")?,"context":r.try_get::<Value,_>("context")?,"connection":r.try_get::<String,_>("name")?,"kind":r.try_get::<String,_>("kind")?}));}Ok(Json(json!(out)))
}
pub async fn attempts(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>>{
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_jobs WHERE id=$1 AND user_id=$2)").bind(id).bind(auth.user.id).fetch_one(&s.db).await?;if !exists{return Err(AppError::missing());}
    let rows=sqlx::query("SELECT a.* FROM delivery_attempts a JOIN notification_jobs j ON j.id=a.job_id WHERE a.job_id=$1 AND j.user_id=$2 ORDER BY a.id DESC LIMIT 100").bind(id).bind(auth.user.id).fetch_all(&s.db).await?;
    let mut out=Vec::new();for r in rows{out.push(json!({"started_at":r.try_get::<chrono::DateTime<chrono::Utc>,_>("started_at")?,"finished_at":r.try_get::<chrono::DateTime<chrono::Utc>,_>("finished_at")?,"success":r.try_get::<bool,_>("success")?,"status":r.try_get::<Option<i32>,_>("response_status")?,"error_code":r.try_get::<Option<String>,_>("error_code")?,"rendered":r.try_get::<Value,_>("rendered")?}));}Ok(Json(json!(out)))
}
pub async fn retry(State(s):State<AppState>,auth:Auth,Path(id):Path<Uuid>) -> Result<Json<Value>>{
    rate(&s,&format!("retry:{}",auth.user.id),10).await?;let mut tx=s.db.begin().await?;lock_user(&mut tx,auth.user.id).await?;
    let result=sqlx::query("UPDATE notification_jobs SET status='pending',attempt_count=0,next_attempt_at=now(),last_error=NULL WHERE id=$1 AND user_id=$2 AND status='dead' AND expires_at>now()").bind(id).bind(auth.user.id).execute(&mut *tx).await?;
    if result.rows_affected()!=1{return Err(AppError::bad("Only failed, unexpired deliveries can be retried; sent notifications are not resent"));}
    planner::reconcile(&mut tx,auth.user.id,&s.config).await?;audit(&mut tx,auth.user.id,"delivery_retry_requested",Some(id)).await?;tx.commit().await?;Ok(Json(json!({"ok":true})))
}
