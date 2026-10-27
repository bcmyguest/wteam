"""Harbor agent: TB tasks with wteam harness arms (continuous / team).

Mounts (from harness/common.sh):
  /opt/cursor-agent  — full Cursor agent install (node + index.js + cursor-agent)
  /usr/local/bin/wteam
  /root/.config/cursor/auth.json
  /opt/wteam-harness/in_container_{continuous,team}.sh

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

    def version(self) -> str:
        return "0.2.0"

    async def setup(self, environment: BaseEnvironment) -> None:
        # Wire PATH to the full cursor-agent package dir (not a lone wrapper file).
        await environment.exec(
            command=(
                "mkdir -p /opt/wteam-harness /logs/agent /usr/local/bin && "
                "chmod +x /opt/wteam-harness/*.sh 2>/dev/null || true; "
                "test -x /opt/cursor-agent/cursor-agent || "
                "{ echo 'FATAL: /opt/cursor-agent/cursor-agent missing — mount full agent dir'; exit 1; }; "
                "test -x /opt/cursor-agent/node || "
                "{ echo 'FATAL: /opt/cursor-agent/node missing'; exit 1; }; "
                "ln -sfn /opt/cursor-agent/cursor-agent /usr/local/bin/agent; "
                "ln -sfn /opt/cursor-agent/cursor-agent /usr/local/bin/cursor-agent; "
                "/usr/local/bin/agent --version >/logs/agent/agent-version.txt 2>&1 || "
                "{ echo 'FATAL: agent --version failed'; cat /logs/agent/agent-version.txt; exit 1; }"
            ),
        )

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        (self.logs_dir / self.MARKER).write_text(
            f"mode={self.mode}\nmodel={self.model}\n",
            encoding="utf-8",
        )
        script = (
            "/opt/wteam-harness/in_container_team.sh"
            if self.mode == "team"
            else "/opt/wteam-harness/in_container_continuous.sh"
        )
        env = {
            "BENCH_MODEL": self.model,
            "WTEAM_SLUG": "tb",
            "HOME": "/root",
            "PATH": "/usr/local/bin:/opt/cursor-agent:/usr/bin:/bin",
        }
        # Capture stdout/stderr so empty agent/ dirs are diagnosable.
        cmd = (
            f"export PATH=/usr/local/bin:/opt/cursor-agent:$PATH HOME=/root; "
            f"set -o pipefail; "
            f"{script} {shlex.quote(instruction)} "
            f"> >(tee /logs/agent/stdout.log) 2> >(tee /logs/agent/stderr.log >&2); "
            f"ec=$?; echo exit=$ec > /logs/agent/exit_code.txt; exit $ec"
        )
        await environment.exec(command=cmd, env=env, timeout_sec=28800)
