#!/usr/bin/env python3
"""Build the sprite and font atlas from assets/source/."""
import io
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1] / "assets"
SOURCE = ROOT / "source"
OUTPUT = ROOT / "raster"
PAGE = 2048  # Maximum texture width or height on the Pi 2 and 3's VideoCore IV.
PAD = 8  # Leave transparent gutters for the atlas's minified mip levels.
FONTS = [("Barlow-SemiBold", 56), ("BarlowCondensed-Bold", 88)]


def raster(name):
    data = subprocess.check_output(["rsvg-convert", str(SOURCE / (name + ".svg"))])
    return Image.open(io.BytesIO(data)).convert("RGBA")


def items():
    yield "s white", Image.new("RGBA", (4, 4), "white"), ""
    for name in ["doctor", "secretary", "savanna", "wordmark", "clinic-mark"]:
        yield "s " + name, raster(name), ""
    controls = raster("cabinet-controls")
    size = controls.height
    for i, name in enumerate(["button-a", "button-b", "button-c", "button-d",
                              "joystick", "coin", "button-start"]):
        yield "s " + name, controls.crop((i * size, 0, (i + 1) * size, size)), ""
    pieces = raster("pieces")
    step = pieces.width // 3
    for y, kind in enumerate(["germ", "end", "single"]):
        for x, color in enumerate(["red", "blue", "gold"]):
            tile = pieces.crop((x * step, y * step, (x + 1) * step, (y + 1) * step))
            yield f"s {kind}-{color}", tile, ""
    for family, (name, size) in enumerate(FONTS):
        font = ImageFont.truetype(str(SOURCE / "fonts" / (name + ".ttf")), size)
        for code in range(32, 127):
            ch = chr(code)
            left, top, right, bottom = font.getbbox(ch, anchor="ls")
            image = Image.new("RGBA", (max(1, right - left), max(1, bottom - top)))
            ImageDraw.Draw(image).text((-left, -top), ch, font=font, anchor="ls", fill="white")
            yield f"g {family} {code}", image, f" {left} {top} {font.getlength(ch):.4f}"


def main():
    OUTPUT.mkdir(exist_ok=True)
    sheet = Image.new("RGBA", (PAGE, PAGE))
    heights = [0] * PAGE
    index = [f"font {family} {size}\n" for family, (_, size) in enumerate(FONTS)]
    for key, image, metrics in sorted(items(), key=lambda item: -item[1].height):
        w, h = image.size
        span = w + 2 * PAD
        # Place the next image on the lowest available ledge.
        starts = (i for i in range(PAGE - span + 1) if i == 0 or heights[i] != heights[i - 1])
        top, left = min((max(heights[i:i + span]), i) for i in starts)
        bottom = top + h + 2 * PAD
        if bottom > PAGE:
            raise ValueError("Sprites exceed the GLES2 atlas budget")
        heights[left:left + span] = [bottom] * span
        x, y = left + PAD, top + PAD
        sheet.paste(image, (x, y))
        index.append(f"{key} {x} {y} {w} {h}{metrics}\n")
    used = max(heights)
    height = 1 << (used - 1).bit_length()
    sheet.crop((0, 0, PAGE, height)).save(OUTPUT / "atlas.png", optimize=True)
    (OUTPUT / "atlas.txt").write_text("".join(index), encoding="ascii")
    print(f"Baked {len(index) - len(FONTS)} regions into {PAGE} x {height}")


if __name__ == "__main__":
    main()
