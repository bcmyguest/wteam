# Hidden polyglot Python, Codex `gpt-6-sol` (2026-10-27)

One full 34-exercise run per arm. `HIDDEN=1` removed the tests before agents started; `grade.sh` ran the pristine tests afterward. Both arms used `BENCH_CLI=codex` and `BENCH_MODEL=gpt-6-sol`, with `BENCH_LIMIT` unset. The team used four pairs, auto dispatch, `BENCH_REVIEW=autopass`, and `bench_isolation none`. Cost was not reported by the metrics files.

Polyglot checkout: `7e0611e77b54e2dea774cdc0aa00cf9f7ed6144f`. wteam base commit: `bcc3e126b589e00488b4c84680d55b9093e015d8`. The team runner needed a local `bench_write_agent` return-status fix before it could start; the first setup attempt stopped before any worker ran. The scored runs were sequential on the same machine. A two-exercise gap from one run per arm is not a reliable estimate of pass-rate difference.

| Arm | Wall seconds | Result | Failing exercises |
|---|---:|---:|---|
| Single continuous session | 383 | 24/34 | connect, dot-dsl, forth, go-counting, grep, hangman, list-ops, paasio, tree-building, wordy |
| wteam, four pairs, autopass reviewer | 613 | 26/34 | connect, forth, go-counting, grep, phone-number, react, tree-building, wordy |

## Single: `metrics.txt` and `grade.txt`

```text
wall_seconds 383
bench_context continuous
bench_mode continuous
bench_cli codex
bench_model gpt-6-sol
bench_lib polyglot
bench_polyglot_src /tmp/polyglot-benchmark
RESULT 24/34
```

```text
PASS affine-cipher
PASS beer-song
PASS book-store
PASS bottle-song
PASS bowling
FAIL connect
PASS dominoes
FAIL dot-dsl
PASS food-chain
FAIL forth
FAIL go-counting
PASS grade-school
FAIL grep
FAIL hangman
FAIL list-ops
FAIL paasio
PASS phone-number
PASS pig-latin
PASS poker
PASS pov
PASS proverb
PASS react
PASS rest-api
PASS robot-name
PASS scale-generator
PASS sgf-parsing
PASS simple-linked-list
PASS transpose
FAIL tree-building
PASS two-bucket
PASS variable-length-quantity
FAIL wordy
PASS zebra-puzzle
PASS zipper
RESULT 24/34
```

## Team: `metrics.txt` and `grade.txt`

```text
wall_seconds 613
bench_context team
bench_isolation none
bench_review autopass
bench_pairs 4
tickets 34
bench_mode team
bench_cli codex
bench_model gpt-6-sol
bench_lib polyglot
bench_polyglot_src /tmp/polyglot-benchmark
Run stopped: no ticket is runnable or in progress.
RESULT 26/34
```

```text
PASS affine-cipher
PASS beer-song
PASS book-store
PASS bottle-song
PASS bowling
FAIL connect
PASS dominoes
PASS dot-dsl
PASS food-chain
FAIL forth
FAIL go-counting
PASS grade-school
FAIL grep
PASS hangman
PASS list-ops
PASS paasio
FAIL phone-number
PASS pig-latin
PASS poker
PASS pov
PASS proverb
FAIL react
PASS rest-api
PASS robot-name
PASS scale-generator
PASS sgf-parsing
PASS simple-linked-list
PASS transpose
FAIL tree-building
PASS two-bucket
PASS variable-length-quantity
FAIL wordy
PASS zebra-puzzle
PASS zipper
RESULT 26/34
```
