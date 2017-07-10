extern crate chrono;
extern crate wowcpe;

use chrono::{Duration, Local};
use wowcpe::Request;

#[test]
fn test_now() {
    let request = Request { time: Local::now() };
    let response = wowcpe::lookup(&request).unwrap();

    assert!(response.start_time <= request.time);
    assert!(response.end_time >= request.time);
    assert!(!response.program.is_empty());
    assert!(!response.title.is_empty());
}

#[test]
fn test_6_days_ago() {
    let request = Request { time: Local::now() - Duration::days(6) };
    let response = wowcpe::lookup(&request).unwrap();

    assert!(response.start_time <= request.time);
    assert!(response.end_time >= request.time);
    assert!(!response.program.is_empty());
    assert!(!response.title.is_empty());
}

#[test]
fn test_1_week_ago() {
    let request = Request { time: Local::now() - Duration::weeks(1) };
    let err = wowcpe::lookup(&request).unwrap_err();

    assert!(err.to_string().contains("not available"));
}
