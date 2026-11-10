"""Run the packaged node's local text path with a disposable identity on each OS."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile


def run(binary: Path, *arguments: str) -> str:
    return subprocess.run(
        [str(binary), *arguments], check=True, capture_output=True,
        text=True, encoding="utf-8", timeout=30,
    ).stdout


def field(output: str, name: str) -> str:
    return next(line.split("=", 1)[1] for line in output.splitlines()
                if line.startswith(name + "="))


def main() -> None:
    # Windows CI can redirect stdout using a legacy console code page. Emit
    # bilingual diagnostics as UTF-8 independently of the inherited locale.
    sys.stdout.reconfigure(encoding="utf-8")
    binary = Path(sys.argv[1]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="chatcommons-release-") as directory:
        state = str(Path(directory) / "state")
        initialized = run(binary, "init", "--state", state)
        user = field(initialized, "USER_ID")
        community = field(run(binary, "create-community", "--state", state,
                              "--name", "release smoke"), "COMMUNITY_ID")
        channel = field(run(binary, "create-channel", "--state", state,
                            "--community", community, "--name", "general"), "CHANNEL_ID")
        text = "release smoke / 发布验证"
        run(binary, "send-message", "--state", state, "--community", community,
            "--channel", channel, "--text", text)
        # Each command is a new process reopening the persisted identity and DB.
        assert field(run(binary, "info", "--state", state), "USER_ID") == user
        messages = json.loads(run(binary, "list-messages", "--state", state,
                                  "--community", community))
        assert len(messages) == 1 and messages[0]["text"] == text
        assert messages[0]["channelId"] == channel
    print("Packaged identity/text/reopen smoke passed / 发布身份、文字与重开验证通过")


if __name__ == "__main__":
    main()
