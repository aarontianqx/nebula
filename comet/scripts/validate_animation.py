#!/usr/bin/env python3
"""Build real-time previews and structural continuity metrics for pet states."""

from __future__ import annotations

import json
import statistics
import sys
from pathlib import Path

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parent.parent
FRAMES_DIR = ROOT / "src/assets/pet/frames"
OUT_DIR = Path("/tmp/comet_animation_preview")

FPS = {
    "walk": 10,
    "run": 12,
    "drink": 8,
    "stretch": 8,
    "grabbed": 8,
}

SEQUENCE_FRAMES = {
    "walk": [6, 7, 8, 9, 10, 11],
    "run": [12, 13, 14, 15],
}

KEYFRAMES = {
    "cheer": ([0, 1, 5, 8, 12, 15], [180, 140, 220, 140, 180, 220]),
    "petted": ([0, 1, 2, 3, 2, 1], [140, 170, 220, 280, 180, 160]),
    "greet": ([0, 2, 4, 6, 9, 15], [160, 150, 180, 180, 180, 220]),
}


def load_state(state: str) -> tuple[list[Image.Image], list[int]]:
    files = sorted(
        FRAMES_DIR.glob(f"{state}_*.png"),
        key=lambda path: int(path.stem.rsplit("_", 1)[1]),
    )
    if not files:
        raise SystemExit(f"no frames found for {state}")
    source = [Image.open(path).convert("RGBA") for path in files]
    if state in KEYFRAMES:
        indices, durations = KEYFRAMES[state]
        return [source[index] for index in indices], durations
    if state in SEQUENCE_FRAMES:
        source = [source[index] for index in SEQUENCE_FRAMES[state]]
    duration = round(1000 / FPS.get(state, 6))
    return source, [duration] * len(source)


def alpha_metrics(frames: list[Image.Image]) -> dict[str, float]:
    areas: list[int] = []
    centers: list[tuple[float, float]] = []
    for frame in frames:
        alpha = frame.getchannel("A")
        bbox = alpha.getbbox()
        if bbox is None:
            raise SystemExit("empty animation frame")
        histogram = alpha.point(lambda value: 255 if value > 10 else 0).histogram()
        areas.append(histogram[255])
        centers.append(((bbox[0] + bbox[2]) / 2, (bbox[1] + bbox[3]) / 2))

    adjacent: list[float] = []
    pairs = list(zip(frames, frames[1:] + frames[:1]))
    for first, second in pairs:
        diff = ImageChops.difference(first, second).convert("RGB")
        mean = sum(value * count for value, count in enumerate(diff.convert("L").histogram()))
        adjacent.append(mean / (first.width * first.height * 255))

    mean_area = statistics.mean(areas)
    return {
        "area_cv": statistics.pstdev(areas) / mean_area if mean_area else 0,
        "center_x_range_px": max(x for x, _ in centers) - min(x for x, _ in centers),
        "center_y_range_px": max(y for _, y in centers) - min(y for _, y in centers),
        "adjacent_diff_mean": statistics.mean(adjacent),
        "adjacent_diff_max": max(adjacent),
        "loop_diff": adjacent[-1],
    }


def save_preview(state: str, frames: list[Image.Image], durations: list[int]) -> Path:
    background = (235, 235, 235, 255)
    rendered: list[Image.Image] = []
    for frame in frames:
        canvas = Image.new("RGBA", frame.size, background)
        canvas.alpha_composite(frame)
        rendered.append(canvas.convert("RGB"))
    out = OUT_DIR / f"{state}.gif"
    rendered[0].save(
        out,
        save_all=True,
        append_images=rendered[1:],
        duration=durations,
        loop=0,
        optimize=False,
    )
    return out


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    states = sys.argv[1:] or ["cheer", "greet", "petted", "walk", "run"]
    report = {}
    for state in states:
        frames, durations = load_state(state)
        metrics = alpha_metrics(frames)
        preview = save_preview(state, frames, durations)
        report[state] = {"frames": len(frames), "preview": str(preview), **metrics}
        print(f"{state}: {len(frames)} frames -> {preview}")
        print(json.dumps(metrics, sort_keys=True))
    (OUT_DIR / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
