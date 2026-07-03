"""把部件对位拼装成完整立绘，预览验证后导出分层 PSD。

LAYOUT: name -> (scale, cx, cy, z)
  scale: 部件相对切片原尺寸的缩放
  cx, cy: 部件中心在画布上的位置
  z: 绘制顺序（小的先画 = 在底下）
"""
import sys
import numpy as np
from PIL import Image

SRC = "assets-src/live2d/parts"
CANVAS = (900, 1100)  # w, h

# 初始布局：以 head_base 232x234、torso 213x226 为基准放大 ~2.8x
LAYOUT = {
    #                scale   cx    cy    z
    "tail":         (2.2,   700,  760,  0),
    "torso":        (2.8,   450,  760,  1),
    "leg_left":     (2.0,   330,  860,  2),
    "leg_right":    (2.0,   570,  860,  3),
    "ear_left":     (2.2,   190,  370,  4),
    "ear_right":    (2.2,   710,  370,  5),
    "head_base":    (2.8,   450,  330,  6),
    "brows":        (1.3,   450,  250,  7),
    "eye_left":     (0.92,  345,  305,  8),
    "eye_right":    (0.92,  555,  305,  9),
    "mouth_closed": (1.15,  450,  435,  10),
    "nose":         (1.0,   450,  385,  11),
}

def load(name):
    return Image.open(f"{SRC}/{name}.png")

def compose(layout, background=(111, 170, 111)):
    canvas = Image.new("RGBA", CANVAS, (*background, 255))
    for name, (scale, cx, cy, _z) in sorted(layout.items(), key=lambda kv: kv[1][3]):
        im = load(name)
        w, h = int(im.width * scale), int(im.height * scale)
        im = im.resize((w, h), Image.LANCZOS)
        canvas.alpha_composite(im, (int(cx - w / 2), int(cy - h / 2)))
    return canvas

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "assets-src/live2d/parts/compose_preview.png"
    compose(LAYOUT).convert("RGB").save(out)
    print("saved", out)
