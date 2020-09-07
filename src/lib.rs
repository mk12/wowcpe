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
//! [`Request`]: struct.Request.html

use {
    chrono::{DateTime, Datelike, Local, TimeZone, Timelike, Weekday},
    chrono_tz::US::Eastern,
    curl::easy::Easy,
    marksman_escape::Unescape,
    scraper::{ElementRef, Html, Selector},
    std::{error, fmt, result},
};

/// Request to look up what is playing on WCPE.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    /// The moment in time to look up.
    pub time: DateTime<Local>,
}

/// Information about a piece playing on WCPE.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response {
    /// Name of the current program, e.g., "Sleepers Awake".
    pub program: &'static str,
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

/// An error that occurs while processing a request.
#[derive(Debug)]
pub enum Error {
    Curl(curl::Error),
    NoData,
    NoEntry,
    BadUtf8,
    BadScrape,
    BadTime,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Curl(err) => err.fmt(f),
            Error::NoData => write!(f, "There is no data for the given time"),
            Error::NoEntry => write!(f, "Cannot find entry for the given time"),
            Error::BadUtf8 => write!(f, "Failed to parse HTML as UTF-8"),
            Error::BadScrape => write!(f, "Failed to scrape the HTML"),
            Error::BadTime => write!(f, "Failed to parse a time in the HTML"),
        }
    }
}

impl From<curl::Error> for Error {
    fn from(err: curl::Error) -> Self {
        Error::Curl(err)
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Curl(err) => Some(err),
            _ => None,
        }
    }
}

/// A specialized `Result` type for the `wowcpe` crate.
pub type Result<T> = result::Result<T, Error>;

/// Looks up what is playing on WCPE based on `request`.
///
/// Returns an error WCPE does not have data for `request.time`, e.g. if it is
/// in the future or too far in the past.
///
/// This will download a page from `https://theclassicalstation.org` using
/// `curl`, so it requires network access. Returns an error if `curl` fails or
/// if extracting the desired information from the HTML fails.
pub fn lookup(request: &Request) -> Result<Response> {
    validate_request(request, Local::now())?;
    let html = download(&get_url(request.time))?;
    lookup_in_html(request, &html)
}

fn validate_request(request: &Request, now: DateTime<Local>) -> Result<()> {
    // The website has no data before this date.
    let earliest = Eastern
        .ymd(2019, 12, 19)
        .and_hms(0, 0, 0)
        .with_timezone(&Local);
    let t = request.time;
    let end_of_day = eastern_eod(now);
    if t < earliest || t > end_of_day {
        Err(Error::NoData)
    } else {
        Ok(())
    }
}

fn get_url(time: DateTime<Local>) -> String {
    // The slash before the query string is important. Without that, we get a
    // 301 Moved Permanently response.
    format!(
        "https://theclassicalstation.org/listen/playlist/?date={}",
        time.format("%Y-%m-%d")
    )
}

fn download(url: &str) -> Result<String> {
    let mut body = Vec::new();
    let mut handle = Easy::new();
    handle.url(url)?;
    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            body.extend_from_slice(data);
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    String::from_utf8(body).or(Err(Error::BadUtf8))
}

fn lookup_in_html(request: &Request, html: &str) -> Result<Response> {
    fn sel(s: &str) -> Selector {
        Selector::parse(s).unwrap()
    }

    let root = Html::parse_fragment(html);
    let root = root.root_element();
    let root = root.select_one(&sel("article.block--playlist"))?;

    let mut end_time = None;
    let mut previous = None;
    for div in root.select(&sel("div.playlist-song")) {
        let time = div
            .select_one(&sel("div.playlist-song__time"))?
            .inner_html();
        let time = time.trim();
        if let Ok(time) = parse_eastern_time(request.time, time) {
            if time > request.time {
                end_time = Some(time);
                break;
            }
            previous = Some((time, div));
        } else {
            // This can happen on DST transitions, e.g. where 1am doesn't exist.
            println!("Note: skipping time {}", time);
        }
    }

    let (start_time, div) = previous.ok_or(Error::NoEntry)?;
    let end_time = end_time.unwrap_or_else(|| eastern_eod(request.time));

    let title = div
        .select(&sel("h4.playlist-song__title"))
        .next()
        .map(|h4| h4.inner_html().trim().to_string());

    let mut composer = None;
    let mut performers = None;
    let mut record_label = None;
    for li in div.select(&sel("ul.playlist-song__meta > li")) {
        let text = li.inner_html();
        let text = text.trim_start();
        if let Some(rest) = text.strip_prefix("Composed by:") {
            composer = Some(rest.to_string());
        } else if let Some(rest) = text.strip_prefix("Performed by:") {
            performers = Some(rest.to_string());
        } else if let Some(rest) = text.strip_prefix("Label:") {
            record_label = Some(rest.to_string());
        }
    }

    Ok(Response {
        program: get_program(request.time),
        start_time,
        end_time,
        composer: parse_field(composer),
        title: parse_field(title),
        performers: parse_field(performers),
        record_label: parse_field(record_label),
    })
}

trait SelectExt<'a> {
    fn select_one(&'a self, sel: &Selector) -> Result<ElementRef<'a>>;
}

impl<'a> SelectExt<'a> for ElementRef<'a> {
    fn select_one(&'a self, sel: &Selector) -> Result<ElementRef<'a>> {
        self.select(sel).next().ok_or(Error::BadScrape)
    }
}

const MISSING: &str = "<missing>";

fn parse_field(html: Option<String>) -> String {
    if let Some(html) = html {
        let bytes = html.trim().bytes();
        String::from_utf8(Unescape::new(bytes).collect()).unwrap()
    } else {
        MISSING.to_string()
    }
}

fn get_program(time: DateTime<Local>) -> &'static str {
    let allegro = "Allegro";
    let as_you_like_it = "As You Like It";
    let classical_cafe = "Classical Café";
    let concert_hall = "Concert Hall";
    let great_sacred_music = "Great Sacred Music";
    let metropolitan_opera = "Metropolitan Opera";
    let monday_night_at_the_symphony = "Monday Night at the Symphony";
    let music_in_the_night = "Music in the Night";
    let my_life_in_music = "My Life in Music";
    let peaceful_reflections = "Peaceful Reflections";
    let preview = "Preview!";
    let renaissance_fare = "Renaissance Fare";
    let rise_and_shine = "Rise and Shine";
    let saturday_evening_request_program = "Saturday Evening Request Program";
    let sing_for_joy = "Sing for Joy";
    let sleepers_awake = "Sleepers, Awake!";
    let thursday_night_opera_house = "Thursday Night Opera House";
    let wavelengths = "Wavelengths";
    let weekend_classics = "Weekend Classics";

    let time = time.with_timezone(&Eastern);

    // Specialty programs: https://theclassicalstation.org/listen/programs/
    match time.weekday() {
        Weekday::Mon => match time.hour() {
            19 => match time.day() {
                1..=7 => return my_life_in_music,
                8..=14 => return renaissance_fare,
                _ => (),
            },
            20..=21 => return monday_night_at_the_symphony,
            _ => (),
        },
        Weekday::Thu => {
            if let 19..=21 = time.hour() {
                return thursday_night_opera_house;
            }
        }
        Weekday::Sat => match (time.month(), time.hour()) {
            // NOTE: This is a guess. Sometimes starts earlier or ends later.
            (12, 13..=17) => return metropolitan_opera,
            (1..=5, 13..=17) => return metropolitan_opera,
            _ => (),
        },
        Weekday::Sun => match time.hour() {
            7 if time.minute() >= 30 => return sing_for_joy,
            8..=11 => return great_sacred_music,
            17 => match time.day() {
                7..=13 => return my_life_in_music,
                14..=20 => return renaissance_fare,
                _ => (),
            },
            18..=20 => return preview,
            21 => return wavelengths,
            22..=23 => return peaceful_reflections,
            _ => (),
        },
        _ => (),
    }

    // Regular programs: https://theclassicalstation.org/about-us/
    match time.weekday() {
        Weekday::Sat => match time.hour() {
            0..=5 => sleepers_awake,
            6..=17 => weekend_classics,
            18..=23 => saturday_evening_request_program,
            _ => unreachable!(),
        },
        Weekday::Sun => match time.hour() {
            0..=5 => sleepers_awake,
            6..=17 => weekend_classics,
            _ => unreachable!(),
        },
        _ => match time.hour() {
            0..=5 => sleepers_awake,
            6..=9 => rise_and_shine,
            10..=12 => classical_cafe,
            13..=15 => as_you_like_it,
            16..=18 => allegro,
            19..=21 => concert_hall,
            22..=23 => music_in_the_night,
            _ => unreachable!(),
        },
    }
}

fn parse_eastern_time(
    base: DateTime<Local>,
    input: &str,
) -> Result<DateTime<Local>> {
    let input = input.trim();
    let index = input.find(':').ok_or(Error::BadTime)?;
    let (hh, colon_mm_ampm) = input.split_at(index);
    let mm_ampm = &colon_mm_ampm[1..];
    if mm_ampm.len() != 4 {
        return Err(Error::BadTime);
    }
    let (mm, ampm) = mm_ampm.split_at(2);
    let (hour, minute) = match (hh.parse::<u32>(), mm.parse::<u32>(), ampm) {
        (Ok(0), _, _) => return Err(Error::BadTime),
        (Ok(12), Ok(m), "am") => (0, m),
        (Ok(h), Ok(m), "am") => (h, m),
        (Ok(12), Ok(m), "pm") => (12, m),
        (Ok(h), Ok(m), "pm") => (h + 12, m),
        _ => return Err(Error::BadTime),
    };

    base.with_timezone(&Eastern)
        .with_hour(hour)
        .and_then(|t| t.with_minute(minute))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .map(|t| t.with_timezone(&Local))
        .ok_or(Error::BadTime)
}

fn eastern_eod(base: DateTime<Local>) -> DateTime<Local> {
    base.with_timezone(&Eastern)
        .with_hour(23)
        .and_then(|t| t.with_minute(59))
        .and_then(|t| t.with_second(59))
        .and_then(|t| t.with_nanosecond(999_999_999))
        .unwrap()
        .with_timezone(&Local)
}

#[cfg(test)]
mod tests {
    use super::*;

    use {
        assert_matches::assert_matches, chrono::Duration,
        chrono_tz::US::Pacific,
    };

    #[test]
    fn test_validate_request_err() {
        let now = Local::now();

        let time = eastern_eod(now) + Duration::seconds(1);
        assert_matches!(validate_request(&Request { time }, now), Err(_));

        let time = Eastern
            .ymd(2019, 12, 18)
            .and_hms(12, 23, 59)
            .with_timezone(&Local);
        assert_matches!(validate_request(&Request { time }, now), Err(_));
    }

    #[test]
    fn test_validate_request_ok() {
        let now = Local::now();

        let time = now;
        assert_matches!(validate_request(&Request { time }, now), Ok(_));

        let time = eastern_eod(now);
        assert_matches!(validate_request(&Request { time }, now), Ok(_));

        let time = eastern_eod(now) - Duration::weeks(1);
        assert_matches!(validate_request(&Request { time }, now), Ok(_));
    }

    #[test]
    fn test_get_url_eastern() {
        let monday = Eastern
            .ymd(2017, 7, 3)
            .and_hms(0, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/listen/playlist/?date=2017-07-03",
            get_url(monday)
        );

        let friday = Eastern
            .ymd(2017, 7, 7)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/listen/playlist/?date=2017-07-07",
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
            "https://theclassicalstation.org/listen/playlist/?date=2017-07-03",
            get_url(monday)
        );

        let friday = Pacific
            .ymd(2017, 7, 7)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            "https://theclassicalstation.org/listen/playlist/?date=2017-07-08",
            get_url(friday)
        );
    }

    #[test]
    fn test_parse_eastern_time_err() {
        let now = Local::now();

        assert_matches!(parse_eastern_time(now, ""), Err(_));
        assert_matches!(parse_eastern_time(now, "00"), Err(_));
        assert_matches!(parse_eastern_time(now, "-1"), Err(_));
        assert_matches!(parse_eastern_time(now, "24:00"), Err(_));
        assert_matches!(parse_eastern_time(now, "A:B"), Err(_));
        assert_matches!(parse_eastern_time(now, "01:02"), Err(_));
        assert_matches!(parse_eastern_time(now, "01:02ZZ"), Err(_));
        assert_matches!(parse_eastern_time(now, "01:02AM"), Err(_));
        assert_matches!(parse_eastern_time(now, "00:01am"), Err(_));
    }

    #[test]
    fn test_parse_eastern_time_ok() {
        let now = Local::now();

        assert_matches!(parse_eastern_time(now, "12:00am"), Ok(_));
        assert_matches!(parse_eastern_time(now, " 12:00am "), Ok(_));
        assert_matches!(parse_eastern_time(now, "12:00am"), Ok(_));
        assert_matches!(parse_eastern_time(now, "11:59pm"), Ok(_));
        assert_matches!(parse_eastern_time(now, "3:34pm"), Ok(_));
    }

    #[test]
    fn test_parse_eastern_time_daylight_savings() {
        let base = Eastern
            .ymd(2019, 11, 3)
            .and_hms(0, 0, 0)
            .with_timezone(&Local);

        assert_matches!(parse_eastern_time(base, "1:34am"), Err(_));
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
            parse_eastern_time(base, "12:00pm").unwrap()
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
            parse_eastern_time(base, "12:00pm").unwrap()
        );
    }

    #[test]
    fn test_eastern_eod() {
        let almost_one_minute = Duration::minutes(1) - Duration::nanoseconds(1);

        let base = Local::now();
        assert_eq!(
            parse_eastern_time(base, "11:59pm").unwrap() + almost_one_minute,
            eastern_eod(base)
        );

        let base = Pacific
            .ymd(2017, 7, 10)
            .and_hms(23, 0, 0)
            .with_timezone(&Local);
        assert_eq!(
            parse_eastern_time(base, "11:59pm").unwrap() + almost_one_minute,
            eastern_eod(base)
        );
    }

    #[test]
    fn test_parse_field_none() {
        assert_eq!(MISSING, parse_field(None));
    }

    #[test]
    fn test_parse_field_some() {
        assert_eq!("Something", parse_field(Some(" Something ".to_string())));
        assert_eq!("a & b", parse_field(Some("a &amp; b ".to_string())));
        assert_eq!("'Twas so", parse_field(Some("&apos;Twas so".to_string())));
        assert_eq!("what &a;", parse_field(Some("what &a;".to_string())));
    }

    #[test]
    fn test_get_program_specialty() {
        let time = Eastern
            .ymd(2020, 9, 7)
            .and_hms(19, 0, 0)
            .with_timezone(&Local);
        assert_eq!("My Life in Music", get_program(time));
    }

    #[test]
    fn test_get_program_regular() {
        let time = Eastern
            .ymd(2020, 9, 4)
            .and_hms(12, 0, 0)
            .with_timezone(&Local);
        assert_eq!("Classical Cafe", get_program(time));
    }

    #[test]
    fn test_get_program_missing() {
        let time = Eastern
            .ymd(2020, 9, 5)
            .and_hms(2, 0, 0)
            .with_timezone(&Local);
        assert_eq!(MISSING, get_program(time));
    }

    #[test]
    fn test_lookup_in_html_parse_err() {
        let request = Request { time: Local::now() };

        assert_matches!(lookup_in_html(&request, ""), Err(_));
        assert_matches!(lookup_in_html(&request, "<table></table>"), Err(_));
        assert_matches!(
            lookup_in_html(&request, "<table><tr></tr></table>"),
            Err(_)
        );
    }

    const HTML: &'static str = r#"
<article class="block block--playlist">
    <div class="bound bound--layout">
        <h2 class="block__title">Playlist for September 1, 2020</h2>
        <h3 class="playlist-hour" id="playlist-hour-12am">12am</h3>
        <div class="playlist-songs">
            <div class="playlist-song">
                <div class="playlist-song__time">12:01am</div>
                <h4 class="playlist-song__title">Tasso: Lament &amp; Trimuph (Symphonic Poem No. 2)</h4>
                <ul class="playlist-song__meta">
                    <li>Composed by: Franz Liszt</li>
                    <li>Performed by: Gewandhaus Orchestra/Masur</li>
                    <li>Label: Naxos</li>
                    <li class="playlist-song__meta-half">Catalog Number: 01234</li>
                </ul>
            </div>			
        </div>
        <div class="playlist-songs">
            <div class="playlist-song">
                <div class="playlist-song__time">6:00am</div>
                <h4 class="playlist-song__title">Concerto Grosso in D, Op. 3 No. 6</h4>
                <ul class="playlist-song__meta">
                    <li>Composed by: George Frideric Handel</li>
                    <li>Performed by: Concentus Musicus of Vienna/Harnoncourt</li>
                    <li>Label: MHS</li>
                    <li class="playlist-song__meta-half">Catalog Number: 01234</li>
                </ul>
            </div>			
        </div>
    </div>
</article>
"#;

    #[test]
    fn test_lookup_in_html_too_early() {
        let time = parse_eastern_time(Local::now(), "12:00am").unwrap();
        assert_matches!(lookup_in_html(&Request { time }, HTML), Err(_));
    }

    #[test]
    fn test_lookup_in_html_first() {
        let t = Eastern
            .ymd(2020, 9, 4)
            .and_hms(0, 0, 0)
            .with_timezone(&Local);

        let expected = Response {
            program: "Sleepers Awake",
            start_time: parse_eastern_time(t, "12:01am").unwrap(),
            end_time: parse_eastern_time(t, "6:00am").unwrap(),
            composer: "Franz Liszt".to_string(),
            title: "Tasso: Lament & Trimuph (Symphonic Poem No. 2)".to_string(),
            performers: "Gewandhaus Orchestra/Masur".to_string(),
            record_label: "Naxos".to_string(),
        };

        let time = parse_eastern_time(t, "12:01am").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(t, "12:02am").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(t, "5:59am").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());
    }

    #[test]
    fn test_lookup_in_html_last() {
        let t = Eastern
            .ymd(2020, 9, 4)
            .and_hms(0, 0, 0)
            .with_timezone(&Local);

        let expected = Response {
            program: "Rise and Shine",
            start_time: parse_eastern_time(t, "6:00am").unwrap(),
            end_time: eastern_eod(t),
            composer: "George Frideric Handel".to_string(),
            title: "Concerto Grosso in D, Op. 3 No. 6".to_string(),
            performers: "Concentus Musicus of Vienna/Harnoncourt".to_string(),
            record_label: "MHS".to_string(),
        };

        let time = parse_eastern_time(t, "6:00am").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(t, "6:01am").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());

        let time = parse_eastern_time(t, "11:59pm").unwrap();
        assert_eq!(expected, lookup_in_html(&Request { time }, HTML).unwrap());
    }
}
