//! CLI 集成测试：通过真实运行二进制，验证 I/O 契约与退出码。
//!
//! 重点验证"对脚本 / CI 友好"的行为：成功→0+stdout，失败→非零+stderr，
//! 以及子命令与 GUI 同源（动态生成，覆盖全部工具）。

use assert_cmd::Command;
use predicates::prelude::*;

fn pulsar() -> Command {
    Command::cargo_bin("pulsar").expect("binary builds")
}

#[test]
fn base64_encode_via_positional() {
    pulsar()
        .args(["base64", "hello world"])
        .assert()
        .success()
        .stdout(predicate::str::contains("aGVsbG8gd29ybGQ="));
}

#[test]
fn base64_decode_via_stdin_and_flag() {
    pulsar()
        .args(["base64", "--mode", "decode"])
        .write_stdin("aGVsbG8gd29ybGQ=")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world"));
}

#[test]
fn json_pretty_sorts_keys() {
    pulsar()
        .arg("json")
        .write_stdin(r#"{"b":2,"a":1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\": 1"));
}

#[test]
fn invalid_input_exits_nonzero_with_stderr() {
    pulsar()
        .arg("json")
        .write_stdin("not json")
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn unknown_tool_exits_nonzero() {
    // clap 处理未知子命令：非零退出。
    pulsar().arg("definitely-not-a-tool").assert().failure();
}

#[test]
fn int_flag_is_respected() {
    pulsar()
        .args(["password", "--length", "24", "--no-symbols"])
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("长度: 24"));
}

#[test]
fn enum_rejects_invalid_choice() {
    pulsar()
        .args(["base64", "--mode", "bogus", "x"])
        .assert()
        .failure();
}

#[test]
fn list_shows_all_thirty_tools() {
    pulsar()
        .arg("list")
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("共 30 个工具"));
}

#[test]
fn list_json_is_valid_array() {
    let out = pulsar()
        .args(["list", "--json"])
        .write_stdin("")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    assert_eq!(v.as_array().map(|a| a.len()), Some(30));
}

#[test]
fn detect_ranks_jwt_first_in_json() {
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMifQ.sig";
    let out = pulsar()
        .args(["detect", "--json"])
        .write_stdin(jwt)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("valid json");
    assert_eq!(v[0]["tool_id"], "encoders.jwt");
}

#[test]
fn no_subcommand_shows_help_and_fails() {
    // arg_required_else_help：无子命令时打印帮助并以非零退出。
    pulsar().assert().failure();
}

#[test]
fn full_id_subcommand_also_works() {
    // 短名之外，完整 id 也应可作为子命令（与 GUI id 一致，无歧义）。
    pulsar()
        .args(["encoders.base64", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("aGVsbG8="));
}
