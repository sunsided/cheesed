#!/usr/bin/env python3
"""Convert the cover painting into the title screen's 256-color VGA-style image.

    python3 tools/cover.py art/cover.png src/render/cover.bin

Crops a picture wider than 16:10 to 16:10, fits it to 200 rows (keeping its aspect), reduces it to a 256-color palette with
Floyd-Steinberg dithering, and writes a tiny raw format the game embeds with include_bytes!:

    u16 width, u16 height (little endian), u8 palette size - 1,
    palette (size * RGB), then width * height palette indices.
"""
import struct
import sys

from PIL import Image

H = 200
COLORS = 256


def main(src, dst):
    im = Image.open(src).convert("RGB")
    # Wider than the 16:10 screen (say 16:9): crop the sides evenly.
    target = 320 / 200
    if im.width / im.height > target + 0.01:
        w = round(im.height * target)
        x0 = (im.width - w) // 2
        im = im.crop((x0, 0, x0 + w, im.height))
    w = round(im.width * H / im.height)
    im = im.resize((w, H), Image.LANCZOS)
    q = im.quantize(colors=COLORS, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG)
    pal = q.getpalette()[: COLORS * 3]
    n = len(pal) // 3
    with open(dst, "wb") as f:
        f.write(struct.pack("<HHB", w, H, n - 1))
        f.write(bytes(pal))
        f.write(q.tobytes())
    q.convert("RGB").resize((w * 3, H * 3), Image.NEAREST).save(dst + ".preview.png")
    print(f"{dst}: {w}x{H}, {n} colors")


if __name__ == "__main__":
    main(*sys.argv[1:3])
