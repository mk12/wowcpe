# WOWCPE

_What's On WCPE?_ 

WOWCPE is a simple command-line tool for the classical radio station [WCPE][1].

[1]: http://theclassicalstation.org/

## Build

You will need to have [Cargo][2] installed. If you want to get the source code and make changes, clone this repository and run `cargo install`. Otherwise, just run `cargo install wowcpe` anywhere to download it from https://crates.io and install it.

[2]: https://crates.io/

## Usage

There are two ways to use WOWCPE:

- `wowcpe`: Show what's playing on WCPE right now.
- `wowcpe -t HH:MM`: Show what will be playing at the time `HH:MM`.

Try `wowcpe --help` for more details..

## Contributing

Contributions are welcome! There are two things to keep in mind:

1. This project uses the nightly Rust toolchain from [rustup][3].
2. This project uses `cargo fmt` to keep the code tidy.

[3]: https://www.rustup.rs/

## License

© 2017 Mitchell Kember

WOWCPE is available under the MIT License; see [LICENSE](LICENSE.md) for details.
