# Classify Client

This is an optimized version of the classify client endpoint in [Normandy](https://github.com/mozilla/normandy).

## Dev instructions

This is a normal Cargo project, so after cloning the repository, you can build and run it with

```shell
$ cargo build
$ cargo run
```

This project should run on the latest stable version of Rust. Unstable features are not allowed.

### GeoIP Database

A GeoIP database will be downloaded automatically during the build.

> Note: It relies on `curl` and `tar` commands. See `build.rs` for insights about how to obtain
> the file manually in case the `.mmdb` file does not show up in the current folder.

## Configuration

Via environment variables:

- `HOST`: host (default: `"localhost"`)
- `PORT`: port number (default: `"8080"`)
- `GEOIP_DB_PATH`: path to GeoIP database (default: `"./GeoLite2-Country.mmdb"`)
- `SENTRY_DSN`: report errors to a Sentry instance (default: `""`)
- `VERSION_FILE`: path to `version.json` file (default: `"./version.json"`)

## Tests

Tests can be run with Cargo as well

```shell
$ cargo test
```

## Linting

Linting is handled via
[Therapist](https://therapist.readthedocs.io/en/latest/). After installing it,
enable the git hooks using either `therapist install` or `therapist install
--fix`. The `--fix` variant will automatically format your code upon commit.
The variant without `--fix` will simply show an error and ask you to reformat
the code using other means before committing.  Therapist runs in CI.

The checks Therapist runs are:

* Rustfmt
* Clippy, using the `clippy::all` preset

