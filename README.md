# Classify Client

This is an optimized version of the classify client endpoint in [Normandy][https://github.com/mozilla/normandy].

## Dev instructions

This is a normal Cargo project, so after cloning the repository, you can build and run it with

```shell
$ cargo build
$ cargo run
```

This project should run on the latest stable version of Rust. Unstable features are not allowed.

### GeoIP Database

A GeoIP database is required. For development purposes, one can be obtained from [the Maxmind website](https://dev.maxmind.com/geoip/geoip2/geolite2/). The GeoLite2 Country database is recommended. It should be placed at the root of the repository.

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

