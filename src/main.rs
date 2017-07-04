// Copyright 2017 Mitchell Kember. Subject to the MIT License.

extern crate chrono;
extern crate clap;
extern crate wcpe;

use chrono::prelude::*;
use clap::{Arg, App};
use std::error::Error;
use std::io::Write;

fn main() {
    let matches = App::new("wcpe")
        .version("0.1.0")
        .author("Mitchell Kember")
        .about("Show what is playing on WCPE - theclassicalstation.org")
        .arg(
            Arg::with_name("time")
                .short("t")
                .long("time")
                .value_name("HH:MM")
                .help("Look up a specific time today")
                .takes_value(true),
        )
        .get_matches();

    let time = matches
        .value_of("time")
        .map(|arg| parse_time(arg).unwrap_or_else(|| invalid_arg(arg)))
        .unwrap_or_else(Local::now);

    match wcpe::lookup(&wcpe::Request { time }) {
        Ok(response) => print_response(response),
        Err(err) => fail(err.description()),
    }
}

fn parse_time(input: &str) -> Option<DateTime<Local>> {
    let input = input.trim();
    let (hour, minute) = if let Some(index) = input.find(':') {
        let (hh, colon_mm) = input.split_at(index);
        let mm = &colon_mm[1..];
        match (hh.parse::<u32>(), mm.parse::<u32>()) {
            (Ok(hour), Ok(minute)) => (hour, minute),
            _ => return None,
        }
    } else if let Ok(hour) = input.parse::<u32>() {
        (hour, 0)
    } else {
        return None;
    };

    Local::now()
        .with_hour(hour)
        .and_then(|t| t.with_minute(minute))
        .and_then(|t| t.with_nanosecond(0))
}

fn print_response(r: wcpe::Response) {
    println!("Time        {} - {}", r.start_time.time(), r.end_time.time());
    println!("Composer    {}", r.composer);
    println!("Title       {}", r.title);
    println!("Performers  {}", r.performers);
}

fn fail(message: &str) -> ! {
    writeln!(std::io::stderr(), "{}", message).unwrap();
    std::process::exit(1);
}

fn invalid_arg(arg: &str) -> ! {
    let mut stderr = std::io::stderr();
    writeln!(&mut stderr, "{}: Invalid argument", arg).unwrap();
    writeln!(&mut stderr, "For more information try --help").unwrap();
    std::process::exit(1)
}
