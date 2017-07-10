// Copyright 2017 Mitchell Kember. Subject to the MIT License.

extern crate chrono;
extern crate clap;
extern crate wowcpe;

use chrono::{DateTime, Local, Timelike};
use clap::{App, Arg};
use std::io::Write;

use wowcpe::{Request, Response};

fn main() {
    let matches = App::new("WOWCPE")
        .version("0.1.0")
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

    let time = if let Some(arg) = matches.value_of("time") {
        parse_time(arg).unwrap_or_else(|| invalid_arg(arg))
    } else {
        current_time()
    };

    match wowcpe::lookup(&Request { time }) {
        Ok(response) => print_response(&response),
        Err(err) => fail(&err.to_string()),
    }
}

fn current_time() -> DateTime<Local> {
    Local::now().with_nanosecond(0).unwrap()
}

fn parse_time(input: &str) -> Option<DateTime<Local>> {
    let input = input.trim();
    let (hour, minute): (u32, u32) = if let Some(index) = input.find(':') {
        let (hh, colon_mm) = input.split_at(index);
        let mm = &colon_mm[1..];
        if let (Ok(hour), Ok(minute)) = (hh.parse(), mm.parse()) {
            (hour, minute)
        } else {
            return None;
        }
    } else if let Ok(hour) = input.parse() {
        (hour, 0)
    } else {
        return None;
    };

    Local::now()
        .with_hour(hour)
        .and_then(|t| t.with_minute(minute))
        .and_then(|t| t.with_nanosecond(0))
}

fn print_response(r: &Response) {
    let fmt = "%I:%M %p";
    let start = r.start_time.time().format(fmt);
    let end = r.end_time.time().format(fmt);

    println!("Program       {}", r.program);
    println!("Time          {} - {}", start, end);
    println!("Composer      {}", r.composer);
    println!("Title         {}", r.title);
    println!("Performers    {}", r.performers);
    println!("Record Label  {}", r.record_label);
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
