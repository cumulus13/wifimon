use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("wifimon").unwrap()
}

#[test]
fn version_flag() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("wifimon"));
}
#[test]
fn version_subcommand() {
    cmd()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("wifimon"));
}
#[test]
fn help_flag() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Wi-Fi"));
}
#[test]
fn scan_help() {
    cmd().args(["scan", "--help"]).assert().success();
}
#[test]
fn list_help() {
    cmd().args(["list", "--help"]).assert().success();
}
#[test]
fn invalid_interval() {
    cmd().args(["--interval", "0"]).assert().failure();
}
#[test]
fn unknown_flag() {
    cmd().arg("--no-such-flag").assert().failure();
}
