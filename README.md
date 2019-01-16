# Classify Client

 [![codecov](https://codecov.io/gh/mozilla/classify-client/branch/master/graph/badge.svg)](https://codecov.io/gh/mozilla/classify-client)

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

- `DEBUG`: A boolean that enables extra debugging options, such as a `/debug`
    endpoint that shows internal server state.
- `GEOIP_DB_PATH`: path to GeoIP database (default: `"./GeoLite2-Country.mmdb"`)
- `HOST`: host to bind to (default: `"localhost"`)
- `HUMAN_LOGS`: set to `true` to use human readable logging (default: MozLog as JSON)
- `METRICS_TARGET`: The host and port to send statsd metrics to. May be a
    hostname like `"metrics.example.com:8125"` or an IP like
    `"127.0.0.1:8125"`. Port is required. (default: `"localhost:8125"`)
- `PORT`: port number to bind to (default: `"8000"`)
- `SENTRY_DSN`: report errors to a Sentry instance (default: `""`)
- `TRUSTED_PROXY_LIST`: A comma-separated list of CIDR ranges that trusted
    proxies will be in. Supports both IPv4 and IPv6.
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

