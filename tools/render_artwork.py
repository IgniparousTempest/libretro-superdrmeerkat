#!/usr/bin/env python3
"""Render the scraper artwork with librsvg and Pillow."""
import base64
import io
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "assets/source"
ART = SOURCE / "artwork"
OUTPUT = ROOT / ".artwork"
SVG = "http://www.w3.org/2000/svg"
XLINK = "{http://www.w3.org/1999/xlink}href"


def embed_images(path, prefix=""):
    # Keep the source SVGs editable with ordinary relative links. At export,
    # inline vectors so even a small game sprite renders at print resolution.
    root = ET.parse(path).getroot()
    for element in root.iter():
        if "id" in element.attrib:
            element.set("id", prefix + element.get("id"))
        for key, value in element.attrib.items():
            if key in (XLINK, "href") and value.startswith("#"):
                element.set(key, "#" + prefix + value[1:])
            elif "url(#" in value:
                element.set(key, value.replace("url(#", "url(#" + prefix))

    images = [(parent, i, child) for parent in root.iter()
              for i, child in enumerate(parent) if child.tag == f"{{{SVG}}}image"]
    for n, (parent, index, image) in enumerate(images):
        source = path.parent / image.attrib.pop(XLINK)
        if source.suffix == ".png":
            image.set(XLINK, "data:image/png;base64," + base64.b64encode(source.read_bytes()).decode("ascii"))
            continue
        nested = embed_images(source, f"{prefix}svg{n}-")
        for key in ("x", "y", "width", "height", "preserveAspectRatio"):
            if key in image.attrib:
                nested.set(key, image.attrib.pop(key))
        group = ET.Element(f"{{{SVG}}}g", image.attrib)
        group.append(nested)
        parent[index] = group
    return root


def raster(path, env):
    svg = ET.tostring(embed_images(path), encoding="utf-8")
    png = subprocess.run(["rsvg-convert"], input=svg, env=env,
                         stdout=subprocess.PIPE, check=True).stdout
    return Image.open(io.BytesIO(png)).convert("RGBA")


def main():
    OUTPUT.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory() as directory:
        config = ET.Element("fontconfig")
        ET.SubElement(config, "dir").text = str(SOURCE / "fonts")
        ET.SubElement(config, "cachedir").text = directory
        config_path = Path(directory) / "fonts.conf"
        ET.ElementTree(config).write(config_path, encoding="utf-8")
        env = dict(os.environ, FONTCONFIG_FILE=str(config_path))

        raster(ART / "cover.svg", env).convert("RGB").save(OUTPUT / "cover.png")
        wrap = raster(ART / "box.svg", env).convert("RGB")
        wrap.save(OUTPUT / "box-texture.png")
        left = 0
        for panel, width in [("back", 2056), ("spine", 240), ("front", 2064)]:
            wrap.crop((left, 0, left + width, wrap.height)).save(OUTPUT / f"box-{panel}.png")
            left += width
        raster(ART / "box-3d.svg", env).save(OUTPUT / "box-3d.png")

        for source, name in [(SOURCE / "wordmark.svg", "logo"), (ART / "credit.svg", "credit-courtney-pitcher")]:
            image = raster(source, env)
            image.crop(image.getbbox()).save(OUTPUT / f"{name}.png")
        shutil.copyfile(ART / "meerkat-icon.svg", OUTPUT / "meerkat-icon.svg")


if __name__ == "__main__":
    main()
