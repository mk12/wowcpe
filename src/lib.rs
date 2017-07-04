// Copyright 2017 Mitchell Kember. Subject to the MIT License.

///! # WCPE
///!
///! Web scraper for WCPE (theclassicalstation.org).
///!
///! This crate provides a single function [`lookup`] to find out what is
///! playing on WCPE at a given time. It returns a [`Response`], which contains
///! the title, duration, and other information about the piece.

extern crate chrono;
extern crate curl;

use chrono::prelude::*;
use curl::easy::Easy;
use std::error;
use std::fmt;

/// Request to look up what is playing on WCPE.
pub struct Request {
    pub time: DateTime<Local>,
}

/// Information about a piece playing on WCPE.
pub struct Response {
    pub start_time: DateTime<Local>,
    pub end_time: DateTime<Local>,
    pub composer: String,
    pub title: String,
    pub performers: String,
}

/// An error that occurs while processing a request.
pub enum Error {
    CurlError(curl::Error),
}

/// Looks up what is playing on WCPE based on `request`.
pub fn lookup(request: &Request) -> Result<Response, Error> {
    let mut body = String::new();
    let mut handle = Easy::new();
    handle.url(&get_url(request.time))?;
    // handle.write_function(|data| {
    //     body = String::from_utf8(data.to_vec()).unwrap();
    //     Ok(5)
    // })?;
    handle.perform()?;

    Ok(Response {
        start_time: request.time,
        end_time: request.time,
        composer: "".to_string(),
        title: "".to_string(),
        performers: "".to_string(),
    })
}

const WEEKDAYS: [&'static str; 7] =
    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn get_url(time: DateTime<Local>) -> String {
    let index = time.weekday().num_days_from_monday() as usize;
    format!(
        "http://theclassicalstation.org/playing_{}.shtml",
        WEEKDAYS[index]
    )
}

impl Error {
    fn error(&self) -> &error::Error {
        match *self {
            Error::CurlError(ref err) => err,
        }
    }
}

impl From<curl::Error> for Error {
    fn from(err: curl::Error) -> Error {
        Error::CurlError(err)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        std::fmt::Debug::fmt(&self.error(), f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        std::fmt::Display::fmt(&self.error(), f)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        self.error().description()
    }

    fn cause(&self) -> Option<&error::Error> {
        self.error().cause()
    }
}
