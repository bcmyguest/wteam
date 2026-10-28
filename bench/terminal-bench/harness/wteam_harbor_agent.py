"""Harbor agent: TB tasks with wteam harness arms (continuous / team).

Mounts (from harness/common.sh): wteam, the selected CLI and its auth file,
and in_container_{continuous,team}.sh.

Do NOT bind-mount only ~/.local/bin/agent — that wrapper looks for sibling node/
index.js and exits in ~100ms inside Docker (all-zero TB scores).
"""

from __future__ import annotations

import asyncio
import shlex
import subprocess
from pathlib import Path

from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext

_HOST_CLEANUP = Path(__file__).resolve().parent / "host_cleanup.sh"


class HarnessAgent(BaseAgent):
    """Run in_container scripts inside the TB sandbox (binaries via bind mounts)."""

    MARKER = "wteam_harness_agent.txt"

    @staticmethod
    def name() -> str:
        return "wteam-harness"

    def __init__(self, logs_dir: Path, model_name: str | None = None, **kwargs):
        super().__init__(logs_dir=logs_dir, model_name=model_name, **kwargs)
        self.mode = str(kwargs.get("mode") or "continuous")
        self.model = str(kwargs.get("model") or model_name or "auto")
        self.cli = str(kwargs.get("cli") or "cursor")
        self.pairs = str(kwargs.get("pairs") or "4")
        self.review = str(kwargs.get("review") or "autopass")
        self.lemonade_base_url = str(kwargs.get("lemonade_base_url") or "")
        self.opencode_continues = str(kwargs.get("opencode_continues") or "")
        self.opencode_fresh_retries = str(kwargs.get("opencode_fresh_retries") or "")
        self.opencode_verify_retries = str(kwargs.get("opencode_verify_retries") or "")
        self.opencode_variant = str(kwargs.get("opencode_variant") or "")
        # Cap wall time per task so a hard TB item cannot burn the whole sweep.
        # Harbor task.toml often sets agent timeout_sec=28800; we still enforce this.
        raw_timeout = kwargs.get("agent_timeout_sec") or kwargs.get("timeout_sec")
        try:
            self.agent_timeout_sec = int(raw_timeout) if raw_timeout else 1800
        except (TypeError, ValueError):
            self.agent_timeout_sec = 1800

    def version(self) -> str:
        return "0.5.4"

    @staticmethod
    def _host_cleanup(*, critical_ok: bool = True) -> int:
        """Reclaim Docker/tmp leftovers between trials. Returns cleanup exit code."""
        if not _HOST_CLEANUP.is_file():
            return 0
        try:
            proc = subprocess.run(
                ["sh", str(_HOST_CLEANUP)],
                check=False,
                timeout=180,
                capture_output=True,
                text=True,
            )
            # cleanup already appends to $TB_HARNESS_DIR/disk-monitor.log when set
            if proc.returncode == 2 and not critical_ok:
                raise RuntimeError(
                    "disk below TB_DISK_MIN_AVAIL_GB after between-task cleanup; "
                    "refusing further trials"
                )
            return int(proc.returncode or 0)
        except subprocess.TimeoutExpired:
            return 1
        except OSError:
            return 1
    async def setup(self, environment: BaseEnvironment) -> None:
        setup_result = await environment.exec(
            command=(
                "if [ ! -e /app ] && [ -d /task ]; then ln -s /task /app; fi; "
                "mkdir -p /opt/wteam-harness /logs/agent /usr/local/bin"
            )
        )
        if setup_result.return_code != 0:
            raise RuntimeError(f"Agent setup exited with code {setup_result.return_code}")
        if self.cli == "codex":
            command = (
                "mkdir -p /root/.codex && "
                "cp /opt/wteam-harness/auth.json /root/.codex/auth.json && "
                "chmod 600 /root/.codex/auth.json && "
                "test -x /usr/local/bin/codex && "
                "test -x /usr/local/bin/codex-code-mode-host && "
                "/usr/local/bin/codex --version >/logs/agent/agent-version.txt 2>&1"
            )
        elif self.cli == "cursor":
            command = (
                "test -x /opt/cursor-agent/cursor-agent && "
                "test -x /opt/cursor-agent/node && "
                "ln -sfn /opt/cursor-agent/cursor-agent /usr/local/bin/agent && "
                "ln -sfn /opt/cursor-agent/cursor-agent /usr/local/bin/cursor-agent && "
                "/usr/local/bin/agent --version >/logs/agent/agent-version.txt 2>&1"
            )
        elif self.cli == "opencode":
            command = (
                "mkdir -p /root/.config/opencode && "
                "cp /opt/wteam-harness/opencode.json /root/.config/opencode/opencode.json && "
                "test -x /usr/local/bin/opencode && "
                "/usr/local/bin/opencode --version >/logs/agent/agent-version.txt 2>&1"
            )
        else:
            raise ValueError(f"Unsupported harness CLI: {self.cli}")
        setup_result = await environment.exec(command=command)
        if setup_result.return_code != 0:
            raise RuntimeError(f"Agent setup exited with code {setup_result.return_code}")

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        (self.logs_dir / self.MARKER).write_text(
            f"mode={self.mode}\ncli={self.cli}\nmodel={self.model}\n"
            f"pairs={self.pairs}\nreview={self.review}\n"
            f"lemonade_base_url={self.lemonade_base_url}\n"
            f"opencode_continues={self.opencode_continues}\n"
            f"opencode_fresh_retries={self.opencode_fresh_retries}\n"
            f"opencode_verify_retries={self.opencode_verify_retries}\n"
            f"opencode_variant={self.opencode_variant}\n"
            f"agent_timeout_sec={self.agent_timeout_sec}\n",
            encoding="utf-8",
        )
        script = (
            "/opt/wteam-harness/in_container_team.sh"
            if self.mode == "team"
            else "/opt/wteam-harness/in_container_continuous.sh"
        )
        env = {
            "BENCH_MODEL": self.model,
            "BENCH_CLI": self.cli,
            "WTEAM_PAIRS": self.pairs,
            "WTEAM_BENCH_REVIEW": self.review,
            "WTEAM_SLUG": "tb",
            "HOME": "/root",
            "CODEX_HOME": "/root/.codex",
            "OPENAI_API_KEY": "lemonade",
            "PATH": "/usr/local/bin:/opt/cursor-agent:/usr/bin:/bin",
        }
        if self.lemonade_base_url:
            env["LEMONADE_BASE_URL"] = self.lemonade_base_url
        if self.opencode_continues:
            env["TB_OPENCODE_CONTINUES"] = self.opencode_continues
        if self.opencode_fresh_retries:
            env["TB_OPENCODE_FRESH_RETRIES"] = self.opencode_fresh_retries
        if self.opencode_verify_retries:
            env["TB_OPENCODE_VERIFY_RETRIES"] = self.opencode_verify_retries
        if self.opencode_variant:
            env["TB_OPENCODE_VARIANT"] = self.opencode_variant
        # Capture stdout/stderr so empty agent/ dirs are diagnosable.
        cmd = (
            f"export PATH=/usr/local/bin:/opt/cursor-agent:$PATH HOME=/root; "
            f"set -o pipefail; "
            f"{script} {shlex.quote(instruction)} "
            f"> >(tee /logs/agent/stdout.log) 2> >(tee /logs/agent/stderr.log >&2); "
            f"ec=$?; "
            f"if [ -d /app/.wteam/tb/logs ]; then "
            f"cp -R /app/.wteam/tb/logs /logs/agent/wteam-logs; fi; "
            f"if [ -f /app/.wteam/tb/state.json ]; then "
            f"cp /app/.wteam/tb/state.json /logs/agent/wteam-state.json; fi; "
            f"echo exit=$ec > /logs/agent/exit_code.txt; exit $ec"
        )
        exit_marker = self.logs_dir / "exit_code.txt"
        try:
            # Harbor's DockerEnvironment.exec raises RuntimeError("Command timed out
            # after N seconds") on timeout — that becomes exception_stats and skips
            # a clean reward=0 grade. Catch and treat as a normal agent failure.
            result = await environment.exec(
                command=cmd, env=env, timeout_sec=self.agent_timeout_sec
            )
            if not exit_marker.is_file():
                code = result.return_code if result.return_code is not None else 1
                exit_marker.write_text(f"exit={code}\n", encoding="utf-8")
        except (TimeoutError, asyncio.TimeoutError, RuntimeError) as exc:
            # DockerEnvironment turns asyncio.TimeoutError into
            # RuntimeError("Command timed out after N seconds").
            timed_out = isinstance(exc, (TimeoutError, asyncio.TimeoutError)) or (
                "timed out" in str(exc).lower()
            )
            if not timed_out:
                raise
            if not exit_marker.is_file():
                exit_marker.write_text("exit=124\n", encoding="utf-8")
            (self.logs_dir / "harness_agent_note.txt").write_text(
                f"agent_timeout code=124\n{type(exc).__name__}: {exc}\n",
                encoding="utf-8",
            )
            # Do not re-raise: Harbor can still run the verifier → reward 0
            # instead of counting the trial under exception_stats.
        finally:
            # Exit 2 from cleanup = disk critically low — abort so we do not
            # keep pulling multi-GB TB images onto a full root FS.
            self._host_cleanup(critical_ok=False)
