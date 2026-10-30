"""Images de présentation sur GitHub (assets/) : le logo et son nom dessous, comme dans
l'installeur (tools/rust/installer_pages.py).

  python tools/rust/github_assets.py

social-preview.png (1280 × 640, le format de GitHub) : à déposer dans Settings › General › Social
preview du dépôt. banner.png : en tête du README.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

from installer_pages import BG, LOGO, ORANGE, PANEL, PINK, TEXT, _font, _spaced

OUT = Path(__file__).resolve().parents[2] / "assets"


def card(logo: Image.Image, size: tuple, side: int) -> Image.Image:
    """Le logo dans un halo rose et orange sur le fond sombre d'ARKLIGHT, un faisceau qui descend,
    le nom dessous et « 22eme ARKANE » : la composition de l'installeur, en large."""
    w, h = size
    shade = Image.linear_gradient("L").resize(size)
    image = Image.composite(Image.new("RGB", size, BG), Image.new("RGB", size, PANEL), shade)
    name, by = round(side * 0.36), round(side * 0.13)
    total = side + round(side * 0.13) + name + round(side * 0.08) + by
    top = (h - total) / 2
    cx, cy = w / 2, top + side / 2
    beam = Image.new("L", size, 0)
    ImageDraw.Draw(beam).polygon([(cx - side * 0.3, cy), (cx + side * 0.3, cy), (cx + side * 1.9, h), (cx - side * 1.9, h)], fill=46)
    beam = beam.filter(ImageFilter.GaussianBlur(side * 0.28))
    image = Image.composite(Image.new("RGB", size, ORANGE), image, beam)
    color = Image.composite(Image.new("RGB", size, ORANGE), Image.new("RGB", size, PINK), Image.linear_gradient("L").resize(size))
    glow = Image.new("L", size, 0)
    r = side * 0.85
    ImageDraw.Draw(glow).ellipse((cx - r, cy - r, cx + r, cy + r), fill=96)
    glow = glow.filter(ImageFilter.GaussianBlur(side * 0.42))
    image = Image.composite(color, image, glow)
    emblem = logo.resize((side, side), Image.LANCZOS)
    image.paste(emblem, (round(cx - side / 2), round(cy - side / 2)), emblem)
    draw = ImageDraw.Draw(image)
    y = cy + side / 2 + side * 0.10
    _spaced(draw, cx, y, "ARKLIGHT", _font(name), side * 0.06, TEXT)
    _spaced(draw, cx, y + name * 1.28, "22eme ARKANE", _font(by), side * 0.04, ORANGE)
    return image


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    emblem = Image.open(LOGO).convert("RGBA")
    card(emblem, (1280, 640), 290).save(OUT / "social-preview.png", optimize=True)
    card(emblem, (1280, 480), 215).save(OUT / "banner.png", optimize=True)
    print(OUT)
