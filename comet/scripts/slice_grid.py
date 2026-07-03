#!/usr/bin/env python3
"""
4×4 宫格 → 16 张高质量透明姿势切片。

用法:
    python3 -m venv .venv && .venv/bin/pip install pillow numpy scipy
    .venv/bin/python scripts/slice_grid.py [宫格图路径] [输出目录]

默认输入 src/assets/pet/_source-grid.png，输出 src/assets/pet/。

管线（解决朴素等分裁剪的三类问题）:
1. 连通域按质心归属格子——肢体/道具越过格线也能完整保留，不会被裁平；
2. 封闭白洞剔除——腿间、碗沿等被身体包围的白底块转为透明（面积阈值
   区分背景透出与眼睛高光）；
3. 白底去污染 + 软边缘——按 alpha 合成模型反解前景色，毛发边缘无白晕。
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

GRID = (4, 4)
NAMES = [
    "idle", "curious", "rest", "sleep",
    "walk_a", "walk_b", "run", "stretch",
    "petted", "grabbed", "greet", "sulk",
    "drink", "focus", "tired", "cheer",
]
# 低饱和亮色判定（白底/浅灰格线/软阴影）
SAT_MAX = 30
LUMA_MIN = 170
HOLE_MIN_AREA = 120   # 大于此面积的封闭白块视为背景透出
NOISE_MAX_AREA = 30   # 小于此面积的前景连通域视为噪点
PAD = 4


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "src/assets/pet/_source-grid.png"
    out_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else root / "src/assets/pet"
    out_dir.mkdir(parents=True, exist_ok=True)

    rgb = np.asarray(Image.open(src).convert("RGB")).astype(np.float64)
    h, w, _ = rgb.shape
    cell_w, cell_h = w // GRID[0], h // GRID[1]

    sat = rgb.max(axis=2) - rgb.min(axis=2)
    whiteish = (sat < SAT_MAX) & (rgb.min(axis=2) > LUMA_MIN)

    # 背景 = 与图像边界连通的 whiteish
    lbl, _ = ndimage.label(whiteish)
    border_ids = set(np.unique(np.concatenate(
        [lbl[0, :], lbl[-1, :], lbl[:, 0], lbl[:, -1]])))
    border_ids.discard(0)
    bg = np.isin(lbl, list(border_ids))

    # 封闭白洞：被前景包围的白底
    enclosed = whiteish & ~bg
    elbl, en = ndimage.label(enclosed)
    esizes = ndimage.sum(enclosed, elbl, range(1, en + 1))
    holes = np.isin(elbl, [i + 1 for i, s in enumerate(esizes) if s > HOLE_MIN_AREA])

    fg = ~bg & ~holes

    # 连通域按质心归属格子
    flbl, fn = ndimage.label(fg)
    centroids = ndimage.center_of_mass(fg, flbl, range(1, fn + 1))
    sizes = ndimage.sum(fg, flbl, range(1, fn + 1))
    cell_masks = [np.zeros((h, w), dtype=bool) for _ in NAMES]
    for i in range(fn):
        if sizes[i] < NOISE_MAX_AREA:
            continue
        cy, cx = centroids[i]
        cell = min(int(cy // cell_h), GRID[1] - 1) * GRID[0] + min(int(cx // cell_w), GRID[0] - 1)
        cell_masks[cell] |= flbl == (i + 1)

    for idx, name in enumerate(NAMES):
        mask = cell_masks[idx]
        if not mask.any():
            print(f"WARN: empty cell {name}", file=sys.stderr)
            continue
        ys, xs = np.where(mask)
        y0, y1 = max(ys.min() - PAD, 0), min(ys.max() + PAD + 1, h)
        x0, x1 = max(xs.min() - PAD, 0), min(xs.max() + PAD + 1, w)

        m = mask[y0:y1, x0:x1].astype(np.float64)
        c = rgb[y0:y1, x0:x1]

        # 软边缘：羽化后收紧过渡带
        alpha = ndimage.gaussian_filter(m, sigma=0.8)
        alpha = np.clip((alpha - 0.25) / 0.5, 0.0, 1.0)

        # 反解白底混合: C_obs = a*C_fg + (1-a)*255
        a3 = alpha[..., None]
        fg_color = np.where(
            a3 > 0.02,
            (c - (1 - a3) * 255.0) / np.maximum(a3, 1e-6),
            c,
        )
        out = np.dstack([np.clip(fg_color, 0, 255), alpha * 255.0]).astype(np.uint8)
        Image.fromarray(out).save(out_dir / f"{name}.png")
        print(f"{name}: {x1 - x0}x{y1 - y0}")


if __name__ == "__main__":
    main()
