# Landing demo (asciinema + agg)

Scripted stand-in agents run a small greet-CLI effort. **Source of truth:**
`play.sh` (tmux split + `main.sh`).

```bash
# From repo root (builds wteam if needed):
sh site/demo/render.sh
```

Uses host `asciinema`, `agg`, and `tmux` when present; otherwise Docker
(`rust:1-bookworm`) builds wteam, records, and converts to `public/demo.gif`.

Manual run (no GIF):

```bash
export WTEAM_BIN="$PWD/target/release"
sh site/demo/play.sh
```

Cast file (gitignored): `demo/.cache/demo.cast` — reconvert with
`agg demo/.cache/demo.cast public/demo.gif`.
