use crate::{
    error::{AppError, Result},
    model::CardInput,
};
use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc, Weekday,
};
use chrono_tz::Tz;

pub fn month(date: NaiveDate, offset: i32) -> NaiveDate {
    let n = date.year() * 12 + date.month0() as i32 + offset;
    NaiveDate::from_ymd_opt(n.div_euclid(12), n.rem_euclid(12) as u32 + 1, 1)
        .expect("bounded supported year")
}
pub fn day(year: i32, month: u32, wanted: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("validated month");
    let last = self::month(first, 1).pred_opt().expect("valid next month");
    first
        .with_day(wanted.min(last.day()))
        .expect("validated day")
}
pub fn adjusted(mut d: NaiveDate, mode: &str) -> NaiveDate {
    let step = if mode == "previous_business_day" {
        -1
    } else if mode == "next_business_day" {
        1
    } else {
        return d;
    };
    while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
        d += Duration::days(step);
    }
    d
}
pub fn due_for_statement(
    c: &CardInput,
    cycle_month: NaiveDate,
    statement: NaiveDate,
) -> Result<NaiveDate> {
    let due = if c.due_mode == "days_after_statement" {
        statement + Duration::days(c.due_offset_days)
    } else {
        let m = month(cycle_month, c.due_month_offset);
        day(
            m.year(),
            m.month(),
            if c.due_last_day { 31 } else { c.due_day },
        )
    };
    let due = adjusted(due, &c.weekend_adjustment);
    if due < statement {
        return Err(AppError::bad(
            "The due date cannot precede the statement date; review month offset and weekend adjustment",
        ));
    }
    Ok(due)
}
pub fn cycle_dates(c: &CardInput, m: NaiveDate) -> Result<(NaiveDate, NaiveDate)> {
    let statement = day(
        m.year(),
        m.month(),
        if c.statement_last_day {
            31
        } else {
            c.statement_day
        },
    );
    Ok((statement, due_for_statement(c, m, statement)?))
}
/// DST overlap: choose the earlier occurrence. Gap: move forward to the first valid minute.
pub fn local_instant(date: NaiveDate, time: NaiveTime, tz: Tz) -> Result<DateTime<Utc>> {
    let naive = date.and_time(time);
    for minutes in 0..=180 {
        match tz.from_local_datetime(&(naive + Duration::minutes(minutes))) {
            LocalResult::Single(v) => return Ok(v.with_timezone(&Utc)),
            LocalResult::Ambiguous(a, b) => return Ok(a.min(b).with_timezone(&Utc)),
            LocalResult::None => (),
        }
    }
    Err(AppError::bad(
        "The configured local time cannot be resolved in this timezone",
    ))
}
pub fn label(kind: &str) -> &'static str {
    match kind {
        "payment_due" => "还款日",
        "statement" => "账单日",
        "annual_fee" => "年费日",
        "benefit" => "权益重置",
        _ => "自定义事项",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clamp_and_rollover() {
        assert_eq!(day(2027, 2, 31).to_string(), "2027-02-28");
        assert_eq!(day(2028, 2, 31).to_string(), "2028-02-29");
        assert_eq!(month(day(2026, 12, 15), 1).to_string(), "2027-01-01");
        assert_eq!(month(day(2026, 1, 1), -1).to_string(), "2025-12-01");
        assert_eq!(
            adjusted(day(2026, 9, 6), "previous_business_day").to_string(),
            "2026-09-04"
        );
        assert_eq!(
            adjusted(day(2026, 9, 6), "next_business_day").to_string(),
            "2026-09-07"
        );
    }
    #[test]
    fn dst_gap_and_overlap() {
        let tz = chrono_tz::America::Los_Angeles;
        assert_eq!(
            local_instant(
                day(2026, 3, 8),
                NaiveTime::from_hms_opt(2, 30, 0).unwrap(),
                tz
            )
            .unwrap()
            .to_rfc3339(),
            "2026-03-08T10:00:00+00:00"
        );
        assert_eq!(
            local_instant(
                day(2026, 11, 1),
                NaiveTime::from_hms_opt(1, 30, 0).unwrap(),
                tz
            )
            .unwrap()
            .to_rfc3339(),
            "2026-11-01T08:30:00+00:00"
        );
    }
}
