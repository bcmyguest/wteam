//! `wteam run`: the watcher loop, handoffs, budget and turn caps, with fake agents.

mod support;

use support::Repo;

/// Orchestrator body: log its prompt, then dispatch every ready ticket and end the turn.
const DISPATCH: &str = r#"printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/orch-prompts"
wteam ready | awk '/^[0-9]/{print $1}' | while read t; do wteam dispatch "$t" || exit 1; done
"#;

/// Worker: edit the ticket's file and submit it.
const WORKER: &str = r#"T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && $0 ~ /\[doing\]/ {print $1; exit}')
mkdir -p src
echo edit >> "src/$T.txt"
wteam submit "$T" --summary "did $T" || exit 1
"#;

/// Reviewer: pass whatever it holds.
const REVIEWER: &str = r#"T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && $0 ~ /\[review\]/ {print $1; exit}')
wteam verdict "$T" pass || exit 1
"#;

/// A three-ticket chain: 01 -> 02 -> 03, one file each.
fn chain() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.ticket("02", "Ticket two", "01", "src/02.txt", "");
    r.ticket("03", "Ticket three", "02", "src/03.txt", "");
    r.use_fake_agents();
    r
}

fn run_ok(r: &Repo, args: &[&str]) -> String {
    let out = r.wteam().arg("run").args(args).assert().success();
    String::from_utf8_lossy(&out.get_output().stdout).to_string()
}

/// Number of orchestrator prompts logged so far.
fn prompts(r: &Repo) -> usize {
    r.read(".wteam-fake/orch-prompts").matches("=====").count()
}

fn status_of(r: &Repo, ticket: &str) -> String {
    r.state()["tickets"][ticket]["status"]
        .as_str()
        .unwrap_or("todo")
        .to_string()
}

#[test]
fn chain_of_three_completes_and_reports_three_done() {
    let r = chain();
    r.agent_script("orchestrator", DISPATCH);
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &[]);

    assert!(
        text.contains("Run stopped: no ticket is runnable or in progress"),
        "{text}"
    );
    for (id, title) in [
        ("01", "Ticket one"),
        ("02", "Ticket two"),
        ("03", "Ticket three"),
    ] {
        assert!(text.contains(&format!("- {id}: {title}")), "{text}");
        assert_eq!(status_of(&r, id), "done");
    }
    assert!(text.contains("## Blocked\nnone"), "{text}");
}

#[test]
fn context_past_threshold_hands_off_once() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        &format!(
            r#"case "$WTEAM_PROMPT" in
  *"handoff baton"*)
    echo handoff >> "$WTEAM_FAKE_DIR/handoffs-asked"
    echo "HANDOFF-BATON"
    exit 0 ;;
esac
if [ -f "$WTEAM_FAKE_DIR/handoffs-asked" ]; then echo "tokens: 1000"; else echo "tokens: 160000"; fi
{DISPATCH}"#
        ),
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model"]);

    assert_eq!(
        r.read(".wteam-fake/handoffs-asked").lines().count(),
        1,
        "exactly one handoff"
    );
    let slug = r.slug.clone();
    let file = r.read(&format!(".wteam/{slug}/handoffs/orchestrator-1.md"));
    assert!(file.contains("HANDOFF-BATON"), "{file}");
    let prompts = r.read(".wteam-fake/orch-prompts");
    assert!(
        prompts.contains("Handoff from the previous orchestrator generation:\nHANDOFF-BATON"),
        "{prompts}"
    );
    assert_eq!(r.state()["orchestrator_generation"], 2);
    assert_eq!(status_of(&r, "01"), "done", "{text}");
}

#[test]
fn blocked_ticket_stops_run_with_dependents_todo() {
    let r = chain();
    r.agent_script("orchestrator", DISPATCH);
    r.agent_script(
        "worker",
        r#"T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && $0 ~ /\[doing\]/ {print $1; exit}')
wteam block "$T" --reason "needs spec" || exit 1
"#,
    );
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &[]);

    assert!(
        text.contains("Run stopped: no ticket is runnable or in progress"),
        "{text}"
    );
    assert!(text.contains("reason: needs spec"), "{text}");
    assert_eq!(status_of(&r, "01"), "blocked");
    assert_eq!(status_of(&r, "02"), "todo");
    assert_eq!(status_of(&r, "03"), "todo");
    // The orchestrator heard about the block in a digest.
    assert!(r.read(".wteam-fake/orch-prompts").contains("blocked 01:"));
}

#[test]
fn budget_stops_dispatching_once_tokens_exceed_it() {
    let r = chain();
    r.agent_script("orchestrator", DISPATCH);
    r.agent_script("worker", &format!("{WORKER}echo \"tokens: 500\"\n"));
    // Slow reviewer: the worker's turn records its tokens before the pass lands.
    r.agent_script("reviewer", &format!("sleep 1\n{REVIEWER}"));

    let text = run_ok(&r, &["--dispatch", "model", "--budget-tokens", "100"]);

    // The in-flight ticket finishes; nothing new is dispatched.
    assert!(text.contains("token budget of 100 exceeded"), "{text}");
    assert_eq!(status_of(&r, "01"), "done");
    assert_eq!(status_of(&r, "02"), "todo", "{text}");
    assert_eq!(
        prompts(&r),
        1,
        "orchestrator resumed after the budget was spent"
    );
}

#[test]
fn max_turns_stops_with_reason() {
    let r = chain();
    r.agent_script("orchestrator", DISPATCH);
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model", "--max-turns", "1"]);

    assert!(
        text.contains("Run stopped: --max-turns 1 reached"),
        "{text}"
    );
    assert_eq!(prompts(&r), 1);
    assert_eq!(status_of(&r, "01"), "done");
    assert_eq!(status_of(&r, "02"), "todo", "{text}");
    assert_eq!(status_of(&r, "03"), "todo");
}

#[test]
fn unlanded_isolation_and_transport_error() {
    let r = chain();
    r.wteam()
        .args(["run", "--isolation", "bogus"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown isolation `bogus`"));
    r.wteam()
        .args(["run", "--transport", "bogus"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown transport `bogus`"));
}

/// Orchestrator that dispatches, then logs one turn and tells a handoff apart by its marker.
fn handoff_orchestrator(tokens: u64) -> String {
    format!(
        r#"case "$WTEAM_PROMPT" in
  *"handoff baton"*)
    echo handoff >> "$WTEAM_FAKE_DIR/handoffs-asked"
    echo "HANDOFF-BATON"
    exit 0 ;;
esac
if [ -f "$WTEAM_FAKE_DIR/handoffs-asked" ]; then echo "tokens: 1000"; else echo "tokens: {tokens}"; fi
{DISPATCH}"#
    )
}

#[test]
fn all_idle_pairs_dirty_stops_with_dirty_reason() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script("orchestrator", "true\n");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);
    // Pair 1 holds uncommitted work from an earlier ticket.
    let pair = r.path().join(".wteam/demo/worktrees/pair1");
    r.git(&[
        "worktree",
        "add",
        "-b",
        "wteam/demo/zz",
        pair.to_str().unwrap(),
        "HEAD",
    ]);
    r.write(".wteam/demo/worktrees/pair1/stray.txt", "left behind");

    let text = run_ok(
        &r,
        &[
            "--dispatch",
            "model",
            "--pairs",
            "1",
            "--isolation",
            "worktree",
        ],
    );

    assert!(
        text.contains("Run stopped: no usable pair: dirty worktrees on pair 1."),
        "{text}"
    );
    assert!(!text.contains("did not dispatch"), "{text}");
}

#[test]
fn undispatched_ready_tickets_stop_after_two_idle_turns() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.ticket("02", "Ticket two", "None", "src/02.txt", "");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        r#"printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/orch-prompts"
"#,
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model"]);

    assert!(
        text.contains("Run stopped: orchestrator did not dispatch ready tickets: 01, 02"),
        "{text}"
    );
    // Kickoff, then one re-prompt naming the tickets; the second idle turn stops the run.
    assert_eq!(prompts(&r), 2);
    assert!(
        r.read(".wteam-fake/orch-prompts")
            .contains("Ready tickets are not dispatched: 01, 02"),
    );
    assert_eq!(status_of(&r, "01"), "todo");
}

#[test]
fn context_window_triggers_handoff_at_three_quarters() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    // 0.75 × 100000 = 75000, well below handoff_tokens (150000): 80000 must hand off.
    r.write(
        ".wteam/agents/orchestrator.toml",
        "name = \"orchestrator\"\ncli = \"fake\"\nprompt = \"orchestrator\"\nprofile = \"edit\"\ncontext_window = 100000\n",
    );
    r.agent_script("orchestrator", &handoff_orchestrator(80000));
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model"]);

    assert_eq!(
        r.read(".wteam-fake/handoffs-asked").lines().count(),
        1,
        "{text}"
    );
    assert_eq!(status_of(&r, "01"), "done", "{text}");
}

#[test]
fn dead_worker_turn_is_redelivered_mid_run() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script("orchestrator", DISPATCH);
    // First attempt: kill the turn's whole process group. Heal must start a new turn.
    r.agent_script(
        "worker",
        &format!(
            "if [ ! -f \"$WTEAM_FAKE_DIR/killed\" ]; then touch \"$WTEAM_FAKE_DIR/killed\"; kill -9 0; fi\n{WORKER}"
        ),
    );
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model", "--timeout", "60"]);

    assert!(r.fake_dir().join("killed").exists());
    assert!(!text.contains("timed out"), "{text}");
    assert_eq!(status_of(&r, "01"), "done", "{text}");
}

#[test]
fn mail_sent_to_orchestrator_mid_turn_waits_for_digest() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        &format!(
            "{DISPATCH}if [ ! -f \"$WTEAM_FAKE_DIR/sent\" ]; then touch \"$WTEAM_FAKE_DIR/sent\"; wteam send orchestrator.1 hi || exit 1; fi\n"
        ),
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model"]);

    // A detached turn would run with the bare mail as its prompt; a digest prefixes it with
    // "Events since". No prompt may start with the mail, and the mail must arrive in a digest.
    let prompts = r.read(".wteam-fake/orch-prompts");
    assert!(
        !prompts
            .split("=====")
            .any(|block| block.trim_start().starts_with("Message from")),
        "a detached orchestrator turn ran: {prompts}"
    );
    assert!(
        prompts.contains("Events since your last turn:\n\nMessage from orchestrator.1:\nhi"),
        "{prompts}"
    );
    assert_eq!(status_of(&r, "01"), "done", "{text}");
}

#[test]
fn orchestrator_turn_error_stops_with_report() {
    let r = chain();
    r.agent_script("orchestrator", "echo boom >&2\nexit 1\n");

    let text = run_ok(&r, &[]);

    assert!(
        text.contains("Run stopped: orchestrator turn failed:"),
        "{text}"
    );
    assert!(text.contains("# Stop report"), "{text}");
}

fn wait_until(what: &str, secs: u64, pred: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    while !pred() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[test]
fn stop_ends_foreground_orchestrator_turn_and_spares_the_watcher() {
    use std::os::unix::process::CommandExt as _;
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        "touch \"$WTEAM_FAKE_DIR/in-turn\"\nsleep 30\n",
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);
    // The watcher gets its own process group, so a stop that signalled it would show.
    let mut watcher = std::process::Command::new(assert_cmd::cargo::cargo_bin("wteam"))
        .args(["run", "--dispatch", "model"])
        .current_dir(r.path())
        .env_remove("WTEAM_AGENT")
        .env("WTEAM_ROOT", r.path())
        .env("WTEAM_SLUG", "demo")
        .env("WTEAM_FAKE_DIR", r.fake_dir())
        .env("WTEAM_CONFIG_DIR", r.path().join(".wteam-config"))
        .env("PATH", support::path_with_wteam())
        .process_group(0)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    wait_until("the orchestrator turn to start", 20, || {
        r.fake_dir().join("in-turn").exists()
    });
    // The adapter's pid is recorded as the turn pid, not the watcher's.
    wait_until("the turn pid to be recorded", 20, || {
        r.state()["agents"]["orchestrator.1"]["turn_pid"].is_u64()
    });
    r.wteam().arg("stop").assert().success();

    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = watcher.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "watcher still running after stop"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let out = watcher.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(status.success(), "watcher exited {status}: {text}");
    assert!(
        text.contains("Run stopped: orchestrator turn failed:"),
        "{text}"
    );
    assert_eq!(r.state()["run"]["active"], false);
}

#[test]
fn mail_to_orchestrator_outside_a_run_spawns_a_detached_turn() {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        "printf '%s\\n=====\\n' \"$WTEAM_PROMPT\" >> \"$WTEAM_FAKE_DIR/orch-prompts\"\n",
    );
    r.wteam()
        .args(["send", "orchestrator.1", "hi"])
        .assert()
        .success();

    assert!(r.effort().join("logs/orchestrator.1-process.log").exists());
    wait_until("the detached orchestrator turn", 20, || {
        r.read(".wteam-fake/orch-prompts").contains("hi")
    });
}

#[test]
fn budget_spent_during_orchestrator_turn_blocks_the_idle_reprompt() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.ticket("02", "Ticket two", "None", "src/02.txt", "");
    r.use_fake_agents();
    // Dispatch only the first ready ticket, then wait while its worker finishes and spends
    // the budget inside this turn. Ticket 02 is left ready with a pair idle.
    r.agent_script(
        "orchestrator",
        r#"printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/orch-prompts"
T=$(wteam ready | awk '/^[0-9]/{print $1; exit}')
wteam dispatch "$T" || exit 1
sleep 2
"#,
    );
    r.agent_script("worker", &format!("{WORKER}echo \"tokens: 500\"\n"));
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(
        &r,
        &[
            "--dispatch",
            "model",
            "--pairs",
            "2",
            "--budget-tokens",
            "100",
        ],
    );

    assert!(
        text.contains("Run stopped: token budget of 100 exceeded"),
        "{text}"
    );
    assert_eq!(
        prompts(&r),
        1,
        "idle re-prompt sent after the budget was spent"
    );
    assert_eq!(status_of(&r, "01"), "done", "{text}");
    assert_eq!(status_of(&r, "02"), "todo", "{text}");
    assert_eq!(r.state()["run"]["active"], false);
}

#[test]
fn stale_run_flag_with_dead_watcher_does_not_block_mail() {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        "printf '%s\\n=====\\n' \"$WTEAM_PROMPT\" >> \"$WTEAM_FAKE_DIR/orch-prompts\"\n",
    );
    // A pid that has exited and been reaped: no live watcher can own the run flag.
    let mut gone = std::process::Command::new("true").spawn().unwrap();
    let dead = gone.id();
    gone.wait().unwrap();
    r.write(
        ".wteam/demo/state.json",
        &serde_json::json!({
            "slug": "demo",
            "run": {
                "pairs": 1, "isolation": "none", "transport": "resume",
                "team": "default", "active": true, "watcher_pid": dead
            }
        })
        .to_string(),
    );

    r.wteam()
        .args(["send", "orchestrator.1", "hi"])
        .assert()
        .success();

    assert!(r.effort().join("logs/orchestrator.1-process.log").exists());
    wait_until("the detached orchestrator turn", 20, || {
        r.read(".wteam-fake/orch-prompts").contains("hi")
    });
    assert_eq!(r.state()["run"]["active"], false);
}

/// Orchestrator that dispatches one ready ticket per turn, ignoring `--fill`.
const ONE_PER_TURN: &str = r#"printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/orch-prompts"
t=$(wteam ready | awk '/^[0-9]/{print $1; exit}')
[ -n "$t" ] && wteam dispatch "$t" >/dev/null 2>&1
true
"#;

/// Run `tickets` independent tickets on `pairs` worktree pairs, dispatching one per turn.
fn one_per_turn_run(pairs: u32, tickets: u32) -> (Repo, String) {
    let r = Repo::new("demo");
    for n in 1..=tickets {
        r.ticket(
            &format!("{n:02}"),
            &format!("Ticket {n}"),
            "None",
            &format!("src/{n:02}.txt"),
            "",
        );
    }
    r.use_fake_agents();
    r.agent_script("orchestrator", ONE_PER_TURN);
    // A slow worker keeps pairs busy across the orchestrator's idle re-prompts.
    r.agent_script("worker", &format!("sleep 1\n{WORKER}"));
    r.agent_script("reviewer", REVIEWER);
    let pairs = pairs.to_string();
    let text = run_ok(
        &r,
        &[
            "--dispatch",
            "model",
            "--pairs",
            &pairs,
            "--isolation",
            "worktree",
        ],
    );
    (r, text)
}

#[test]
fn one_dispatch_per_turn_runs_every_ticket_with_two_pairs() {
    let (r, text) = one_per_turn_run(2, 5);
    assert!(!text.contains("did not dispatch"), "{text}");
    for n in 1..=5 {
        assert_eq!(status_of(&r, &format!("{n:02}")), "done", "{text}");
    }
}

#[test]
fn one_dispatch_per_turn_runs_every_ticket_with_four_pairs() {
    let (r, text) = one_per_turn_run(4, 8);
    assert!(!text.contains("did not dispatch"), "{text}");
    for n in 1..=8 {
        assert_eq!(status_of(&r, &format!("{n:02}")), "done", "{text}");
    }
}

#[test]
fn kickoff_and_digest_end_with_the_fill_hint() {
    let (r, _) = one_per_turn_run(2, 2);
    let prompts = r.read(".wteam-fake/orch-prompts");
    assert!(
        prompts
            .matches("Run `wteam dispatch --fill` to keep pairs busy.")
            .count()
            >= 2,
        "{prompts}"
    );
}

#[test]
fn auto_dispatch_finishes_every_ticket_with_no_orchestrator_turn() {
    let r = Repo::new("demo");
    for n in 1..=6 {
        let id = format!("0{n}");
        r.ticket(
            &id,
            &format!("Ticket {n}"),
            "None",
            &format!("src/{id}.txt"),
            "",
        );
    }
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        "echo called >> \"$WTEAM_FAKE_DIR/orch-called\"\nexit 1\n",
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--pairs", "2"]);

    assert!(
        text.contains("Run stopped: no ticket is runnable or in progress"),
        "{text}"
    );
    assert!(text.contains("Dispatch: auto"), "{text}");
    for n in 1..=6 {
        assert_eq!(status_of(&r, &format!("0{n}")), "done", "{text}");
    }
    assert!(!r.path().join(".wteam-fake/orch-called").exists(), "{text}");
}

#[test]
fn auto_wakes_orchestrator_once_for_a_block() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script(
        "orchestrator",
        "printf '%s\\n=====\\n' \"$WTEAM_PROMPT\" >> \"$WTEAM_FAKE_DIR/orch-prompts\"\nwteam requeue 01 --note retry || exit 1\n",
    );
    r.agent_script(
        "worker",
        r#"T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && $0 ~ /\[doing\]/ {print $1; exit}')
if [ ! -f "$WTEAM_FAKE_DIR/tried" ]; then
  touch "$WTEAM_FAKE_DIR/tried"
  wteam block "$T" --reason flaky || exit 1
  exit 0
fi
mkdir -p src
echo edit >> "src/$T.txt"
wteam submit "$T" --summary "did $T" || exit 1
"#,
    );
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &[]);

    assert_eq!(prompts(&r), 1, "{text}");
    let seen = r.read(".wteam-fake/orch-prompts");
    assert!(seen.contains("blocked 01: flaky"), "{seen}");
    assert_eq!(status_of(&r, "01"), "done", "{text}");
}

#[test]
fn model_dispatch_keeps_orchestrator_driven_dispatch() {
    let r = chain();
    r.agent_script("orchestrator", DISPATCH);
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--dispatch", "model"]);

    assert!(text.contains("Dispatch: model"), "{text}");
    assert!(prompts(&r) >= 1, "{text}");
    assert_eq!(status_of(&r, "03"), "done", "{text}");
}

#[test]
fn run_and_report_json_carry_the_dispatch_mode() {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.agent_script("orchestrator", "echo ok\n");
    for mode in ["auto", "model"] {
        let out = r
            .wteam()
            .args(["--json", "run", "--dispatch", mode])
            .assert()
            .success();
        let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
        assert_eq!(v["dispatch"], mode, "{v}");
        let out = r.wteam().args(["--json", "report"]).assert().success();
        let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
        assert_eq!(v["dispatch"], mode, "{v}");
    }
}

#[test]
fn auto_with_every_ticket_blocked_stops_with_a_report() {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.ticket("02", "Ticket two", "None", "src/02.txt", "");
    r.use_fake_agents();
    // The orchestrator reads the blocks and leaves them blocked.
    r.agent_script(
        "orchestrator",
        "printf '%s\\n=====\\n' \"$WTEAM_PROMPT\" >> \"$WTEAM_FAKE_DIR/orch-prompts\"\n",
    );
    r.agent_script(
        "worker",
        "T=$(wteam status | awk -v a=\"$WTEAM_AGENT\" '$NF==a && $0 ~ /\\[doing\\]/ {print $1; exit}')\nwteam block \"$T\" --reason nope || exit 1\n",
    );
    r.agent_script("reviewer", REVIEWER);

    let text = run_ok(&r, &["--timeout", "60"]);

    assert!(
        text.contains("Run stopped: no ticket is runnable or in progress"),
        "{text}"
    );
    assert!(text.contains("- 01: Ticket one"), "{text}");
    assert!(text.contains("- 02: Ticket two"), "{text}");
    assert_eq!(status_of(&r, "01"), "blocked", "{text}");
    assert_eq!(status_of(&r, "02"), "blocked", "{text}");
    assert!(prompts(&r) <= 2, "{text}");
}

/// One ticket, `01`, on `src/01.txt`.
fn one_ticket() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.use_fake_agents();
    r.agent_script("orchestrator", "exit 0\n");
    r.agent_script("reviewer", REVIEWER);
    r
}

/// Worker: block `01` until the planner has run, then submit it.
const BLOCK_UNTIL_REPLANNED: &str = r#"T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && $0 ~ /\[doing\]/ {print $1; exit}')
if [ ! -f "$WTEAM_FAKE_DIR/replanned" ]; then
  wteam block "$T" --reason "needs spec" || exit 1
  exit 0
fi
mkdir -p src
echo edit >> "src/$T.txt"
wteam submit "$T" --summary "did $T" || exit 1
"#;

/// Planner: log the call, mark it done, requeue `01`.
const PLANNER: &str = r#"echo call >> "$WTEAM_FAKE_DIR/planner-calls"
echo done > "$WTEAM_FAKE_DIR/replanned"
wteam requeue 01 --note "narrowed scope" || exit 1
"#;

/// Worker that blocks every time.
const ALWAYS_BLOCK: &str = r#"T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && $0 ~ /\[doing\]/ {print $1; exit}')
wteam block "$T" --reason "needs spec" || exit 1
"#;

fn planner_calls(r: &Repo) -> usize {
    r.read(".wteam-fake/planner-calls").lines().count()
}

#[test]
fn replan_rewrites_blocked_ticket_and_lands_it() {
    let r = one_ticket();
    r.agent_script("worker", BLOCK_UNTIL_REPLANNED);
    r.agent_script("planner", PLANNER);

    let text = run_ok(&r, &["--dispatch", "auto", "--replan"]);

    assert_eq!(r.state()["run"]["replan"], true);
    assert_eq!(planner_calls(&r), 1, "{text}");
    assert_eq!(status_of(&r, "01"), "done", "{text}");
    assert!(
        text.contains("Run stopped: no ticket is runnable or in progress"),
        "{text}"
    );
}

#[test]
fn without_replan_blocked_ticket_stops_the_run() {
    let r = one_ticket();
    r.agent_script("worker", BLOCK_UNTIL_REPLANNED);
    r.agent_script("planner", PLANNER);

    let text = run_ok(&r, &["--dispatch", "auto"]);

    assert_eq!(r.state()["run"]["replan"], false);
    assert_eq!(status_of(&r, "01"), "blocked", "{text}");
    assert_eq!(planner_calls(&r), 0, "planner ran without --replan");
}

#[test]
fn ticket_blocked_again_after_replan_stays_blocked() {
    let r = one_ticket();
    r.agent_script("worker", ALWAYS_BLOCK);
    r.agent_script("planner", PLANNER);

    let text = run_ok(&r, &["--dispatch", "auto", "--replan"]);

    assert_eq!(planner_calls(&r), 1, "planner must run once per ticket");
    assert_eq!(status_of(&r, "01"), "blocked", "{text}");
    assert!(text.contains("re-planned once"), "{text}");
}

#[test]
fn wteam_dir_writes_do_not_dirty_pickup() {
    // No fake agents: their scripts sit untracked in the repo root and would be flagged.
    let r = Repo::new("demo");
    r.ticket("01", "Ticket one", "None", "src/01.txt", "");
    r.write(".wteam/demo/notes.md", "planner scratch\n");
    let out = r.wteam().arg("pickup").assert();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(!text.contains("git tree is dirty"), "{text}");

    r.write("stray.txt", "outside .wteam\n");
    let out = r.wteam().arg("pickup").assert();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(text.contains("git tree is dirty: stray.txt"), "{text}");
}
