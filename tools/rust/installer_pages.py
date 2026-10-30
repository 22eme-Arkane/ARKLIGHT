"""Les pages de l'installeur Windows de la version Rust (rust/installer/arklight.iss), dans le
style sombre de celui d'AIODOS, aux couleurs d'ARKLIGHT (le logo dans un halo rose et orange, les
couleurs de son dégradé ; approuvé par l'auteur le 27/10/2026) : la charte et les conditions
d'utilisation à accepter, en RTF, et les images de l'assistant, tirées du logo.

Demandé par l'auteur le 27/10/2026 : un installeur comme celui d'AIODOS, avec le logo, la
présentation d'ARKLIGHT (dans arklight.iss, [Messages], WelcomeLabel2), la charte et les
conditions d'utilisation (rust/installer/legal/<langue>/).

    python tools/rust/installer_pages.py <dossier de sortie>
    → legal-fr.rtf, legal-en.rtf, wizard-*.png, small-*.png ; affiche les chemins en JSON
"""
import json
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[2]
LEGAL = ROOT / "rust" / "installer" / "legal"
LOGO = ROOT / "build" / "icon.png"
LANGUAGES = ("fr", "en")
DOCS = ("charter", "terms")
# Grande image (accueil, fin) et petite (en haut à droite des autres pages), de 100 à 250 % :
# celles parmi lesquelles Inno Setup 6.6 et plus choisit
LARGE = ((202, 386), (269, 515), (336, 643), (403, 772), (430, 824), (498, 953), (534, 1022))
SMALL = (58, 77, 97, 116, 124, 143, 159)
# Couleurs d'ARKLIGHT (rust/crates/arklight-desktop/ui/app.slint) et du logo (dégradé rose → orange)
BG = (12, 14, 17)
PANEL = (20, 23, 28)
ORANGE = (255, 173, 59)
PINK = (255, 79, 129)
TEXT = (242, 244, 247)
FONT = Path(r"C:\Windows\Fonts\seguisb.ttf")          # Segoe UI Semibold

_BOLD = re.compile(r"\*\*(.+?)\*\*")
_ITALIC = re.compile(r"\*(.+?)\*")


# --- La charte et les conditions, en RTF (ce qu'affiche la page de licence d'Inno Setup) ---

def escape(text: str) -> str:
    """Du texte en RTF : \\, { et } échappés, tout caractère hors ASCII en \\uN? (en paire de
    substitution au-delà du plan de base). Le fichier ne contient que de l'ASCII."""
    out = []
    for ch in text:
        code = ord(ch)
        if ch in "\\{}":
            out.append("\\" + ch)
        elif code < 128:
            out.append(ch)
        else:
            units = [code] if code < 0x10000 else [0xD800 + ((code - 0x10000) >> 10), 0xDC00 + ((code - 0x10000) & 0x3FF)]
            out.extend(f"\\u{u - 65536 if u > 32767 else u}?" for u in units)
    return "".join(out)


def inline(text: str) -> str:
    """**gras** et *italique*, les seuls styles des textes."""
    text = _BOLD.sub(r"{\\b \1}", escape(text))
    return _ITALIC.sub(r"{\\i \1}", text)


def blocks(markdown: str) -> list:
    """Les blocs du Markdown : titres, paragraphes, éléments de liste (lignes repliées rejointes)."""
    out, current = [], None
    for line in markdown.splitlines():
        text = line.strip()
        if not text:
            current = None
        elif text.startswith("#"):
            level = len(text) - len(text.lstrip("#"))
            out.append([f"h{level}", text[level:].strip()])
            current = None
        elif text.startswith("- "):
            current = ["li", text[2:].strip()]
            out.append(current)
        elif current is not None:
            current[1] += " " + text
        else:
            current = ["p", text]
            out.append(current)
    return [tuple(b) for b in out]


def rtf(texts: list) -> str:
    """Les textes, l'un après l'autre, en un document RTF : titres en orange ARKLIGHT, puces,
    Segoe UI 9 pt."""
    r, g, b = ORANGE
    parts = ["{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1"
             "{\\fonttbl{\\f0\\fswiss\\fcharset0 Segoe UI;}}"
             f"{{\\colortbl ;\\red{r}\\green{g}\\blue{b};}}"
             "\\viewkind4\\f0\\fs18\n"]
    for i, markdown in enumerate(texts):
        for kind, text in blocks(markdown):
            body = inline(text)
            if kind == "h1":
                before = "\\sb480" if i else "\\sb0"
                parts.append(f"\\pard{before}\\sa160\\cf1\\b\\fs28 {body}\\b0\\fs18\\cf0\\par\n")
            elif kind in ("h2", "h3"):
                parts.append(f"\\pard\\sb200\\sa80\\b\\fs20 {body}\\b0\\fs18\\par\n")
            elif kind == "li":
                parts.append(f"\\pard\\fi-220\\li440\\sa60\\tx440 \\u8226?\\tab {body}\\par\n")
            else:
                parts.append(f"\\pard\\sa120 {body}\\par\n")
    parts.append("}\n")
    return "".join(parts)


# --- Les images de l'assistant ---

def _font(size: int):
    try:
        return ImageFont.truetype(str(FONT), size)
    except OSError:
        return ImageFont.load_default(size)


def _spaced(draw, center: float, top: float, text: str, font, spacing: float, fill):
    """Un mot aux lettres espacées, centré sur `center`."""
    widths = [font.getlength(ch) for ch in text]
    x = center - (sum(widths) + spacing * (len(text) - 1)) / 2
    baseline = top + font.getmetrics()[0]          # même ligne de base pour toutes les lettres
    for ch, width in zip(text, widths):
        draw.text((x, baseline), ch, font=font, fill=fill, anchor="ls")
        x += width + spacing


def large(logo: Image.Image, size: tuple) -> Image.Image:
    """Grande image : le logo dans un halo orange et rose (les couleurs de son dégradé), sur le
    fond sombre d'ARKLIGHT, son nom dessous et un faisceau de lumière qui descend."""
    w, h = size
    shade = Image.linear_gradient("L").resize(size)              # noir en haut, clair en bas
    image = Image.composite(Image.new("RGB", size, BG), Image.new("RGB", size, PANEL), shade)
    cx, cy = w / 2, h * 0.38
    # faisceau : un cône orange très doux sous le logo, comme un projecteur dans la brume
    beam = Image.new("L", size, 0)
    ImageDraw.Draw(beam).polygon([(cx - w * 0.08, cy), (cx + w * 0.08, cy), (cx + w * 0.62, h), (cx - w * 0.62, h)], fill=46)
    beam = beam.filter(ImageFilter.GaussianBlur(w * 0.08))
    image = Image.composite(Image.new("RGB", size, ORANGE), image, beam)
    # halo : rose en haut, orange en bas
    halo_color = Image.composite(Image.new("RGB", size, ORANGE), Image.new("RGB", size, PINK), Image.linear_gradient("L").resize(size))
    glow = Image.new("L", size, 0)
    r = w * 0.46
    ImageDraw.Draw(glow).ellipse((cx - r, cy - r, cx + r, cy + r), fill=96)
    glow = glow.filter(ImageFilter.GaussianBlur(w * 0.14))
    image = Image.composite(halo_color, image, glow)
    side = round(w * 0.56)
    emblem = logo.resize((side, side), Image.LANCZOS)
    image.paste(emblem, (round(cx - side / 2), round(cy - side / 2)), emblem)
    draw = ImageDraw.Draw(image)
    _spaced(draw, cx, cy + side / 2 + w * 0.10, "ARKLIGHT", _font(round(w * 0.105)), w * 0.018, TEXT)
    _spaced(draw, cx, cy + side / 2 + w * 0.25, "22eme ARKANE", _font(round(w * 0.045)), w * 0.012, ORANGE)
    return image


def small(logo: Image.Image, side: int) -> Image.Image:
    """Petite image : le logo seul, sur fond transparent."""
    return logo.resize((side, side), Image.LANCZOS)


def prepare(out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    legal = {}
    for lang in LANGUAGES:
        texts = [(LEGAL / lang / f"{doc}.md").read_text(encoding="utf-8") for doc in DOCS]
        legal[lang] = out / f"legal-{lang}.rtf"
        legal[lang].write_text(rtf(texts), encoding="ascii", newline="\n")
    emblem = Image.open(LOGO).convert("RGBA")
    larges, smalls = [], []
    for size in LARGE:
        larges.append(out / f"wizard-{size[0]}.png")
        large(emblem, size).save(larges[-1])
    for side in SMALL:
        smalls.append(out / f"small-{side}.png")
        small(emblem, side).save(smalls[-1])
    return {"legal": {k: str(v) for k, v in legal.items()}, "large": [str(p) for p in larges], "small": [str(p) for p in smalls]}


if __name__ == "__main__":
    print(json.dumps(prepare(Path(sys.argv[1]))))
