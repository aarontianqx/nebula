#!/usr/bin/env python3
"""Generate a deterministic preview of the programmatic idle blink.

The crop ratios mirror PetCanvas. Outputs are review artifacts under
`/tmp/comet_preview`: `idle_programmatic.gif` and `idle_programmatic_sheet.png`.
"""

from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
FRAMES = ROOT / "src/assets/pet/frames"
OUT = Path("/tmp/comet_preview")
EYE_PATCH = (0.17, 0.19, 0.66, 0.23)


def render(eyes_closed: bool) -> Image.Image:
    base = Image.open(FRAMES / "idle_0.png").convert("RGBA")
    if not eyes_closed:
        return base
    closed = Image.open(FRAMES / "idle_1.png").convert("RGBA")
    width, height = base.size
    x = round(width * EYE_PATCH[0])
    y = round(height * EYE_PATCH[1])
    patch_width = round(width * EYE_PATCH[2])
    patch_height = round(height * EYE_PATCH[3])
    box = (x, y, x + patch_width, y + patch_height)
    base.alpha_composite(closed.crop(box), (x, y))
    return base


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    opened = render(False)
    closed = render(True)

    # Three realistic blink cycles, including long holds for visual inspection.
    frames = [opened] * 18 + [closed] + [opened] * 25 + [closed] + [opened] * 20
    frames[0].save(
        OUT / "idle_programmatic.gif",
        save_all=True,
        append_images=frames[1:],
        duration=120,
        loop=0,
        disposal=2,
    )

    sheet = Image.new("RGBA", (opened.width * 2, opened.height), (235, 235, 235, 255))
    sheet.alpha_composite(opened, (0, 0))
    sheet.alpha_composite(closed, (opened.width, 0))
    draw = ImageDraw.Draw(sheet)
    draw.text((4, 4), "open", fill=(0, 0, 0, 255))
    draw.text((opened.width + 4, 4), "blink", fill=(0, 0, 0, 255))
    sheet.save(OUT / "idle_programmatic_sheet.png")

    diff = ImageChops.difference(opened, closed)
    changed = sum(1 for pixel in diff.getdata() if any(pixel))
    ratio = changed / (opened.width * opened.height)
    print(f"changed pixels: {changed} ({ratio:.2%} of frame)")
    if ratio > 0.18:
        raise SystemExit("blink changes too much of the character")


if __name__ == "__main__":
    main()
