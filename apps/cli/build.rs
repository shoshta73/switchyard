use chrono::{Datelike, Utc};
use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");

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
    let week = iso_week.week();
    let prefix = format!("{year:02}w{week:02}");
    let version = local_snapshot_version(&prefix);

    println!("cargo:warning=SwitchYard version derived from local snapshot: {version}");
    println!("cargo:rustc-env=SWITCHYARD_VERSION={version}");
}

fn local_snapshot_version(prefix: &str) -> String {
    let Some(tag) = latest_git_tag(prefix) else {
        return format!("{prefix}a");
    };

    let suffix = &tag[prefix.len()..];
    let Some(next_suffix) = next_lowercase_suffix(suffix) else {
        return format!("{prefix}a");
    };

    format!("{prefix}{next_suffix}")
}

fn latest_git_tag(prefix: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["tag", "--list", &format!("{prefix}*"), "--sort=-v:refname"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .find(|tag| is_dev_release_tag(tag))
        .map(str::to_owned)
}

fn next_lowercase_suffix(suffix: &str) -> Option<String> {
    if suffix.is_empty() {
        return Some("a".to_owned());
    }

    let mut bytes = suffix.as_bytes().to_vec();
    if !bytes.iter().all(u8::is_ascii_lowercase) {
        return None;
    }

    for byte in bytes.iter_mut().rev() {
        if *byte < b'z' {
            *byte += 1;
            return String::from_utf8(bytes).ok();
        }

        *byte = b'a';
    }

    bytes.insert(0, b'a');
    String::from_utf8(bytes).ok()
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
