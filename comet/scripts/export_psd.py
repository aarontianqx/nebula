"""导出 Live2D 用分层 PSD：所有部件独立图层，替换部件（闭眼/张嘴/舌头）默认隐藏."""
import numpy as np
from PIL import Image
from pytoshop.user import nested_layers
from pytoshop import enums

SRC = "assets-src/live2d/parts"
CANVAS_W, CANVAS_H = 900, 1100

# (name, scale, cx, cy, visible)  自底向上
LAYERS = [
    ("tail",         2.2,  700, 760,  True),
    ("torso",        2.8,  450, 760,  True),
    ("leg_left",     2.0,  330, 860,  True),
    ("leg_right",    2.0,  570, 860,  True),
    ("ear_left",     2.2,  190, 370,  True),
    ("ear_right",    2.2,  710, 370,  True),
    ("head_base",    2.8,  450, 330,  True),
    ("brows",        1.3,  450, 250,  True),
    ("eye_left",     0.92, 345, 305,  True),
    ("eye_right",    0.92, 555, 305,  True),
    ("eyes_closed",  1.0,  450, 305,  False),
    ("mouth_closed", 1.15, 450, 435,  True),
    ("tongue",       0.9,  450, 480,  False),
    ("mouth_open",   1.15, 450, 445,  False),
    ("nose",         1.0,  450, 385,  True),
]

layers = []
# pytoshop 图层顺序：列表首位是最顶层
for name, scale, cx, cy, visible in reversed(LAYERS):
    im = Image.open(f"{SRC}/{name}.png")
    w, h = int(im.width * scale), int(im.height * scale)
    im = im.resize((w, h), Image.LANCZOS)
    arr = np.asarray(im)
    left = int(cx - w / 2)
    top = int(cy - h / 2)
    layer = nested_layers.Image(
        name=name,
        visible=visible,
        top=top,
        left=left,
        channels={
            0: arr[..., 0].copy(),
            1: arr[..., 1].copy(),
            2: arr[..., 2].copy(),
            -1: arr[..., 3].copy(),
        },
    )
    layers.append(layer)

psd = nested_layers.nested_layers_to_psd(
    layers,
    color_mode=enums.ColorMode.rgb,
    size=(CANVAS_H, CANVAS_W),
)
out = "assets-src/live2d/comet_live2d.psd"
with open(out, "wb") as f:
    psd.write(f)
print("saved", out)
