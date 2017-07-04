// Copyright 2017 Mitchell Kember. Subject to the MIT License.

extern crate clap;
extern crate wcpe;

use clap::{Arg, App};

fn main() {
    let matches = App::new("wcpe")
        .about("Looks up what is playing on WCPE (theclassicalstation.org)")
        .arg(
            Arg::with_name("time")
                .short("t")
                .long("time")
                .value_name("HH:MM")
                .help("Look up a specific time")
                .takes_value(true),
        )
        .get_matches();

    let time = matches.value_of("time");
}
