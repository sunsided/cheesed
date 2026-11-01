#!/usr/bin/env python3
"""Convert a painting into one of the game's 256-color, VGA-style full-screen pictures.

    python3 tools/cover.py art/cover.png src/render/cover.bin

`task cover` runs this for every painting in art/ (the title cover, the episode intros, the
victory, defeat, onward and farewell cards). The source paintings are kept out of git; the
.bin outputs are committed and embedded by src/render/picture.rs with include_bytes!.

Steps:

1. Crop: a picture wider than the 320x200 screen's 16:10 is cropped evenly on both sides
   to exactly 16:10 (a tolerance of 0.01 on the width/height ratio avoids shaving a column
   off an image that is already 16:10 up to rounding). Narrower pictures are left alone.
2. Resize to exactly 200 rows (the framebuffer height), keeping the aspect ratio, with
   Lanczos filtering. The width becomes 320 for a 16:10 source; a narrower source comes
   out narrower than 320.
3. Reduce to at most 256 colors: Pillow's median-cut quantizer picks the palette (it
   repeatedly splits the box of colors in RGB space at the median of its longest axis,
   so busy colour regions get more palette entries), and Floyd-Steinberg error diffusion
   spreads each pixel's rounding error onto its right and lower neighbours, so smooth
   gradients turn into fine dither instead of hard bands. The result looks like a
   256-color VGA picture.

Output format, a tiny raw format the game embeds with include_bytes!:

    u16 width, u16 height (little endian), u8 palette size - 1,
    palette (size * RGB), then width * height palette indices.

The indices are one byte per pixel, row-major from the top-left. The palette size is
stored minus one so that 256 fits in a byte.

A 3x nearest-neighbour preview is written next to the output as <dst>.preview.png, for
checking the result by eye.
"""
import struct
import sys

from PIL import Image

# Target height in pixels: the full height of the game's 320x200 framebuffer.
H = 200
# Palette size: the 256 colors of VGA mode 13h.
COLORS = 256


def main(src, dst):
    """Convert the painting at path `src` and write the .bin file to path `dst`."""
    im = Image.open(src).convert("RGB")
    # Wider than the 16:10 screen (say 16:9): crop the sides evenly.
    target = 320 / 200
    if im.width / im.height > target + 0.01:
        w = round(im.height * target)
        x0 = (im.width - w) // 2
        im = im.crop((x0, 0, x0 + w, im.height))
    # Fit to 200 rows; the width follows from the (possibly cropped) aspect ratio.
    w = round(im.width * H / im.height)
    im = im.resize((w, H), Image.LANCZOS)
    # Median-cut palette plus Floyd-Steinberg dithering; `q` is a palette ("P" mode) image
    # whose pixel values are palette indices.
    q = im.quantize(colors=COLORS, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG)
    # getpalette() returns a flat [r, g, b, r, g, b, ...] list; keep at most 256 entries.
    pal = q.getpalette()[: COLORS * 3]
    n = len(pal) // 3
    with open(dst, "wb") as f:
        # Header: width, height as little-endian u16, then palette size minus one as u8.
        f.write(struct.pack("<HHB", w, H, n - 1))
        f.write(bytes(pal))
        # One index byte per pixel, row by row.
        f.write(q.tobytes())
    # A 3x blown-up preview with hard pixel edges, to eyeball the dithering.
    q.convert("RGB").resize((w * 3, H * 3), Image.NEAREST).save(dst + ".preview.png")
    print(f"{dst}: {w}x{H}, {n} colors")


if __name__ == "__main__":
    main(*sys.argv[1:3])
