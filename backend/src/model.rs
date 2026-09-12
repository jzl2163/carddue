use crate::error::{AppError, Result};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

fn yes() -> bool {
    true
}
fn currency() -> String {
    "CNY".into()
}
fn color() -> String {
    "#2563eb".into()
}
fn due_mode() -> String {
    "fixed_day".into()
}
fn one() -> i32 {
    1
}
fn none() -> String {
    "none".into()
}
fn privacy() -> String {
    "normal".into()
}
fn kinds() -> Vec<String> {
    vec![
        "payment_due".into(),
        "statement".into(),
        "annual_fee".into(),
        "benefit".into(),
        "custom".into(),
    ]
}
fn time() -> String {
    "09:00".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CardInput {
    pub name: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub network: String,
    #[serde(default)]
    pub last4: String,
    #[serde(default = "color")]
    pub color: String,
    #[serde(default = "currency")]
    pub currency: String,
    pub statement_day: u32,
    #[serde(default)]
    pub statement_last_day: bool,
    #[serde(default = "due_mode")]
    pub due_mode: String,
    pub due_day: u32,
    #[serde(default)]
    pub due_last_day: bool,
    #[serde(default = "one")]
    pub due_month_offset: i32,
    #[serde(default)]
    pub due_offset_days: i64,
    #[serde(default = "none")]
    pub weekend_adjustment: String,
    pub annual_fee_month: Option<u32>,
    pub annual_fee_day: Option<u32>,
    pub annual_fee_amount: Option<String>,
    #[serde(default)]
    pub notes: String,
}
impl CardInput {
    pub fn effective_timezone(&self, account: &str) -> Result<chrono_tz::Tz> {
        self.timezone
            .as_deref()
            .unwrap_or(account)
            .parse()
            .map_err(|_| AppError::bad("卡片时区必须是有效的 IANA 时区名称"))
    }
    pub fn validate(&self) -> Result<()> {
        text(&self.name, 1, 100)?;
        if let Some(region) = &self.region
            && (region.len() != 2 || !region.bytes().all(|b| b.is_ascii_uppercase()))
        {
            return Err(AppError::bad("发行地区必须是两位大写地区代码"));
        }
        self.effective_timezone("UTC")?;
        text(&self.issuer, 0, 100)?;
        text(&self.network, 0, 40)?;
        text(&self.notes, 0, 4000)?;
        if !(self.last4.is_empty()
            || self.last4.len() == 4 && self.last4.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(AppError::bad(
                "Last4 must contain exactly four digits or be empty",
            ));
        }
        if self.color.len() != 7
            || !self.color.starts_with('#')
            || !self.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(AppError::bad("Color must be #rrggbb"));
        }
        if self.currency.len() != 3 || !self.currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(AppError::bad(
                "Currency must be a three-letter uppercase code",
            ));
        }
        if !(1..=31).contains(&self.statement_day) || !(1..=31).contains(&self.due_day) {
            return Err(AppError::bad("Days must be in 1..31"));
        }
        if !["fixed_day", "days_after_statement"].contains(&self.due_mode.as_str())
            || !(0..=2).contains(&self.due_month_offset)
        {
            return Err(AppError::bad("Invalid due-date mode or month offset"));
        }
        if self.due_mode == "days_after_statement" && !(1..=90).contains(&self.due_offset_days) {
            return Err(AppError::bad("Days after statement must be in 1..90"));
        }
        if !["none", "previous_business_day", "next_business_day"]
            .contains(&self.weekend_adjustment.as_str())
        {
            return Err(AppError::bad("Invalid weekend adjustment"));
        }
        match (self.annual_fee_month, self.annual_fee_day) {
            (None, None) => (),
            (Some(m), Some(d)) if (1..=12).contains(&m) && (1..=31).contains(&d) => (),
            _ => {
                return Err(AppError::bad(
                    "Annual fee month and day must both be specified",
                ));
            }
        }
        money(self.annual_fee_amount.as_deref())?;
        // Validate a leap-year and a non-leap-year cycle set, including month clamp.
        for year in [2027, 2028] {
            for month in 1..=12 {
                crate::dates::cycle_dates(
                    self,
                    NaiveDate::from_ymd_opt(year, month, 1).expect("valid month"),
                )?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct CardView {
    pub id: Uuid,
    pub data: CardInput,
    pub active: bool,
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MilestoneInput {
    pub card_id: Uuid,
    pub title: String,
    pub kind: String,
    pub recurrence: String,
    pub start_date: NaiveDate,
    #[serde(default)]
    pub months: Vec<u32>,
    #[serde(default)]
    pub dates: Vec<NaiveDate>,
}
impl MilestoneInput {
    pub fn validate(&self) -> Result<()> {
        text(&self.title, 1, 120)?;
        if !["annual_fee", "benefit", "custom"].contains(&self.kind.as_str())
            || ![
                "one_time",
                "monthly",
                "yearly",
                "quarterly",
                "semiannual",
                "custom_months",
                "custom_dates",
            ]
            .contains(&self.recurrence.as_str())
        {
            return Err(AppError::bad("Invalid milestone type or recurrence"));
        }
        date_range(self.start_date)?;
        let unique_months: std::collections::HashSet<_> = self.months.iter().collect();
        let unique_dates: std::collections::HashSet<_> = self.dates.iter().collect();
        if self.recurrence == "custom_months" {
            if self.months.is_empty()
                || self.months.len() > 12
                || unique_months.len() != self.months.len()
                || self.months.iter().any(|m| !(1..=12).contains(m))
            {
                return Err(AppError::bad("Select unique months between 1 and 12"));
            }
        } else if !self.months.is_empty() {
            return Err(AppError::bad("Months only apply to custom_months"));
        }
        if self.recurrence == "custom_dates" {
            if self.dates.is_empty()
                || self.dates.len() > 100
                || unique_dates.len() != self.dates.len()
            {
                return Err(AppError::bad("Provide 1 to 100 unique dates"));
            }
            for date in &self.dates {
                date_range(*date)?;
                if *date < self.start_date {
                    return Err(AppError::bad("Custom dates cannot precede the start date"));
                }
            }
        } else if !self.dates.is_empty() {
            return Err(AppError::bad("Dates only apply to custom_dates"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CyclePatch {
    pub statement_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub amount: Option<String>,
    pub minimum_payment: Option<String>,
    #[serde(default)]
    pub clear_amount: bool,
    #[serde(default)]
    pub clear_minimum_payment: bool,
    pub note: Option<String>,
    #[serde(default)]
    pub reset_dates: bool,
}
#[derive(Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct CycleView {
    pub id: Uuid,
    pub card_id: Uuid,
    pub cycle_month: NaiveDate,
    pub statement_date: NaiveDate,
    pub due_date: NaiveDate,
    pub statement_overridden: bool,
    pub due_overridden: bool,
    pub amount: Option<String>,
    pub minimum_payment: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
    pub note: String,
    pub revision: i32,
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FeedInput {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "privacy")]
    pub privacy: String,
    #[serde(default = "kinds")]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub card_ids: Vec<Uuid>,
    #[serde(default)]
    pub hide_paid: bool,
    #[serde(default)]
    pub alarms_days_before: Vec<i32>,
}
impl FeedInput {
    pub fn validate(&self) -> Result<()> {
        text(&self.name, 1, 100)?;
        if !["private", "normal", "detailed"].contains(&self.privacy.as_str())
            || self.card_ids.len() > 200
            || self.kinds.is_empty()
            || self.kinds.iter().any(|k| !kinds().contains(k))
        {
            return Err(AppError::bad("Invalid feed settings"));
        }
        if self.alarms_days_before.len() > 5
            || self
                .alarms_days_before
                .iter()
                .any(|v| !(0..=30).contains(v))
        {
            return Err(AppError::bad(
                "Calendar alarms must be between 0 and 30 days before",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateInput {
    pub name: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub sound: String,
    #[serde(default)]
    pub level: String,
}
impl Default for TemplateInput {
    fn default() -> Self {
        Self {
        name: "默认还款提醒".into(), title: "💳 {{ card.name }} · {{ event.label }}".into(),
        body: "日期：{{ event.date }}\n距离到期：{{ event.days_until }} 天{% if cycle.amount %}\n金额：{{ cycle.amount }} {{ card.currency }}{% endif %}".into(),
        html: "<h2>{{ card.name }}</h2><p>{{ event.label }}：<strong>{{ event.date }}</strong></p><p>请以银行实际账单为准。</p>".into(),
        url: "{{ app.url }}/cards/{{ card.id }}".into(), group: "CardDue".into(), sound: "bell".into(), level: "active".into(),
    }
    }
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleInput {
    pub name: String,
    pub event_kind: String,
    #[serde(default)]
    pub card_ids: Vec<Uuid>,
    pub offsets: Vec<i32>,
    #[serde(default = "time")]
    pub local_time: String,
    pub connection_ids: Vec<Uuid>,
    pub template_id: Uuid,
    #[serde(default = "yes")]
    pub enabled: bool,
}
impl RuleInput {
    pub fn validate(&self) -> Result<()> {
        text(&self.name, 1, 100)?;
        if !kinds().contains(&self.event_kind)
            || self.card_ids.len() > 200
            || self.offsets.is_empty()
            || self.offsets.len() > 12
            || self.offsets.iter().any(|o| !(-30..=365).contains(o))
            || self.connection_ids.is_empty()
            || self.connection_ids.len() > 20
        {
            return Err(AppError::bad(
                "Invalid reminder rule; positive offset means days before, negative means after",
            ));
        }
        NaiveTime::parse_from_str(&self.local_time, "%H:%M")
            .map_err(|_| AppError::bad("Reminder time must be HH:MM"))?;
        if self.local_time.len() != 5 {
            return Err(AppError::bad("Reminder time must be HH:MM"));
        }
        let unique: std::collections::HashSet<_> = self.offsets.iter().collect();
        if unique.len() != self.offsets.len() {
            return Err(AppError::bad("Offsets must be unique"));
        }
        let unique: std::collections::HashSet<_> = self.connection_ids.iter().collect();
        if unique.len() != self.connection_ids.len() {
            return Err(AppError::bad("Connections must be unique"));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectionInput {
    pub name: String,
    pub kind: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Omit on update to preserve encrypted credentials. Credentials are never returned.
    pub config: Option<serde_json::Value>,
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct ConnectionView {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Event {
    pub id: Uuid,
    pub user_id: Uuid,
    pub card_id: Uuid,
    pub cycle_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub event_date: NaiveDate,
    pub paid: bool,
    pub active: bool,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}
pub fn text(s: &str, min: usize, max: usize) -> Result<()> {
    if s.trim().chars().count() < min || s.len() > max || s.contains('\0') {
        return Err(AppError::bad(format!(
            "Text must contain {min}..{max} bytes and no NUL"
        )));
    }
    Ok(())
}
pub fn money(s: Option<&str>) -> Result<()> {
    if let Some(s) = s {
        let re = regex::Regex::new(r"^\d{1,12}(\.\d{1,2})?$").expect("constant regex");
        if !re.is_match(s) {
            return Err(AppError::bad(
                "Amount must be a non-negative decimal with at most two decimal places",
            ));
        }
    }
    Ok(())
}
pub fn date_range(d: NaiveDate) -> Result<()> {
    use chrono::Datelike;
    if !(2000..=2200).contains(&d.year()) {
        return Err(AppError::bad("Date must be in years 2000..2200"));
    }
    Ok(())
}
