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

extern crate chrono;
extern crate chrono_tz;
extern crate curl;
extern crate table_extract;

use chrono::{DateTime, Utc, Local, Datelike, Timelike};
use chrono_tz::US::Eastern;
use curl::easy::Easy;
use std::error;
use std::fmt;
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
    pub start: DateTime<Local>,
    /// Time the piece stopped (or will stop) playing.
    pub end: DateTime<Local>,
    /// Composer of the piece.
    pub composer: String,
    /// Title of the piece.
    pub title: String,
    /// Perfomers in the recording of the piece.
    pub performers: String,
}

/// An error that occurs while processing a request.
pub enum Error {
    CurlError(curl::Error),
    TableNotFoundError,
    TimeParseError,
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

const INVALID_UTF8: &'static str = "<invalid utf-8>";

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

const HEADERS: [&'static str; 5] =
    ["Program", "Start Time", "Composer", "Title", "Performers"];

fn lookup_in_html(request: &Request, html: &str) -> Result<Response> {
    let table = Table::find_by_headers(html, &HEADERS)
        .ok_or(Error::TableNotFoundError)?;

    let now = Local::now();
    let (row1, row2) = table
        .iter()
        .zip(table.iter().skip(1))
        .find(|&(row1, row2)| {
            row2.get("Start Time").map_or(false, |time| {
                parse_eastern_time(time).iter().any(|time| time > &now)
            })
        })
        .ok_or(Error::TableNotFoundError)?;


    Ok(Response {
        program: "".to_string(),
        start: request.time,
        end: request.time,
        composer: "".to_string(),
        title: "".to_string(),
        performers: "".to_string(),
    })
}

fn parse_eastern_time(input: &str) -> Result<DateTime<Local>> {
    let input = input.trim();
    let index = input.find(':').ok_or(Error::TimeParseError)?;

    let (hh, colon_mm) = input.split_at(index);
    let mm = &colon_mm[1..];
    let hour = hh.parse::<u32>()?;
    let minute = mm.parse::<u32>()?;

    Utc::now()
        .with_timezone(&Eastern)
        .with_hour(hour)
        .and_then(|t| t.with_minute(minute))
        .map(|t| t.with_timezone(&Local))
        .ok_or(Error::TimeParseError)
}

impl From<curl::Error> for Error {
    fn from(err: curl::Error) -> Error {
        Error::CurlError(err)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Error {
        Error::TimeParseError
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::CurlError(ref err) => err.fmt(f),
            _ => f.write_str(<Error as error::Error>::description(self)),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Error::CurlError(ref err) => err.fmt(f),
            _ => f.write_str(<Error as error::Error>::description(self)),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::CurlError(ref err) => err.description(),
            Error::TableNotFoundError => "Could not find table in HTML",
            Error::TimeParseError => "Could not parse time in table",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::CurlError(ref err) => err.cause(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
    fn test_lookup_in_html() {
        assert!(false)
    }

    #[test]
    fn test_parse_eastern_time() {
        assert!(false)
    }
}
