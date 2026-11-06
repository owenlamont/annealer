# /// script
# requires-python = ">=3.15"
# dependencies = ["orjson>=3.12", "typer>=0.26.7"]
# ///

import csv
import math
import shutil
import subprocess
from collections.abc import Sequence
from pathlib import Path
from typing import Annotated, Final, TypedDict

import orjson
import typer

_FRAME_COUNT: Final = 300
_FPS: Final = 30
_HEADER: Final = [
    "frame",
    "seconds",
    "commit",
    "files",
    "avatars",
    "beams",
]


class _Stream(TypedDict):
    codec_name: str
    pix_fmt: str
    width: int
    height: int
    avg_frame_rate: str
    nb_read_frames: str


class _Format(TypedDict):
    duration: str


class _Probe(TypedDict):
    streams: list[_Stream]
    format: _Format


app = typer.Typer(
    add_completion=False, context_settings={"help_option_names": ["-h", "--help"]}
)


def _run(*, command: Sequence[str | Path], log: Path, timeout_seconds: int) -> None:
    typer.echo(f"Running {log.stem}; output: {log}")
    with log.open("wb") as stream:
        subprocess.run(
            command,
            check=True,
            timeout=timeout_seconds,
            stdin=subprocess.DEVNULL,
            stdout=stream,
            stderr=subprocess.STDOUT,
        )


def _verify_frames(frames: Path) -> None:
    expected = {f"frame-{frame:05d}.png" for frame in range(_FRAME_COUNT)}
    if {image.name for image in frames.glob("*.png")} != expected:
        raise typer.BadParameter("Expected exactly frames 00000 through 00299")
    if any((frames / name).stat().st_size == 0 for name in expected):
        raise typer.BadParameter("An exported PNG is empty")
    with (frames / "frames.csv").open(encoding="utf-8", newline="") as stream:
        rows = list(csv.reader(stream))
    if not rows or rows[0] != _HEADER or len(rows) != _FRAME_COUNT + 1:
        raise typer.BadParameter("Invalid frame manifest header or row count")
    for frame, row in enumerate(rows[1:]):
        if (
            len(row) != len(_HEADER)
            or int(row[0]) != frame
            or not math.isclose(float(row[1]), frame / _FPS, rel_tol=0, abs_tol=1e-6)
        ):
            raise typer.BadParameter(f"Invalid logical time or index for frame {frame}")


def _verify_probe(path: Path) -> None:
    probe: _Probe = orjson.loads(path.read_bytes())
    if len(probe["streams"]) != 1:
        raise typer.BadParameter("Expected exactly one video stream")
    stream = probe["streams"][0]
    if (
        stream["codec_name"] != "h264"
        or stream["pix_fmt"] != "yuv420p"
        or stream["width"] != 1280
        or stream["height"] != 720
        or stream["avg_frame_rate"] != "30/1"
        or stream["nb_read_frames"] != str(_FRAME_COUNT)
        or not math.isclose(
            float(probe["format"]["duration"]), 10, rel_tol=0, abs_tol=1e-6
        )
    ):
        raise typer.BadParameter(
            "Video must be H.264/yuv420p, 1280x720, 30fps, 300 frames, 10s"
        )


@app.command()
def main(
    *,
    binary: Annotated[Path, typer.Argument(help="Built Codestrand executable.")],
    output: Annotated[
        Path, typer.Argument(help="New output directory; must not exist.")
    ],
) -> None:
    """Verify a 10-second MP4 export; FFmpeg/libx264 and ffprobe must be on PATH.

    Source PNGs remain in OUTPUT/frames. Remove that directory after visual inspection
    to reclaim space; retain the MP4, CSV, probe and logs. Failures keep partial output
    for diagnosis. Retry in a new directory.
    """
    binary = binary.resolve(strict=True)
    if not binary.is_file():
        raise typer.BadParameter("Codestrand executable must be a file")
    ffmpeg = shutil.which("ffmpeg")
    ffprobe = shutil.which("ffprobe")
    if ffmpeg is None or ffprobe is None:
        raise typer.BadParameter("FFmpeg and ffprobe must be available on PATH")
    output = output.resolve()
    output.mkdir()
    frames = output / "frames"
    video = output / "codestrand.mp4"
    _run(
        command=[ffmpeg, "-version"],
        log=output / "ffmpeg-version.txt",
        timeout_seconds=30,
    )
    _run(
        command=[ffprobe, "-version"],
        log=output / "ffprobe-version.txt",
        timeout_seconds=30,
    )
    _run(
        command=[
            binary,
            "export",
            frames,
            "--frames",
            str(_FRAME_COUNT),
            "--fps",
            str(_FPS),
        ],
        log=output / "export.log",
        timeout_seconds=1800,
    )
    _verify_frames(frames)
    _run(
        command=[
            ffmpeg,
            "-hide_banner",
            "-nostdin",
            "-n",
            "-threads",
            "2",
            "-filter_threads",
            "2",
            "-framerate",
            str(_FPS),
            "-start_number",
            "0",
            "-i",
            frames / "frame-%05d.png",
            "-map",
            "0:v:0",
            "-an",
            "-c:v",
            "libx264",
            "-threads",
            "2",
            "-preset",
            "medium",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-fps_mode",
            "passthrough",
            "-movflags",
            "+faststart",
            video,
        ],
        log=output / "encode.log",
        timeout_seconds=600,
    )
    with (
        (output / "probe.json").open("wb") as stream,
        (output / "probe.log").open("wb") as log,
    ):
        subprocess.run(
            [
                ffprobe,
                "-v",
                "error",
                "-threads",
                "2",
                "-count_frames",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
                video,
            ],
            check=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
            stdout=stream,
            stderr=log,
        )
    _verify_probe(output / "probe.json")
    _run(
        command=[
            ffmpeg,
            "-hide_banner",
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-threads",
            "2",
            "-i",
            video,
            "-map",
            "0:v:0",
            "-f",
            "null",
            "-",
        ],
        log=output / "decode.log",
        timeout_seconds=120,
    )
    shutil.copyfile(frames / "frames.csv", output / "frames.csv")
    typer.echo(f"Verified 300 decoded frames, 1280x720, 30fps, 10s: {video}")
    typer.echo(f"After visual inspection, remove the PNG directory: {frames}")


if __name__ == "__main__":
    app()
