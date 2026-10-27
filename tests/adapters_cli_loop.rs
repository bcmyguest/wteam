//! Dispatch loop with fake CLI wrappers that replay OpenCode/Pi JSONL fixtures.

mod support;

use std::path::Path;

use serde_json::Value;
use support::Repo;

fn fixture(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(rel)
        .display()
        .to_string()
}

fn repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
    r.use_fake_agents();
    r
}

fn wrap_stdout(_cli: &str, fixture_rel: &str, before: &str) -> String {
    format!(
        "{before} >&2\ncat '{}'\n",
        fixture(fixture_rel).replace('\'', "'\\''")
    )
}

fn events(r: &Repo) -> Vec<Value> {
    let out = r
        .wteam()
        .args(["--json", "wait", "--timeout", "30"])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    v["events"].as_array().unwrap().clone()
}

#[test]
fn opencode_wrapper_replays_fixture_on_stdout() {
    let r = repo();
    r.wrap_cli(
        "opencode",
        &wrap_stdout(
            "opencode",
            "tests/fixtures/adapters/opencode/first-turn.jsonl",
            "true",
        ),
    );
    r.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"opencode\"\nprofile = \"full\"\nprompt = \"worker\"\n",
    );
    let out = std::process::Command::new(r.path().join("bin/opencode"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ses_1a2b3c"), "wrapper stdout: {stdout}");
}

#[test]
fn opencode_fixture_wrapper_completes_ticket() {
    let r = repo();
    r.wrap_cli(
        "opencode",
        &wrap_stdout(
            "opencode",
            "tests/fixtures/adapters/opencode/first-turn.jsonl",
            "wteam submit 01 --summary \"opencode fixture\"",
        ),
    );
    r.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"opencode\"\nprofile = \"full\"\nprompt = \"worker\"\n",
    );
    r.agent_script("reviewer", "wteam verdict 01 pass\n");
    r.wteam().args(["dispatch", "01"]).assert().success();
    let st = r.wait_for(30, |s| {
        !s["agents"]["worker.1"]["busy"].as_bool().unwrap_or(true)
    });
    let log = r.read(&format!(".wteam/{}/logs/worker.1-1.log", r.slug));
    assert!(
        log.contains("ses_1a2b3c"),
        "turn log missing fixture session id:\n{log}"
    );
    assert_eq!(st["agents"]["worker.1"]["turns"], 1, "worker log:\n{log}");
    assert_eq!(st["agents"]["worker.1"]["session_id"], "ses_1a2b3c");
    assert_eq!(st["agents"]["worker.1"]["context_tokens"], 1500);
    let ev = events(&r);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["kind"], "passed");
}

#[test]
fn opencode_truncated_fixture_still_parses_text() {
    let r = repo();
    r.wrap_cli(
        "opencode",
        &wrap_stdout(
            "opencode",
            "tests/fixtures/adapters/opencode/truncated-delta.jsonl",
            "wteam submit 01 --summary \"truncated\"",
        ),
    );
    r.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"opencode\"\nprofile = \"full\"\nprompt = \"worker\"\n",
    );
    r.agent_script("reviewer", "wteam verdict 01 pass\n");
    r.wteam().args(["dispatch", "01"]).assert().success();
    let st = r.wait_for(30, |s| {
        !s["agents"]["worker.1"]["busy"].as_bool().unwrap_or(true)
    });
    assert_eq!(st["agents"]["worker.1"]["session_id"], "ses_trunc");
    events(&r);
}

#[test]
fn pi_fixture_wrapper_completes_ticket() {
    let r = repo();
    r.wrap_cli(
        "pi",
        &wrap_stdout(
            "pi",
            "tests/fixtures/adapters/pi/first-turn.jsonl",
            "wteam submit 01 --summary \"pi fixture\"",
        ),
    );
    r.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"pi\"\nprofile = \"full\"\nprompt = \"worker\"\n",
    );
    r.agent_script("reviewer", "wteam verdict 01 pass\n");
    r.wteam().args(["dispatch", "01"]).assert().success();
    let st = r.wait_for(30, |s| {
        !s["agents"]["worker.1"]["busy"].as_bool().unwrap_or(true)
    });
    assert_eq!(
        st["agents"]["worker.1"]["session_id"],
        "0f6c1a2e-7b3d-4c8e-9a1f-2d5b6e7c8a90"
    );
    let ev = events(&r);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["kind"], "passed");
}
