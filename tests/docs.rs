//! README.md documents only flags that the CLI accepts: every `--flag` in the
//! README must appear in some `wteam <cmd> --help` output.

use std::collections::BTreeSet;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;

/// Flags README shows for other tools (cargo, pre-commit). Not wteam flags.
const OTHER_TOOLS: &[&str] = &[
    "--git",
    "--locked",
    "--release",
    "--all-targets",
    "--check",
    "--install-hooks",
    "--hook-type",
];

/// `wteam --help` output for the given arguments.
fn help_of(args: &[&str]) -> String {
    let out = Command::new(cargo_bin("wteam"))
        .args(args)
        .arg("--help")
        .output()
        .expect("run wteam");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Top-level subcommand names, read from the `Commands:` block of `wteam --help`.
fn subcommands(top: &str) -> Vec<String> {
    top.lines()
        .skip_while(|l| *l != "Commands:")
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_string)
        .collect()
}

/// Every `--flag` token in `text`.
fn flags_in(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|t| {
            t.strip_prefix("--")
                .is_some_and(|rest| rest.starts_with(char::is_alphabetic))
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn extracts_flags_from_markdown() {
    let md = "run `wteam pickup --from .scratch/x` or `--json`, then `cargo test --locked`.";
    let flags = flags_in(md);
    assert!(flags.contains("--from"));
    assert!(flags.contains("--json"));
    assert!(flags.contains("--locked"));
    assert_eq!(flags.len(), 3);
}

#[test]
fn every_readme_flag_exists_in_cli_help() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("read README.md");
    let flags = flags_in(&readme);
    // Guards against an empty scan passing the check below.
    assert!(flags.contains("--json"), "README should document --json");
    assert!(flags.contains("--from"), "README should document --from");

    let top = help_of(&[]);
    let mut help = top.clone();
    for cmd in subcommands(&top) {
        help.push_str(&help_of(&[&cmd]));
    }
    let accepted = flags_in(&help);

    for flag in flags {
        if OTHER_TOOLS.contains(&flag.as_str()) {
            continue;
        }
        assert!(
            accepted.contains(&flag),
            "README flag {flag} is in no `wteam <cmd> --help` output"
        );
    }
}
