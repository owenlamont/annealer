# /// script
# requires-python = ">=3.15"
# dependencies = ["typer>=0.26.7"]
# ///

import os
import shutil
import subprocess
from pathlib import Path
from typing import Annotated

import typer

app = typer.Typer(
    add_completion=False, context_settings={"help_option_names": ["-h", "--help"]}
)


@app.command()
def main(
    destination: Annotated[Path, typer.Argument(help="New fixture repository.")],
) -> None:
    """Create 100 deterministic SHA-1 commits; refuse an existing destination."""
    git = shutil.which("git")
    if git is None:
        raise typer.BadParameter("Git must be available on PATH")
    destination = destination.resolve()
    destination.mkdir()
    hooks = destination / "disabled-hooks"
    hooks.mkdir()
    environment = os.environ | {
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_AUTHOR_NAME": "Codestrand fixture",
        "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
        "GIT_COMMITTER_NAME": "Codestrand fixture",
        "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
    }

    def run(arguments: list[str]) -> None:
        subprocess.run(
            [git, "-C", str(destination), *arguments],
            env=environment,
            check=True,
            timeout=30,
            stdin=subprocess.DEVNULL,
        )

    run(["init", "--quiet", "--initial-branch=main", "--object-format=sha1"])
    run(["config", "core.autocrlf", "false"])
    run(["config", "core.hooksPath", str(hooks)])
    hooks.rmdir()
    for commit in range(100):
        with (destination / "history.txt").open(
            "a", encoding="utf-8", newline="\n"
        ) as stream:
            stream.write(f"commit {commit}\n")
        (destination / f"file-{commit}.txt").write_bytes(f"file {commit}\n".encode())
        timestamp = f"{1_700_000_000 + commit} +0000"
        environment.update(GIT_AUTHOR_DATE=timestamp, GIT_COMMITTER_DATE=timestamp)
        run(["add", "--all"])
        run(
            [
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                f"fixture {commit}",
            ]
        )
    run(["rev-parse", "HEAD"])


if __name__ == "__main__":
    app()
