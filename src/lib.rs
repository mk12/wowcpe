// Copyright 2017 Mitchell Kember. Subject to the MIT License.

//! _What's On WCPE?_
//!
//! This crate provides a single function [`lookup`] to find out what is playing
//! on the [classical radio station WCPE](https://theclassicalstation.org). It
//! returns a [`Response`], which contains the title, composer, and other
//! information about the piece.
//!
//! [`lookup`]: fn.lookup.html
//! [`Response`]: struct.Response.html

#[macro_use]
extern crate quick_error;

extern crate chrono;
extern crate chrono_tz;
extern crate curl;
extern crate option_filter;
extern crate table_extract;

use chrono::{DateTime, Datelike, Local, Timelike, Utc};
use chrono_tz::US::Eastern;
use curl::easy::Easy;
use option_filter::OptionFilterExt;
use std::result;
use table_extract::Table;

/// Request to look up what is playing on WCPE.
pub struct Request {
    /// The moment in time to look up.
    pub time: DateTime<Local>,
}

/// Information about a piece playing on WCPE.
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
/// This will download a page from `https://theclassicalstation.org` using
/// `curl`, so it requires network access. Returns an error if `curl` fails or
/// if extracting the desired information from the HTML fails.
pub fn lookup(request: &Request) -> Result<Response> {
    let html = download(&get_url(request.time))?;
    lookup_in_html(request, &html)
}

const WEEKDAYS: [&'static str; 7] =
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn get_url(time: DateTime<Local>) -> String {
    let index = time.weekday().num_days_from_monday() as usize;
    let day = WEEKDAYS[index];
    format!("http://theclassicalstation.org/playing_{}.shtml", day)
}

const INVALID_UTF8: &'static str = "<!-- invalid utf-8 -->";

fn download(url: &str) -> Result<String> {
    let mut body = String::new();
    let mut handle = Easy::new();
    handle.url(url)?;
    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            body.push_str(&String::from_utf8(data.to_vec())
                .unwrap_or_else(|_| INVALID_UTF8.to_string()));
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    Ok(body)
}

const MISSING: &'static str = "<missing>";

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
        let time = parse_eastern_time(time)?;
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
    let end_time = end_time.unwrap_or_else(eastern_eod);
    let program = program.unwrap_or(MISSING).to_string();
    let extract =
        |name| row.get(&header(name)).unwrap_or(MISSING).trim().to_string();

    Ok(Response {
        program,
        start_time,
        end_time,
        composer: extract("Composer"),
        title: extract("Title"),
        performers: extract("Perfomers"),
    })
}

fn header(name: &str) -> String {
    format!("<p>{}\n</p>", name)
}

fn parse_eastern_time(input: &str) -> Result<DateTime<Local>> {
    let input = input.trim();
    let index = input.find(':').ok_or(Error::TimeParse)?;
    let (hh, colon_mm) = input.split_at(index);
    let mm = &colon_mm[1..];
    let hour = hh.parse::<u32>()?;
    let minute = mm.parse::<u32>()?;

    Utc::now()
        .with_timezone(&Eastern)
        .with_hour(hour)
        .and_then(|t| t.with_minute(minute))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .map(|t| t.with_timezone(&Local))
        .ok_or(Error::TimeParse)
}

fn eastern_eod() -> DateTime<Local> {
    Utc::now()
        .with_timezone(&Eastern)
        .with_hour(23)
        .and_then(|t| t.with_minute(59))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap()
        .with_timezone(&Local)
}

#[cfg(test)]
mod test {
    use super::*;

    use chrono::TimeZone;

    #[test]
    fn test_get_url() {
        let monday = Local.ymd(2017, 7, 3).and_hms(0, 0, 0);
        assert_eq!(
            "http://theclassicalstation.org/playing_mon.shtml",
            get_url(monday)
        );

        let friday = Local.ymd(2017, 7, 7).and_hms(12, 0, 0);
        assert_eq!(
            "http://theclassicalstation.org/playing_fri.shtml",
            get_url(friday)
        );
    }

    #[test]
    fn test_parse_eastern_time_ok() {
        assert!(parse_eastern_time("00:00").is_ok());
        assert!(parse_eastern_time("12:00").is_ok());
        assert!(parse_eastern_time("23:59").is_ok());
        assert!(parse_eastern_time(" 1:34 ").is_ok());
    }

    #[test]
    fn test_parse_eastern_time_err() {
        assert!(parse_eastern_time("").is_err());
        assert!(parse_eastern_time("00").is_err());
        assert!(parse_eastern_time("-1").is_err());
        assert!(parse_eastern_time("24:00").is_err());
        assert!(parse_eastern_time("A:B").is_err());
    }

    #[test]
    fn test_lookup_in_html() {
        let req = Request{ time: Local::now() };

        assert_eq!(Error::HtmlParse, lookup_in_html(&req, ""));
    }
}
