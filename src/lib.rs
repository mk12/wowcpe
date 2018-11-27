// Copyright 2017 Mitchell Kember. Subject to the MIT License.

//! _What's On WCPE?_
//!
//! This crate provides a single function [`lookup`] to find out what is playing
//! on the [classical radio station WCPE](https://theclassicalstation.org). It
//! returns a [`Response`], which contains the title, composer, and other
//! information about the piece. The WCPE website only exposes data for the
//! past week, so [`Request`] times must be in that range.
//!
//! [`lookup`]: fn.lookup.html
//! [`Response`]: struct.Response.html
//! [`Request`]: struct.Request.html

#[macro_use]
extern crate quick_error;

extern crate chrono;
extern crate chrono_tz;
extern crate curl;
extern crate encoding;
extern crate marksman_escape;
extern crate table_extract;

use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use chrono_tz::US::Eastern;
use curl::easy::Easy;
use encoding::all::WINDOWS_1252;
use encoding::{Encoding, DecoderTrap};
use marksman_escape::Unescape;
use std::result;
use table_extract::Table;

/// Request to look up what is playing on WCPE.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    /// The moment in time to look up.
    pub time: DateTime<Local>,
}

/// Information about a piece playing on WCPE.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response {
    /// Name of the current program, e.g., "Sleepers, Awake!"
    pub program: String,
    /// Time the piece started playing.
    pub start_time: DateTime<Local>,
    /// Time the piece stopped (or will stop) playing.
    pub end_time: DateTime<Local>,
    /// Composer of the piece.
    pub composer: String,
    /// Title of the piece.
    pub title: String,
    /// Perfomers in the recording of the piece.
    pub performers: String,
    /// Record label of the recording of the piece.
    pub record_label: String,
}

quick_error!{
    /// An error that occurs while processing a request.
    #[derive(Debug)]
    pub enum Error {
        Curl(err: curl::Error) {
            cause(err)
            description(err.description())
            from()
        }
        Unavailable {
            description("Data for the given time is not available")
        }
        HtmlParse {
            description("Failed to parse the HTML document")
        }
        RowNotFound {
            description("Failed to find the current table row")
        }
        TimeParse {
            description("Failed to parse the time")
            from(std::num::ParseIntError)
        }
    }
}

/// A specialized `Result` type for the `wowcpe` crate.
pub type Result<T> = result::Result<T, Error>;

/// Looks up what is playing on WCPE based on `request`.
///
/// Returns an error if `request.time` is not within the past week, since WCPE
/// only provides data for that time frame.
///
/// This will download a page from `https://theclassicalstation.org` using
/// `curl`, so it requires network access. Returns an error if `curl` fails or
/// if extracting the desired information from the HTML fails.
pub fn lookup(request: &Request) -> Result<Response> {
    validate_request(request)?;
    let html = download(&get_url(request.time))?;
    lookup_in_html(request, &html)
}

fn validate_request(request: &Request) -> Result<()> {
    let t = request.time;
    let end_of_day = eastern_eod(Local::now());
    if t <= end_of_day - Duration::weeks(1) || t > end_of_day {
        Err(Error::Unavailable)
    } else {
        Ok(())
    }
}

const WEEKDAYS: [&'static str; 7] =
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn get_url(time: DateTime<Local>) -> String {
    let index = time.with_timezone(&Eastern)
        .weekday()
        .num_days_from_monday() as usize;
    let day = WEEKDAYS[index];
    format!("https://theclassicalstation.org/playing_{}.shtml", day)
}

// NOTE: theclassicalstation.org uses Windows-1252 encoding.
fn download(url: &str) -> Result<String> {
    let mut body = String::new();
    let mut handle = Easy::new();
    handle.url(url)?;
    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            WINDOWS_1252
                .decode_to(data, DecoderTrap::Ignore, &mut body)
                .unwrap();
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    Ok(body)
}

fn lookup_in_html(request: &Request, html: &str) -> Result<Response> {
    let time_header = header("Start Time");
    let program_header = header("Program");
    let table = Table::find_by_headers(html, &[&program_header, &time_header])
        .ok_or(Error::HtmlParse)?;

    let mut end_time = None;
    let mut program = None;
    let mut previous = None;
    for row in &table {
        let time = row.get(&time_header).ok_or(Error::HtmlParse)?;
        let time = parse_eastern_time(request.time, time)?;
        if time > request.time {
            end_time = Some(time);
            break;
        }

        program = row.get(&program_header)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or(program);
        previous = Some((row, time))
    }

    let (row, start_time) = previous.ok_or(Error::RowNotFound)?;
    let end_time = end_time.unwrap_or_else(|| eastern_eod(request.time));
    let program = parse_field(program);
    let extract = |name| parse_field(row.get(&header(name)));

    Ok(Response {
        program,
        start_time,
        end_time,
        composer: extract("Composer"),
        title: extract("Title"),
        performers: extract("Performers"),
        record_label: extract("Record Label"),
    })
}

fn header(name: &str) -> String {
    format!("<p>{}</p>", name)
}

fn parse_eastern_time(
    base: DateTime<Local>,
    input: &str,
) -> Result<DateTime<Local>> {
    let input = input.trim();
    let index = input.find(':').ok_or(Error::TimeParse)?;
    let (hh, colon_mm) = input.split_at(index);
    let mm = &colon_mm[1..];
    let hour = hh.parse::<u32>()?;
    let minute = mm.parse::<u32>()?;

    base.with_timezone(&Eastern)
        .with_hour(hour)
        .and_then(|t| t.with_minute(minute))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .map(|t| t.with_timezone(&Local))
        .ok_or(Error::TimeParse)
}

fn eastern_eod(base: DateTime<Local>) -> DateTime<Local> {
    base.with_timezone(&Eastern)
        .with_hour(23)
        .and_then(|t| t.with_minute(59))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap()
        .with_timezone(&Local)
}

const MISSING: &'static str = "<missing>";

fn parse_field(html: Option<&str>) -> String {
    if let Some(html) = html {
        let bytes = html.trim().bytes();
        String::from_utf8(Unescape::new(bytes).collect()).unwrap()
    } else {
        MISSING.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::TimeZone;
    use chrono_tz::US::Pacific;

    #[test]
    fn test_validate_request_err() {
        let time = eastern_eod(Local::now()) + Duration::seconds(1);
        assert!(validate_request(&Request { time }).is_err());

        let time = eastern_eod(Local::now()) - Duration::weeks(1);
        assert!(validate_request(&Request { time }).is_err());
    }

    #[test]
    fn test_validate_request_ok() {
        let time = Local::now();
        assert!(validate_request(&Request { time }).is_ok());

        let time = eastern_eod(Local::now());
        assert!(validate_request(&Request { time }).is_ok());

        let time = eastern_eod(Local::now()) - Duration::weeks(1) +
            Duration::minutes(1);
        assert!(validate_request(&Request { time }).is_ok());
    }

    #[test]
    fn test_get_url_eastern() {
        let monday = Eastern
            .ymd(2017, 7, 3)
            .and_hms(0, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/playing_mon.shtml",
            get_url(monday)
        );

        let friday = Eastern
            .ymd(2017, 7, 7)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/playing_fri.shtml",
            get_url(friday)
        );
    }

    #[test]
    fn test_get_url_pacific() {
        let monday = Pacific
            .ymd(2017, 7, 3)
            .and_hms(0, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/playing_mon.shtml",
            get_url(monday)
        );

        let friday = Pacific
            .ymd(2017, 7, 7)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/playing_sat.shtml",
            get_url(friday)
        );
    }

    #[test]
    fn test_parse_eastern_time_err() {
        let now = Local::now();

        assert!(parse_eastern_time(now, "").is_err());
        assert!(parse_eastern_time(now, "00").is_err());
        assert!(parse_eastern_time(now, "-1").is_err());
        assert!(parse_eastern_time(now, "24:00").is_err());
        assert!(parse_eastern_time(now, "A:B").is_err());
    }

    #[test]
    fn test_parse_eastern_time_ok() {
        let now = Local::now();

        assert!(parse_eastern_time(now, "00:00").is_ok());
        assert!(parse_eastern_time(now, "12:00").is_ok());
        assert!(parse_eastern_time(now, "23:59").is_ok());
        assert!(parse_eastern_time(now, " 1:34 ").is_ok());
    }

    #[test]
    fn test_parse_eastern_time_eastern() {
        let base = Eastern
            .ymd(2017, 7, 10)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);

        assert_eq!(
            Eastern
                .ymd(2017, 7, 10)
                .and_hms(12, 0, 0)
                .with_timezone(&Local),
            parse_eastern_time(base, "12:00").unwrap()
        );
    }

    #[test]
    fn test_parse_eastern_time_pacific() {
        let base = Pacific
            .ymd(2017, 7, 10)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);

        assert_eq!(
            Eastern
                .ymd(2017, 7, 11)
                .and_hms(12, 0, 0)
                .with_timezone(&Local),
            parse_eastern_time(base, "12:00").unwrap()
        );
    }

    #[test]
    fn test_eastern_eod() {
        let base = Local::now();
        assert_eq!(
            parse_eastern_time(base, "23:59").unwrap(),
            eastern_eod(base)
        );

        let base = Pacific
            .ymd(2017, 7, 10)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            parse_eastern_time(base, "23:59").unwrap(),
            eastern_eod(base)
        );
    }

    #[test]
    fn parse_field_none() {
        assert_eq!(MISSING, parse_field(None));
    }

    #[test]
    fn parse_field_some() {
        assert_eq!("Something", parse_field(Some(" Something ")));
        assert_eq!("this & that", parse_field(Some("this &amp; that ")));
        assert_eq!("'Twas so", parse_field(Some("&apos;Twas so")));
        assert_eq!("what &a;", parse_field(Some("what &a;")));
    }

    #[test]
    fn test_lookup_in_html_parse_err() {
        let request = Request { time: Local::now() };

        assert!(lookup_in_html(&request, "").is_err());
        assert!(lookup_in_html(&request, "<table></table>").is_err());
        assert!(lookup_in_html(&request, "<table><tr></tr></table>").is_err());
    }

    const HTML: &'static str = r#"
<table>
<tr>
<th>
<p>Program
</th><th>
<p>Start Time
</th><th>
<p>Composer
</th><th>
<p>Title
</th><th>
<p>Performers
</th><th>
<p>Record Label
</th></tr>
<tr>
<td>Sleepers, Awake!</td>
<td>00:01</td>
<td>Liszt</td>
<td>Tasso: Lament &amp; Trimuph (Symphonic Poem No. 2)</td>
<td>Gewandhaus Orchestra/Masur</td>
<td>Naxos</td></tr>
<tr>
<td></td>
<td>00:27</td>
<td>Handel</td>
<td>Concerto Grosso in D, Op. 3 No. 6</td>
<td>Concentus Musicus of Vienna/Harnoncourt</td>
<td>MHS</td></tr>
</table>
"#;

    #[test]
    fn test_lookup_in_html_too_early() {
        let time = parse_eastern_time(Local::now(), "00:00").unwrap();
        assert!(lookup_in_html(&Request { time }, HTML).is_err());
    }

    #[test]
    fn test_lookup_in_html_first() {
        let now = Local::now();
        let expected = Response {
            program: "Sleepers, Awake!".to_string(),
            start_time: parse_eastern_time(now, "00:01").unwrap(),
            end_time: parse_eastern_time(now, "00:27").unwrap(),
            composer: "Liszt".to_string(),
            title: "Tasso: Lament & Trimuph (Symphonic Poem No. 2)".to_string(),
            performers: "Gewandhaus Orchestra/Masur".to_string(),
            record_label: "Naxos".to_string(),
        };

        let time = parse_eastern_time(now, "00:01").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(now, "00:02").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(now, "00:26").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());
    }

    #[test]
    fn test_lookup_in_html_last() {
        let now = Local::now();
        let expected = Response {
            program: "Sleepers, Awake!".to_string(),
            start_time: parse_eastern_time(now, "00:27").unwrap(),
            end_time: parse_eastern_time(now, "23:59").unwrap(),
            composer: "Handel".to_string(),
            title: "Concerto Grosso in D, Op. 3 No. 6".to_string(),
            performers: "Concentus Musicus of Vienna/Harnoncourt".to_string(),
            record_label: "MHS".to_string(),
        };

        let time = parse_eastern_time(now, "00:27").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(now, "00:28").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(now, "23:59").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());
    }
}
