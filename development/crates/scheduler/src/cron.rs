//! Strict Unix five-field calendar schedules, shared by preview and dispatch.
use chrono::{DateTime, Datelike, Days, LocalResult, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Cron {
    pub expression: String,
    pub values: Vec<Vec<u32>>,
    wild: [bool; 5],
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub expression: String,
    pub description: String,
    pub times: Vec<String>,
    pub fields: Vec<Vec<u32>>,
}
impl Cron {
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.len() > 256 {
            return Err("Cron 表达式过长".into());
        }
        let fields: Vec<_> = input.split_whitespace().collect();
        if fields.len() != 5 {
            return Err("请使用 Unix 五字段：分钟 小时 日期 月份 星期，不包含秒或年份".into());
        }
        let limits = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 7)];
        let names = ["分钟", "小时", "日期", "月份", "星期"];
        let mut values = Vec::new();
        let mut wild = [false; 5];
        for (i, raw) in fields.iter().enumerate() {
            let (lo, hi) = limits[i];
            let mut set = std::collections::BTreeSet::new();
            let number = |s: &str| -> Result<u32, String> {
                if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(format!("{}仅支持数字、*、逗号、范围和步长", names[i]));
                }
                s.parse().map_err(|_| format!("{}数值无效", names[i]))
            };
            for token in raw.split(',') {
                let parts: Vec<_> = token.split('/').collect();
                if parts.len() > 2 {
                    return Err("Cron 步长格式无效".into());
                }
                let step = if parts.len() == 2 {
                    number(parts[1])?
                } else {
                    1
                };
                if step == 0 || step > hi - lo + 1 {
                    return Err(format!("{}步长超出范围", names[i]));
                }
                let (start, end) = if parts[0] == "*" {
                    wild[i] = true;
                    (lo, hi)
                } else {
                    let range: Vec<_> = parts[0].split('-').collect();
                    if range.len() > 2 {
                        return Err("Cron 范围格式无效".into());
                    }
                    let start = number(range[0])?;
                    let end = if range.len() == 2 {
                        number(range[1])?
                    } else if parts.len() == 2 {
                        hi
                    } else {
                        start
                    };
                    (start, end)
                };
                if start < lo || end > hi || start > end {
                    return Err(format!(
                        "{}范围应为 {}–{}，起点不能大于终点",
                        names[i], lo, hi
                    ));
                }
                for n in (start..=end).step_by(step as usize) {
                    set.insert(if i == 4 && n == 7 { 0 } else { n });
                }
            }
            values.push(set.into_iter().collect());
        }
        Ok(Self {
            expression: fields.join(" "),
            values,
            wild,
        })
    }
    fn day(&self, date: NaiveDate) -> bool {
        let dom = self.values[2].contains(&date.day());
        let dow = self.values[4].contains(&date.weekday().num_days_from_sunday());
        self.values[3].contains(&date.month())
            && if !self.wild[2] && !self.wild[4] {
                dom || dow
            } else {
                dom && dow
            }
    }
    fn first(zone: Tz, local: chrono::NaiveDateTime) -> Option<DateTime<Utc>> {
        match zone.from_local_datetime(&local) {
            LocalResult::Single(t) => Some(t.with_timezone(&Utc)),
            LocalResult::Ambiguous(a, b) => Some(a.min(b).with_timezone(&Utc)),
            LocalResult::None => None,
        }
    }
    pub fn matches(&self, at: DateTime<Utc>, zone: Tz) -> bool {
        let local = at.with_timezone(&zone);
        self.day(local.date_naive())
            && self.values[0].contains(&local.minute())
            && self.values[1].contains(&local.hour())
            && Self::first(zone, local.naive_local()).is_some_and(|first| first == at)
    }
    pub fn next(&self, zone: Tz, after: DateTime<Utc>, count: usize) -> Vec<DateTime<Utc>> {
        let mut date = after.with_timezone(&zone).date_naive();
        let mut out = Vec::new();
        // Gregorian calendar repeats within 400 years; leap-day ten-run previews need > 28 years.
        for _ in 0..146_097 {
            if self.day(date) {
                for hour in &self.values[1] {
                    for minute in &self.values[0] {
                        if let Some(at) =
                            Self::first(zone, date.and_hms_opt(*hour, *minute, 0).unwrap())
                        {
                            if at > after {
                                out.push(at);
                                if out.len() == count {
                                    return out;
                                }
                            }
                        }
                    }
                }
            }
            match date.checked_add_days(Days::new(1)) {
                Some(next) => date = next,
                None => break,
            }
        }
        out
    }
    pub fn description(&self) -> String {
        let p: Vec<_> = self.expression.split(' ').collect();
        let v = &self.values;
        let time = if v[0].len() == 1 && v[1].len() == 1 {
            Some(format!("{:02}:{:02}", v[1][0], v[0][0]))
        } else {
            None
        };
        if p[2..] == ["*", "*", "*"] {
            if p[0] == "*" && p[1] == "*" {
                return "每分钟执行一次".into();
            }
            if p[0].starts_with("*/") && p[1] == "*" {
                return format!("每小时内，每 {} 分钟执行", &p[0][2..]);
            }
            if v[0].len() == 1 && p[1] == "*" {
                return format!("每小时第 {} 分钟执行", v[0][0]);
            }
            if let Some(t) = time.as_ref() {
                return format!("每天 {t} 执行");
            }
        }
        if let Some(t) = time {
            if p[2] == "*" && p[3] == "*" {
                let names = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
                return format!(
                    "{} {t} 执行",
                    v[4].iter()
                        .map(|n| names[*n as usize])
                        .collect::<Vec<_>>()
                        .join("、")
                );
            }
            if p[3] == "*" && p[4] == "*" {
                return format!(
                    "每月 {} 日 {t} 执行（无对应日期则跳过）",
                    v[2].iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join("、")
                );
            }
        }
        format!(
            "分钟 {} · 小时 {} · 日期 {} · 月份 {} · 星期 {}{}",
            p[0],
            p[1],
            p[2],
            p[3],
            p[4],
            if !self.wild[2] && !self.wild[4] {
                "；日期与星期按「或」匹配"
            } else {
                ""
            }
        )
    }
}
pub fn preview(expression: &str, zone: &str, after: DateTime<Utc>) -> Result<Preview, String> {
    let cron = Cron::parse(expression)?;
    let tz = zone
        .parse::<Tz>()
        .map_err(|_| "无效 IANA 时区，例如 Asia/Shanghai".to_string())?;
    let times = cron.next(tz, after, 10);
    if times.is_empty() {
        return Err("此计划在未来 400 年内无触发时间，请检查日期与月份".into());
    }
    Ok(Preview {
        expression: cron.expression.clone(),
        description: cron.description(),
        times: times.iter().map(|t| t.to_rfc3339()).collect(),
        fields: cron.values,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }
    #[test]
    fn strict_fields() {
        for bad in [
            "0 0 9 * * ?",
            "*/0 * * * *",
            "60 * * * *",
            "* * * * MON",
            "* * 1-0 * *",
            "* * * * 8",
            "* * * * 1,,2",
        ] {
            assert!(Cron::parse(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn weekly_and_or() {
        let p = preview("0 9 * * 1-5", "Asia/Shanghai", at("2026-11-20T02:00:00Z")).unwrap();
        assert_eq!(p.times[0], "2026-11-23T01:00:00+00:00");
        let c = Cron::parse("0 9 1 * 1").unwrap();
        assert!(c.matches(at("2026-12-01T09:00:00Z"), chrono_tz::UTC));
        assert!(c.matches(at("2026-11-23T09:00:00Z"), chrono_tz::UTC));
    }
    #[test]
    fn calendar_and_dst() {
        let c = Cron::parse("30 2 * * *").unwrap();
        let n = c.next(chrono_tz::America::New_York, at("2026-03-08T00:00:00Z"), 1);
        assert_eq!(n[0], at("2026-03-09T06:30:00Z"));
        let c = Cron::parse("30 1 * * *").unwrap();
        assert!(c.matches(at("2026-11-01T05:30:00Z"), chrono_tz::America::New_York));
        assert!(!c.matches(at("2026-11-01T06:30:00Z"), chrono_tz::America::New_York));
        assert!(Cron::parse("0 0 31 2 *")
            .unwrap()
            .next(chrono_tz::UTC, Utc::now(), 1)
            .is_empty());
        assert_eq!(
            preview("0 0 29 2 *", "UTC", Utc::now())
                .unwrap()
                .times
                .len(),
            10
        );
    }
}
