use std::fs;
use std::path::Path;
use std::process::Command;

const URL: &str = "https://geolite.maxmind.com/download/geoip/database/GeoLite2-Country.tar.gz";

fn main() {
    let db_file = Path::new(".").join("GeoLite2-Country.mmdb");
    if !db_file.exists() {
        let tmp_file = ".geoip-db.tar.gz";
        Command::new("curl")
            .args(&[URL, "-o", &tmp_file])
            .status()
            .unwrap();
        Command::new("tar")
            .args(&[
                "--strip-components=1",
                "-zxvf",
                &tmp_file,
                "--wildcards",
                "*.mmdb",
            ])
            .status()
            .unwrap();
        fs::remove_file(tmp_file).unwrap();
    }
}
