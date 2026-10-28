"""Harbor agent: TB tasks with wteam harness arms (continuous / team).

Mounts (from harness/common.sh): wteam, the selected CLI and its auth file,
and in_container_{continuous,team}.sh.

Do NOT bind-mount only ~/.local/bin/agent — that wrapper looks for sibling node/
index.js and exits in ~100ms inside Docker (all-zero TB scores).
"""

from __future__ import annotations

import shlex
from pathlib import Path

from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext


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

    def version(self) -> str:
        return "0.4.0"

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
            f"lemonade_base_url={self.lemonade_base_url}\n",
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
        result = await environment.exec(command=cmd, env=env, timeout_sec=28800)
        if result.return_code != 0:
            raise RuntimeError(
                f"Agent command exited with code {result.return_code}; "
                "see agent/stdout.log and agent/stderr.log"
            )
        exit_marker = self.logs_dir / "exit_code.txt"
        if not exit_marker.is_file():
            raise RuntimeError("Agent command finished without agent/exit_code.txt")
        if exit_marker.read_text(encoding="utf-8").strip() != "exit=0":
            raise RuntimeError("Agent command reported failure in agent/exit_code.txt")
