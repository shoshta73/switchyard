use chrono::{Datelike, Utc};
use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");

    if let Ok(tag) = env::var("GITHUB_REF_NAME") {
        if is_dev_release_tag(&tag) {
            println!("cargo:warning=SwitchYard version derived from GitHub tag: {tag}");
            println!("cargo:rustc-env=SWITCHYARD_VERSION={tag}");
            return;
        }

        println!("cargo:warning=Ignoring non-dev-release GitHub tag for SwitchYard version: {tag}");
    }

    let now = Utc::now();
    let iso_week = now.iso_week();
    let year = iso_week.year().rem_euclid(100);
    let version = format!("{year:02}w{:02}", iso_week.week());

    println!("cargo:warning=SwitchYard version derived from current ISO week: {version}");
    println!("cargo:rustc-env=SWITCHYARD_VERSION={version}");
}

fn is_dev_release_tag(tag: &str) -> bool {
    let bytes = tag.as_bytes();

    bytes.len() >= 5
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'w'
        && bytes[3].is_ascii_digit()
        && bytes[4].is_ascii_digit()
        && bytes[5..].iter().all(u8::is_ascii_lowercase)
}
