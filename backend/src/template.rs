use crate::{error::{AppError, Result}, model::{TemplateInput, text}};
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::io::{self, Write};

#[derive(Clone, Serialize, Deserialize)]
pub struct Rendered {
    pub title: String, pub body: String, pub html: String, pub url: String,
    pub group: String, pub sound: String, pub level: String,
}
struct LimitedWriter { buf: Vec<u8>, max: usize }
impl Write for LimitedWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.buf.len()+b.len()>self.max { return Err(io::Error::other("rendered output too large")); }
        self.buf.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn render_one(source: &str, context: &Value, html: bool, max: usize) -> Result<String> {
    let mut env = Environment::new();
    env.set_fuel(Some(30_000));
    env.set_recursion_limit(30);
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_auto_escape_callback(move |_|if html {AutoEscape::Html} else {AutoEscape::None});
    // No loader, filesystem, network or application functions are exposed.
    env.add_template("message",source).map_err(|e|AppError::bad(format!("Template syntax: {e}")))?;
    let template = env.get_template("message").map_err(|_|AppError::internal())?;
    let mut output = LimitedWriter {buf:Vec::new(),max};
    template.render_captured_to(context,&mut output).map_err(|e|AppError::bad(format!("Template rendering: {}",e.kind())))?;
    String::from_utf8(output.buf).map_err(|_|AppError::internal())
}
pub fn sample(base: &str) -> Value {
    json!({"card":{"id":"00000000-0000-0000-0000-000000000001","name":"示例信用卡","issuer":"示例银行","last4":"1234","currency":"CNY"},"event":{"type":"payment_due","label":"还款日","date":"2026-09-18","days_until":3},"cycle":{"statement_date":"2026-09-01","due_date":"2026-09-18","amount":"1234.56","minimum_payment":"123.45"},"user":{"timezone":"Asia/Shanghai"},"app":{"url":base}})
}
pub fn validate(t: &TemplateInput, base: &str) -> Result<()> {
    text(&t.name,1,100)?;
    for value in [&t.title,&t.body,&t.html,&t.url,&t.group,&t.sound,&t.level] {text(value,0,16000)?;}
    let mut context = sample(base);
    render(t,&context,base)?;
    context["cycle"]["amount"] = Value::Null;
    context["card"]["last4"] = json!("");
    render(t,&context,base)?;
    Ok(())
}
pub fn render(t: &TemplateInput, context: &Value, base: &str) -> Result<Rendered> {
    let title = render_one(&t.title,context,false,512)?;
    if title.contains(['\r','\n']) {return Err(AppError::bad("Title cannot contain line breaks"));}
    let raw_html = render_one(&t.html,context,true,64000)?;
    let html = ammonia::Builder::default().url_schemes(["https","mailto"].into_iter().collect()).clean(&raw_html).to_string();
    let url = render_one(&t.url,context,false,2000)?;
    if !url.is_empty() {
        let u = url::Url::parse(&url).map_err(|_|AppError::bad("Notification link must be an absolute URL"))?;
        let b = url::Url::parse(base).map_err(|_|AppError::internal())?;
        if u.origin()!=b.origin() || !u.username().is_empty() || u.password().is_some() {return Err(AppError::bad("Notification links must use the application origin"));}
    }
    let level = render_one(&t.level,context,false,32)?;
    if !["","active","passive","timeSensitive"].contains(&level.as_str()) {return Err(AppError::bad("Unsupported Bark level; critical alerts are not enabled"));}
    Ok(Rendered {title,body:render_one(&t.body,context,false,12000)?,html,url,group:render_one(&t.group,context,false,100)?,sound:render_one(&t.sound,context,false,64)?,level})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_template_and_escaping() {
        let mut context=sample("https://carddue.example");
        context["card"]["name"]=json!("<script>alert(1)</script>");
        let r=render(&TemplateInput::default(),&context,"https://carddue.example").unwrap();
        assert!(!r.html.contains("<script>"));
        assert!(r.html.contains("&lt;script&gt;"));
    }
    #[test]
    fn errors_and_output_limits() {
        let mut t=TemplateInput{title:"{{ unknown }}".into(),..TemplateInput::default()};
        assert!(validate(&t,"https://carddue.example").is_err());
        t.title="{% for n in range(999999) %}x{% endfor %}".into();
        assert!(validate(&t,"https://carddue.example").is_err());
        t.title="hello".into();t.url="https://evil.example".into();
        assert!(validate(&t,"https://carddue.example").is_err());
    }
}
