#!/usr/bin/env python3
"""
多宫格帧序列切片：assets-src/frames/{state}_grid_4x4.png → 每状态 16 帧透明 PNG。

用法:
    python scripts/slice_frames.py            # 切片 + pngquant 压缩
    python scripts/slice_frames.py --no-quant # 跳过压缩（调试用）

依赖: pillow numpy scipy；压缩需系统安装 pngquant（缺失时自动跳过并提示）。

关键设计（保证播放协调感）:
1. 同一状态所有帧共用同一裁剪窗口（各帧前景 bbox 并集 + PAD）——
   帧间相对位置与原宫格一致，播放零错位;
2. 锚点对齐：逐帧以本体（质心 x，脚底 y）平移对齐，消除 AI 宫格随机漂移;
3. 前景触碰格子边缘时告警（内容被格线截断 = 素材问题，需重生成）;
4. 跨状态体型归一化：以 idle 状态的前景面积中位数为基准，
   输出各状态的建议缩放系数到 manifest.json，渲染端按系数缩放。

背景去除管线：白底/浅灰格线判定 → 边界连通 = 背景 → 封闭白洞剔除
（保留眼睛高光）→ 白底去污染反解前景色 + 软边缘。
"""
import json
import math
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

# state -> (grid file, rows, cols)；16 帧版（4x4 宫格，原生分辨率无超分）
GRIDS: dict[str, tuple[str, int, int]] = {
    "idle": ("idle_grid_4x4.png", 4, 4),
    "walk": ("walk_grid_4x4.png", 4, 4),
    "run": ("run_grid_4x4.png", 4, 4),
    "cheer": ("cheer_grid_4x4.png", 4, 4),
    "stretch": ("stretch_grid_4x4.png", 4, 4),
    "petted": ("petted_grid_4x4.png", 4, 4),
    "sleep": ("sleep_grid_4x4.png", 4, 4),
    "rest": ("rest_grid_4x4.png", 4, 4),
    "drink": ("drink_grid_4x4.png", 4, 4),
    "focus": ("focus_grid_4x4.png", 4, 4),
    "tired": ("tired_grid_4x4.png", 4, 4),
    "sulk": ("sulk_grid_4x4.png", 4, 4),
    "greet": ("greet_grid_4x4.png", 4, 4),
    "curious": ("curious_grid_4x4.png", 4, 4),
    "grabbed": ("grabbed_grid_4x4.png", 4, 4),
}

SAT_MAX = 30
LUMA_MIN = 170
HOLE_MIN_AREA = 120
NOISE_MAX_AREA = 40
PAD = 6
# 前景距格子边缘小于该值视为"内容触边"（可能被截断）
EDGE_WARN_PX = 2
# 每格内缩比例：裁掉格线及其模糊扩散（主体居中，不会误裁）
CELL_INSET_FRAC = 0.012

# 贴地状态：按脚部锚点对齐各帧，消除 AI 宫格的随机漂移。
# 排除空中动作（run 腾空 / cheer 跳跃 / grabbed 悬空），其纵向位移是真实动画。
GROUND_ALIGNED = {
    "idle", "walk", "sleep", "rest", "drink", "focus",
    "tired", "sulk", "greet", "curious", "stretch", "petted",
}
# 空中动作只做水平对齐（左右漂移仍是宫格噪声）
HORIZONTAL_ONLY = {"run", "cheer", "grabbed"}

# 预期的触边（不告警）：petted 的手从顶部伸入、grabbed 被拎的
# 后颈毛延伸到顶部，均为刻意出格设计而非截断。
EXPECTED_EDGE_TOUCH: dict[str, set[str]] = {
    "petted": {"top"},
    "grabbed": {"top"},
}


def extract_foreground(rgb: np.ndarray) -> np.ndarray:
    """单元格 RGB → 前景 mask（True=前景）。"""
    h, w, _ = rgb.shape
    sat = rgb.max(axis=2) - rgb.min(axis=2)
    whiteish = (sat < SAT_MAX) & (rgb.min(axis=2) > LUMA_MIN)

    lbl, _ = ndimage.label(whiteish)
    border_ids = set(np.unique(np.concatenate(
        [lbl[0, :], lbl[-1, :], lbl[:, 0], lbl[:, -1]])))
    border_ids.discard(0)
    bg = np.isin(lbl, list(border_ids))

    enclosed = whiteish & ~bg
    elbl, en = ndimage.label(enclosed)
    if en:
        esizes = ndimage.sum(enclosed, elbl, range(1, en + 1))
        holes = np.isin(elbl, [i + 1 for i, s in enumerate(esizes) if s > HOLE_MIN_AREA])
    else:
        holes = np.zeros_like(enclosed)

    fg = ~bg & ~holes

    # 去噪：保留最大连通域（本体）；次级连通域（爱心/Zzz/问号等装饰）
    # 仅在不触格子边界时保留——触边的次级块必为相邻格内容渗入或格线残留。
    flbl, fn = ndimage.label(fg)
    if fn:
        sizes = ndimage.sum(fg, flbl, range(1, fn + 1))
        body_id = int(np.argmax(sizes)) + 1
        keep = [body_id]
        for i in range(1, fn + 1):
            if i == body_id or sizes[i - 1] < NOISE_MAX_AREA:
                continue
            comp = flbl == i
            touches_border = (
                comp[0, :].any() or comp[-1, :].any()
                or comp[:, 0].any() or comp[:, -1].any()
            )
            if touches_border:
                continue
            ys, xs = np.where(comp)
            bh = ys.max() - ys.min() + 1
            bw = xs.max() - xs.min() + 1
            # 细线/低填充率碎片 = 格线残留或运动模糊拖影
            if (bh <= 6 and bw >= 25) or (bw <= 6 and bh >= 25):
                continue
            if sizes[i - 1] / (bh * bw) < 0.1:
                continue
            keep.append(i)
        fg = np.isin(flbl, keep)
    return fg


def body_anchor(mask: np.ndarray) -> tuple[float, float]:
    """最大连通域（宠物本体）的 (质心x, 脚底y)。忽略爱心/Z字等漂浮装饰。"""
    lbl, n = ndimage.label(mask)
    sizes = ndimage.sum(mask, lbl, range(1, n + 1))
    body = lbl == (int(np.argmax(sizes)) + 1)
    ys, xs = np.where(body)
    return float(xs.mean()), float(ys.max())


def cut_soft(rgb: np.ndarray, mask: np.ndarray) -> np.ndarray:
    """mask 区域 → RGBA（软边缘 + 白底去污染）。"""
    m = mask.astype(np.float64)
    alpha = ndimage.gaussian_filter(m, sigma=0.8)
    alpha = np.clip((alpha - 0.25) / 0.5, 0.0, 1.0)
    a3 = alpha[..., None]
    fg_color = np.where(
        a3 > 0.02,
        (rgb - (1 - a3) * 255.0) / np.maximum(a3, 1e-6),
        rgb,
    )
    return np.dstack([np.clip(fg_color, 0, 255), alpha * 255.0]).astype(np.uint8)


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    src_dir = root / "assets-src/frames"
    out_dir = root / "src/assets/pet/frames"
    out_dir.mkdir(parents=True, exist_ok=True)

    manifest: dict = {"states": {}}
    state_areas: dict[str, float] = {}
    warnings: list[str] = []

    for state, (fname, rows, cols) in GRIDS.items():
        img = np.asarray(Image.open(src_dir / fname).convert("RGB")).astype(np.float64)
        h, w, _ = img.shape
        cell_h, cell_w = h // rows, w // cols

        inset_y = round(cell_h * CELL_INSET_FRAC)
        inset_x = round(cell_w * CELL_INSET_FRAC)
        masks: list[np.ndarray] = []
        cells: list[np.ndarray] = []
        for r in range(rows):
            for c in range(cols):
                cell = img[
                    r * cell_h + inset_y:(r + 1) * cell_h - inset_y,
                    c * cell_w + inset_x:(c + 1) * cell_w - inset_x,
                ]
                mask = extract_foreground(cell)
                if not mask.any():
                    warnings.append(f"{state}#{r * cols + c}: empty cell")
                    continue
                # 触边检测（削头/截肢告警）
                ch2, cw2 = mask.shape
                ys, xs = np.where(mask)
                touches = []
                if ys.min() < EDGE_WARN_PX:
                    touches.append("top")
                if ys.max() >= ch2 - EDGE_WARN_PX:
                    touches.append("bottom")
                if xs.min() < EDGE_WARN_PX:
                    touches.append("left")
                if xs.max() >= cw2 - EDGE_WARN_PX:
                    touches.append("right")
                touches = [
                    t for t in touches
                    if t not in EXPECTED_EDGE_TOUCH.get(state, set())
                ]
                if touches:
                    warnings.append(
                        f"{state}#{len(cells)}: content touches cell edge ({', '.join(touches)})")
                masks.append(mask)
                cells.append(cell)

        # --- 锚点对齐：消除 AI 宫格各格子的随机位置漂移 ---
        # 锚点 = (前景质心 x, 脚底 y)。以各帧锚点均值为共同锚点，
        # 平移每帧内容，使锚点重合后再统一裁剪。
        anchors = [body_anchor(mask) for mask in masks]
        cx_ref = float(np.mean([a[0] for a in anchors]))
        fy_ref = float(np.mean([a[1] for a in anchors]))

        shifted_cells: list[np.ndarray] = []
        shifted_masks: list[np.ndarray] = []
        for (cx, fy), cell, mask in zip(anchors, cells, masks):
            dx = round(cx_ref - cx)
            dy = round(fy_ref - fy) if state in GROUND_ALIGNED else 0
            shifted_cells.append(
                ndimage.shift(cell, (dy, dx, 0), order=0, cval=255.0))
            shifted_masks.append(
                ndimage.shift(mask, (dy, dx), order=0, cval=False))

        # 状态级共用裁剪窗口 = 对齐后各帧 bbox 并集
        in_h, in_w = shifted_masks[0].shape
        ys_all = np.concatenate([np.where(m)[0] for m in shifted_masks])
        xs_all = np.concatenate([np.where(m)[1] for m in shifted_masks])
        y0 = max(int(ys_all.min()) - PAD, 0)
        y1 = min(int(ys_all.max()) + PAD + 1, in_h)
        x0 = max(int(xs_all.min()) - PAD, 0)
        x1 = min(int(xs_all.max()) + PAD + 1, in_w)

        frame_files = []
        areas = []
        for i, (cell, mask) in enumerate(zip(shifted_cells, shifted_masks)):
            rgba = cut_soft(cell[y0:y1, x0:x1], mask[y0:y1, x0:x1])
            fn_out = f"{state}_{i}.png"
            Image.fromarray(rgba).save(out_dir / fn_out)
            frame_files.append(fn_out)
            areas.append(float(mask.sum()))

        # 体型归一化基准：前景面积（对姿势变化鲁棒；毛发蓬松差异可接受）
        state_areas[state] = float(np.median(areas))
        manifest["states"][state] = {
            "frames": frame_files,
            "w": x1 - x0,
            "h": y1 - y0,
            "cellW": cell_w,
            "cellH": cell_h,
        }
        print(f"{state}: {len(frame_files)} frames, window {x1 - x0}x{y1 - y0}")

    # 以 idle 为基准计算各状态建议缩放（面积比开方 = 线性尺度比）
    base = state_areas["idle"]
    for state, area in state_areas.items():
        manifest["states"][state]["scale"] = round(math.sqrt(base / area), 4)

    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False))
    print(f"\nmanifest written; base(idle) area={base:.0f}")

    if "--no-quant" not in sys.argv:
        compress(out_dir)

    if warnings:
        print("\n=== WARNINGS ===", file=sys.stderr)
        for wmsg in warnings:
            print(f"  {wmsg}", file=sys.stderr)


def compress(out_dir: Path) -> None:
    """pngquant 有损量化压缩全部帧（~75% 体积缩减，视觉无损）。"""
    if not shutil.which("pngquant"):
        print("pngquant not found, skip compression "
              "(brew install pngquant)", file=sys.stderr)
        return
    total_before = total_after = 0
    for f in sorted(out_dir.glob("*.png")):
        total_before += f.stat().st_size
        subprocess.run(
            ["pngquant", "--quality=75-92", "--force", "--output", str(f), str(f)],
            check=False,
        )
        total_after += f.stat().st_size
    print(f"compressed: {total_before / 1e6:.1f}MB -> {total_after / 1e6:.1f}MB")


if __name__ == "__main__":
    main()
