//! Read-only timetable for the selected source, independent of scheduler coverage.

use std::collections::BTreeMap;
use std::fs;
use std::ops::Bound::Included;
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::model::{self, Calendar, HijriDate, Prayer};
use crate::{locations, online};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CalendarBasis {
    Gregorian,
    Hijri,
}

pub(crate) enum InstalledSource<'a> {
    Bundled { root: &'a Path, location: &'a str },
    Downloaded { root: &'a Path, slug: &'a str },
}

impl InstalledSource<'_> {
    fn files(&self) -> std::io::Result<Vec<PathBuf>> {
        let (directory, prefix, suffix) = match self {
            Self::Bundled { root, location } => (root.join(location), "prayer-times-".into(), format!("-{location}.csv")),
            Self::Downloaded { root, slug } => (root.join(slug), format!("{slug}_"), ".csv".into()),
        };
        let mut files = Vec::new();
        for entry in fs::read_dir(directory)? {
            let name = entry?.file_name();
            let Some(digits) = name.to_str().and_then(|name| name.strip_prefix(&prefix)?.strip_suffix(&suffix)) else { continue };
            if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let year = digits.parse().unwrap();
            let path = match self {
                Self::Bundled { root, location } => locations::calendar_file(root, location, year),
                Self::Downloaded { root, slug } => online::calendar_file(root, slug, year),
            };
            if path.file_name() == Some(name.as_os_str()) {
                files.push((year, path));
            }
        }
        files.sort_unstable_by_key(|(year, _)| *year);
        Ok(files.into_iter().map(|(_, path)| path).collect())
    }
}

struct Coverage {
    first: NaiveDate,
    last: NaiveDate,
    paired: Option<(HijriDate, HijriDate)>,
}

type Periods = BTreeMap<i32, [Option<Coverage>; 12]>;

fn cover(periods: &mut Periods, year: i32, month: u8, date: NaiveDate, paired: Option<HijriDate>) {
    let months = periods.entry(year).or_default();
    let range = months[usize::from(month - 1)].get_or_insert(Coverage { first: date, last: date, paired: None });
    range.last = date;
    if let Some(hijri) = paired {
        range.paired.get_or_insert((hijri, hijri)).1 = hijri;
    }
}

pub(crate) struct Snapshot {
    calendar: Calendar,
    gregorian: Periods,
    hijri: Periods,
    today: NaiveDate,
    default_hijri: Option<HijriDate>,
    warning: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OtherRange {
    other_start: Option<String>,
    other_end: Option<String>,
}

#[derive(Serialize)]
struct Year {
    year: i32,
    months: [Option<OtherRange>; 12],
}

fn years(periods: &Periods, other_basis: CalendarBasis) -> Vec<Year> {
    periods.iter().map(|(&year, months)| Year {
        year,
        months: std::array::from_fn(|index| months[index].as_ref().map(|range| {
            let (other_start, other_end) = match other_basis {
                CalendarBasis::Gregorian => (Some(model::date_key(range.first)), Some(model::date_key(range.last))),
                CalendarBasis::Hijri => range.paired.map_or((None, None), |(first, last)| (Some(first.iso()), Some(last.iso()))),
            };
            OtherRange { other_start, other_end }
        })),
    }).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    today_gregorian: String,
    default_hijri: Option<String>,
    warning: Option<&'static str>,
    gregorian_years: Vec<Year>,
    hijri_years: Vec<Year>,
}

#[derive(Serialize)]
struct Month<'a> {
    rows: Vec<Day<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Day<'a> {
    gregorian_date: &'a str,
    hijri_date: Option<String>,
    times: [Option<String>; Prayer::ALL.len()],
}

fn invalid_calendar(error: impl std::fmt::Display) -> &'static str {
    eprintln!("Dulook calendar browser: {error}");
    "invalidCalendar"
}

impl Snapshot {
    pub(crate) fn custom(path: &Path, now: i64) -> Result<Self, &'static str> {
        let bytes = fs::read(path).map_err(|_| "fileUnavailable")?;
        let calendar = model::parse_calendar(&String::from_utf8_lossy(&bytes), None)
            .map_err(|error| invalid_calendar(format_args!("{}: {error}", path.display())))?;
        Ok(Self::index(calendar, now, None))
    }

    pub(crate) fn installed(source: InstalledSource<'_>, timezone: Option<Tz>, now: i64) -> Result<Self, &'static str> {
        let files = source.files().map_err(invalid_calendar)?;
        if files.is_empty() {
            return Err("calendarBrowserNoRows");
        }
        let mut warning = None;
        let calendars = files.iter().filter_map(|path| {
            let result = fs::read(path).map_err(|error| error.to_string()).and_then(|bytes| {
                model::parse_calendar(&String::from_utf8_lossy(&bytes), timezone)
            });
            match result {
                Ok(calendar) => Some(calendar),
                Err(error) => {
                    invalid_calendar(format_args!("{}: {error}", path.display()));
                    warning = Some("calendarBrowserSomeFilesUnavailable");
                    None
                }
            }
        });
        let calendar = model::merge_calendars(calendars).map_err(invalid_calendar)?;
        if calendar.dates.is_empty() {
            return Err("invalidCalendar");
        }
        Ok(Self::index(calendar, now, warning))
    }

    fn index(calendar: Calendar, now: i64, warning: Option<&'static str>) -> Self {
        let today = model::calendar_date_at(now, calendar.source_timezone());
        let mut gregorian = Periods::new();
        let mut hijri = Periods::new();
        let mut before_today = None;
        let mut from_today = None;
        for key in calendar.dates.keys() {
            let date = model::date_from_key(key).expect("calendar keys are validated dates");
            let paired = calendar.hijri_date(date);
            cover(&mut gregorian, date.year(), date.month() as u8, date, paired);
            if let Some(paired) = paired {
                cover(&mut hijri, paired.year, paired.month, date, None);
                if date < today {
                    before_today = Some(paired);
                } else {
                    from_today.get_or_insert(paired);
                }
            }
        }
        Self { calendar, gregorian, hijri, today, default_hijri: from_today.or(before_today), warning }
    }

    pub(crate) fn catalog(&self) -> impl Serialize {
        Catalog {
            today_gregorian: model::date_key(self.today),
            default_hijri: self.default_hijri.map(HijriDate::iso),
            warning: self.warning,
            gregorian_years: years(&self.gregorian, CalendarBasis::Hijri),
            hijri_years: years(&self.hijri, CalendarBasis::Gregorian),
        }
    }

    pub(crate) fn month(&self, basis: CalendarBasis, year: i32, month: u8) -> impl Serialize + '_ {
        let periods = match basis {
            CalendarBasis::Gregorian => &self.gregorian,
            CalendarBasis::Hijri => &self.hijri,
        };
        let coverage = month.checked_sub(1).and_then(|index| periods.get(&year)?.get(usize::from(index))?.as_ref());
        let Some(range) = coverage else { return Month { rows: Vec::new() } };
        let first = model::date_key(range.first);
        let last = model::date_key(range.last);
        let rows = self.calendar.dates.range::<str, _>((Included(first.as_str()), Included(last.as_str())))
            .filter_map(|(key, _)| {
                let paired = self.calendar.hijri_date(model::date_from_key(key)?);
                if matches!(basis, CalendarBasis::Hijri) && !paired.is_some_and(|date| (date.year, date.month) == (year, month)) {
                    return None;
                }
                Some(Day { gregorian_date: key, hijri_date: paired.map(HijriDate::iso), times: model::day_time_cells(&self.calendar, key) })
            }).collect();
        Month { rows }
    }
}
