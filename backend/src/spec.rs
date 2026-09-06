use crate::{auth::{AccountInput,LoginInput,PasswordInput,SetupInput,User},model::*};
use serde_json::{Value,json};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(info(title="CardDue API",version="0.1.0",description="Cookie sessions. Mutations require the configured Origin and X-CSRF-Token. No credentials are returned. PATCH resources accept complete validated input objects, except CyclePatch."),components(schemas(CardInput,CardView,CyclePatch,CycleView,MilestoneInput,FeedInput,TemplateInput,RuleInput,ConnectionInput,ConnectionView,Event,LoginInput,SetupInput,AccountInput,PasswordInput,User)))]
struct ApiDoc;
pub fn document()->Value{
    let mut d=serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI serialization");
    d["servers"]=json!([{"url":"/api/v1"}]);
    d["components"]["securitySchemes"]=json!({"session":{"type":"apiKey","in":"cookie","name":"__Host-carddue","description":"Development HTTP uses carddue. Cookie is HttpOnly; obtain CSRF token from /auth/session."}});
    d["security"]=json!([{"session":[]}]);
    let definitions:[(&str,&str,Option<&str>);43]=[
        ("/auth/status","get",None),("/auth/setup","post",Some("SetupInput")),("/auth/register","post",Some("SetupInput")),("/auth/login","post",Some("LoginInput")),("/auth/logout","post",None),("/auth/logout-all","post",None),("/auth/session","get",None),
        ("/account","patch",Some("AccountInput")),("/account/password","post",Some("PasswordInput")),("/dashboard","get",None),
        ("/cards","get",None),("/cards","post",Some("CardInput")),("/cards/{id}","get",None),("/cards/{id}","patch",Some("CardInput")),("/cards/{id}","delete",None),("/cards/{id}/restore","post",None),
        ("/cards/{card}/cycles","get",None),("/cards/{card}/cycles/{cycle}","patch",Some("CyclePatch")),("/cards/{card}/cycles/{cycle}/pay","post",None),("/cards/{card}/cycles/{cycle}/pay","delete",None),
        ("/milestones","get",None),("/milestones","post",Some("MilestoneInput")),("/milestones/{id}","patch",Some("MilestoneInput")),("/milestones/{id}","delete",None),
        ("/calendar/feeds","get",None),("/calendar/feeds","post",Some("FeedInput")),("/calendar/feeds/{id}","patch",Some("FeedInput")),("/calendar/feeds/{id}","delete",None),("/calendar/feeds/{id}/rotate-token","post",None),
        ("/notification/connections","get",None),("/notification/connections","post",Some("ConnectionInput")),("/notification/connections/{id}","patch",Some("ConnectionInput")),("/notification/connections/{id}","delete",None),
        ("/notification/templates","get",None),("/notification/templates","post",Some("TemplateInput")),("/notification/templates/{id}","patch",Some("TemplateInput")),("/notification/templates/{id}","delete",None),("/notification/templates/preview","post",Some("TemplateInput")),("/notification/templates/validate","post",Some("TemplateInput")),
        ("/notification/rules","get",None),("/notification/rules","post",Some("RuleInput")),("/notification/rules/{id}","patch",Some("RuleInput")),("/notification/rules/{id}","delete",None),
    ];
    for (path,method,schema) in definitions{
        let id=format!("{}_{}",method,path.replace(['/','{','}'],"_"));
        let mut op=json!({"operationId":id,"responses":{"200":{"description":"Success","content":{"application/json":{"schema":{}}}},"400":{"description":"VALIDATION_ERROR"},"401":{"description":"UNAUTHORIZED"},"403":{"description":"CSRF or ownership violation"},"404":{"description":"Resource not found"},"429":{"description":"RATE_LIMITED"}}});
        let mut params=Vec::new();for part in path.split('/') {if part.starts_with('{'){params.push(json!({"name":part.trim_matches(['{','}']),"in":"path","required":true,"schema":{"type":"string","format":"uuid"}}));}}
        if method!="get"{params.push(json!({"name":"X-CSRF-Token","in":"header","required":!path.starts_with("/auth/"),"schema":{"type":"string"}}));}
        op["parameters"]=json!(params);
        if let Some(name)=schema{op["requestBody"]=json!({"required":true,"content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/{name}")}}}});}
        if ["/auth/status","/auth/setup","/auth/register","/auth/login"].contains(&path){op["security"]=json!([]);}
        d["paths"][path][method]=op;
    }
    for (path,method) in [("/notification/connections/{id}/test","post"),("/notification/webpush/key","get"),("/notification/deliveries","get"),("/notification/deliveries/{id}/attempts","get"),("/notification/deliveries/{id}/retry","post"),("/settings/system","get"),("/settings/audit","get"),("/openapi.json","get")]{
        let mut op=json!({"operationId":format!("{}_{}",method,path.replace(['/','{','}'],"_")),"responses":{"200":{"description":"Success","content":{"application/json":{"schema":{}}}}}});
        if path.contains("{id}"){op["parameters"]=json!([{"name":"id","in":"path","required":true,"schema":{"type":"string","format":"uuid"}}]);}
        if path.ends_with("/test"){op["requestBody"]=json!({"required":true,"content":{"application/json":{"schema":{"type":"object","properties":{"template_id":{"type":["string","null"],"format":"uuid"}}}}}});}
        d["paths"][path][method]=op;
    }
    d
}
