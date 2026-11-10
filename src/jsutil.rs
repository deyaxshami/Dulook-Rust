//! Helpers that reproduce the JavaScript semantics the original QML/JS code relied on.

use chrono::{Local, TimeZone};
use serde_json::Value;

/// JS `String.prototype.trim`: Unicode whitespace plus the BOM.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// JS `String(value)` for JSON values.
pub fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_owned(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                n.as_f64().map(|f| f.to_string()).unwrap_or_default()
            }
        }
        Value::Array(_) | Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
    }
}

/// JS `Number(value)` for JSON values.
pub fn js_number(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Null => 0.0,
        Value::String(s) => {
            let t = js_trim(s);
            if t.is_empty() { 0.0 } else { t.parse::<f64>().unwrap_or(f64::NAN) }
        }
        Value::Array(_) | Value::Object(_) => f64::NAN,
    }
}

/// JS `String.prototype.length` (UTF-16 code units).
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

pub fn pad2(n: i64) -> String {
    if n < 10 { format!("0{n}") } else { n.to_string() }
}

/// Maps ASCII digits to Arabic-Indic digits when `arabic` is set.
pub fn localized_digits(s: &str, arabic: bool) -> String {
    if !arabic {
        return s.to_owned();
    }
    s.chars()
        .map(|c| match c {
            '0'..='9' => char::from_u32(0x0660 + (c as u32 - '0' as u32)).unwrap_or(c),
            _ => c,
        })
        .collect()
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// JS `Date.prototype.getTimezoneOffset` for the instant `ms`.
pub fn timezone_offset_minutes(ms: i64) -> i32 {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| -(t.offset().local_minus_utc() / 60))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn js_conversions() {
        assert_eq!(js_trim("\u{feff} a \n"), "a");
        assert_eq!(js_string(&json!(7.0)), "7");
        assert_eq!(js_string(&json!(7.1)), "7.1");
        assert_eq!(js_string(&json!(12)), "12");
        assert!(js_number(&json!("abc")).is_nan());
        assert_eq!(js_number(&json!(" 42 ")), 42.0);
        assert_eq!(localized_digits("UTC+02:00", true), "UTC+٠٢:٠٠");
        assert_eq!(pad2(5), "05");
    }
}
