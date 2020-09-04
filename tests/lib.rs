use {
    chrono::{Duration, Local, TimeZone},
    wowcpe::Request,
};

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
    let request = Request {
        time: Local::now() - Duration::days(6),
    };
    let response = wowcpe::lookup(&request).unwrap();

    assert!(response.start_time <= request.time);
    assert!(response.end_time >= request.time);
    assert!(!response.program.is_empty());
    assert!(!response.title.is_empty());
}

#[test]
fn test_long_ago() {
    let request = Request {
        time: Local.ymd(1950, 1, 1).and_hms(0, 0, 0),
    };
    let err = wowcpe::lookup(&request).unwrap_err();

    assert!(err.to_string().contains("no data"));
}
