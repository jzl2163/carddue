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

#[derive(Debug)]
pub struct InterestEstimate {
    pub statement: NaiveDate,
    pub due: NaiveDate,
    pub days: i64,
    pub longest: i64,
}

/// Estimate a new purchase posted today. Statement-day purchases are assumed to
/// enter that day's bill. Calendar-day differences exclude the purchase day.
pub fn interest_estimate(
    cycles: &[crate::model::CycleView],
    today: NaiveDate,
) -> Option<InterestEstimate> {
    let mut ordered: Vec<_> = cycles.iter().collect();
    ordered.sort_by_key(|c| (c.statement_date, c.cycle_month));
    let next = ordered.iter().find(|c| c.statement_date >= today)?;
    let longest = ordered
        .windows(2)
        .filter(|w| {
            w[1].statement_date >= today && w[1].statement_date <= today + Duration::days(366)
        })
        .map(|w| (w[1].due_date - (w[0].statement_date + Duration::days(1))).num_days())
        .max()
        .unwrap_or((next.due_date - today).num_days());
    Some(InterestEstimate {
        statement: next.statement_date,
        due: next.due_date,
        days: (next.due_date - today).num_days(),
        longest,
    })
}

#[cfg(test)]
mod interest_tests {
    use super::*;
    fn cycle(statement: &str, due: &str) -> crate::model::CycleView {
        let statement: NaiveDate = statement.parse().unwrap();
        crate::model::CycleView {
            id: uuid::Uuid::new_v4(),
            card_id: uuid::Uuid::new_v4(),
            cycle_month: month(statement, 0),
            statement_date: statement,
            due_date: due.parse().unwrap(),
            statement_overridden: false,
            due_overridden: false,
            amount: None,
            minimum_payment: None,
            paid_at: None,
            note: String::new(),
            revision: 0,
        }
    }
    #[test]
    fn ranking_cross_month_leap_year_and_statement_day() {
        let cycles = vec![
            cycle("2028-01-31", "2028-02-20"),
            cycle("2028-02-29", "2028-03-20"),
            cycle("2028-03-31", "2028-04-20"),
        ];
        let a = interest_estimate(&cycles, day(2028, 2, 1)).unwrap();
        assert_eq!(a.days, 48);
        assert_eq!(a.longest, 50);
        assert_eq!(
            interest_estimate(&cycles, day(2028, 2, 29)).unwrap().days,
            20
        );
        assert_eq!(
            interest_estimate(&cycles, day(2028, 3, 1)).unwrap().days,
            50
        );
    }
    #[test]
    fn card_local_date_across_international_date_line() {
        let now = DateTime::parse_from_rfc3339("2028-03-01T01:00:00Z").unwrap();
        let cycles = vec![
            cycle("2028-02-29", "2028-03-20"),
            cycle("2028-03-31", "2028-04-20"),
        ];
        let ny = now
            .with_timezone(&chrono_tz::America::New_York)
            .date_naive();
        let sh = now.with_timezone(&chrono_tz::Asia::Shanghai).date_naive();
        assert_eq!(interest_estimate(&cycles, ny).unwrap().days, 20);
        assert_eq!(interest_estimate(&cycles, sh).unwrap().days, 50);
    }
}
