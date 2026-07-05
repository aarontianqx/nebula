#!/usr/bin/env python3
"""
帧序列目检拼图：把切片产物拼成每状态一张 4 列联络表（contact sheet），
供重生成资产后快速人工检查削头/错位/下沉/道具丢失等问题。

用法:
    python scripts/preview_frames.py [state ...]   # 缺省全部状态
    输出到 /tmp/comet_preview/{state}.png
"""
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

FRAMES_DIR = Path(__file__).resolve().parent.parent / "src/assets/pet/frames"
OUT_DIR = Path("/tmp/comet_preview")
COLS = 4
MAX_SIZE = 1200
BG = (240, 240, 240, 255)


def build_sheet(state: str) -> Path | None:
    files = sorted(
        FRAMES_DIR.glob(f"{state}_*.png"),
        key=lambda p: int(p.stem.rsplit("_", 1)[1]),
    )
    if not files:
        return None
    # pngquant 产物为调色板模式，转 RGBA 才能作为透明蒙版粘贴
    frames = [Image.open(f).convert("RGBA") for f in files]
    w, h = frames[0].size
    rows = (len(frames) + COLS - 1) // COLS
    sheet = Image.new("RGBA", (COLS * w, rows * h), BG)
    for i, frame in enumerate(frames):
        sheet.paste(frame, ((i % COLS) * w, (i // COLS) * h), frame)
    sheet.thumbnail((MAX_SIZE, MAX_SIZE))
    out = OUT_DIR / f"{state}.png"
    sheet.save(out)
    return out


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    states = sys.argv[1:] or sorted(
        {f.stem.rsplit("_", 1)[0] for f in FRAMES_DIR.glob("*_*.png")}
    )
    for state in states:
        out = build_sheet(state)
        print(f"{state}: {out if out else 'no frames found'}")


if __name__ == "__main__":
    main()
