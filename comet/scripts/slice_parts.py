"""切片部件分解图：白底转透明 + 紧裁，输出各部件 PNG."""
import numpy as np
from PIL import Image
from scipy import ndimage

SRC = "assets-src/live2d/_source-parts-grid.png"
OUT = "assets-src/live2d/parts"
CELL_W, CELL_H = 384, 256
NAMES = [
    "reference", "torso", "head_base", "ear_right",
    "ear_left", "eye_left", "eye_right", "eyes_closed",
    "nose", "mouth_closed", "mouth_open", "leg_left",
    "leg_right", "tail", "tongue", "brows",
]

img = Image.open(SRC).convert("RGB")
rgb = np.asarray(img).astype(np.float64)
H, W, _ = rgb.shape

sat = rgb.max(axis=2) - rgb.min(axis=2)
whiteish = (sat < 30) & (rgb.min(axis=2) > 170)

lbl, _ = ndimage.label(whiteish)
border_ids = set(np.unique(np.concatenate(
    [lbl[0, :], lbl[-1, :], lbl[:, 0], lbl[:, -1]])))
border_ids.discard(0)
bg = np.isin(lbl, list(border_ids))

enclosed = whiteish & ~bg
elbl, en = ndimage.label(enclosed)
esizes = ndimage.sum(enclosed, elbl, range(1, en + 1))
# 部件图里眼睛高光/齿缝都要保留，只剔除大块封闭白
holes = np.isin(elbl, [i + 1 for i, s in enumerate(esizes) if s > 400])

fg = ~bg & ~holes

flbl, fn = ndimage.label(fg)
centroids = ndimage.center_of_mass(fg, flbl, range(1, fn + 1))
sizes = ndimage.sum(fg, flbl, range(1, fn + 1))
cell_masks = [np.zeros((H, W), dtype=bool) for _ in NAMES]
for i in range(fn):
    if sizes[i] < 25:
        continue
    cy, cx = centroids[i]
    cell = min(int(cy // CELL_H), 3) * 4 + min(int(cx // CELL_W), 3)
    cell_masks[cell] |= flbl == (i + 1)

import os
os.makedirs(OUT, exist_ok=True)
for idx, name in enumerate(NAMES):
    mask = cell_masks[idx]
    if not mask.any():
        print(f"WARN empty {name}")
        continue
    ys, xs = np.where(mask)
    pad = 3
    y0, y1 = max(ys.min() - pad, 0), min(ys.max() + pad + 1, H)
    x0, x1 = max(xs.min() - pad, 0), min(xs.max() + pad + 1, W)
    m = mask[y0:y1, x0:x1].astype(np.float64)
    c = rgb[y0:y1, x0:x1]
    alpha = ndimage.gaussian_filter(m, sigma=0.8)
    alpha = np.clip((alpha - 0.25) / 0.5, 0.0, 1.0)
    a3 = alpha[..., None]
    unblended = np.where(a3 > 0.02, (c - (1 - a3) * 255.0) / np.maximum(a3, 1e-6), c)
    out = np.dstack([np.clip(unblended, 0, 255), alpha * 255.0]).astype(np.uint8)
    Image.fromarray(out).save(f"{OUT}/{name}.png")
    print(f"{name}: {x1-x0}x{y1-y0}")
