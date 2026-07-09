#!/usr/bin/env python3
"""Merge math-symbol glyphs from JuliaMono into another monospace font.

Most monospace fonts miss large parts of the Unicode math repertoire
(mathematical alphanumerics, big operators, bracket pieces …), and when a
terminal falls back to a proportional font the 2D layout of mascii output
breaks. JuliaMono (https://juliamono.netlify.app/) covers nearly all of it.

This script copies every glyph in the chosen math ranges that the base font
lacks from JuliaMono into a new font, uniformly rescaling each outline so
its advance width is exactly N terminal cells of the base font (N = 2 for
East-Asian-wide chars, else 1). The result stays metrics-compatible with
the base font, so alignment is preserved.

Requirements: pip install fonttools
Usage:
    python3 tools/merge_math_font.py BASE_FONT [-j JuliaMono-Regular.ttf]
        [-o output.ttf] [--font-number N] [--all-missing]

    BASE_FONT      .ttf/.ttc, TrueType outlines (glyf). For .ttc pick the
                   face with --font-number (default 0).
    -j/--julia     path to JuliaMono-Regular.ttf (download from the
                   JuliaMono releases page)
    --all-missing  copy every codepoint JuliaMono has that the base lacks,
                   not just the curated math ranges

Limitations: CFF/OTF base fonts are not supported (glyf outlines only);
JuliaMono's OpenType features (ligatures etc.) are not carried over —
only base glyphs, which is all a terminal grid needs.
"""

import argparse
import sys
import unicodedata
from collections import Counter

try:
    from fontTools.ttLib import TTFont
    from fontTools.pens.ttGlyphPen import TTGlyphPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.ttLib.tables._c_m_a_p import CmapSubtable
except ImportError:
    sys.exit("fontTools is required: pip install fonttools")

# Curated ranges: everything mascii's canonical AA form can emit, plus the
# broader math blocks so user symbols (\bbR, arrows, …) render too.
MATH_RANGES = [
    (0x00A8, 0x00AF),    # ¨ ¯ (accent marks)
    (0x02C6, 0x02DF),    # spacing modifier letters (ˇ ˘ ˙ ˚ ˜ …)
    (0x0370, 0x03FF),    # Greek
    (0x2010, 0x205F),    # general punctuation (‗ ‾ ′ …)
    (0x2070, 0x209F),    # superscripts and subscripts
    (0x20D0, 0x20FF),    # combining marks for symbols
    (0x2100, 0x214F),    # letterlike symbols (ℏ ℝ ℱ …)
    (0x2190, 0x21FF),    # arrows
    (0x2200, 0x22FF),    # mathematical operators
    (0x2300, 0x23FF),    # misc technical (⌠ ⎛ ⎡ …)
    (0x2460, 0x24FF),    # enclosed alphanumerics
    (0x2500, 0x257F),    # box drawing
    (0x2580, 0x259F),    # block elements (▌ …)
    (0x25A0, 0x25FF),    # geometric shapes (□ ▶ …)
    (0x2700, 0x27BF),    # dingbats (❯ …)
    (0x27C0, 0x27EF),    # misc mathematical symbols-A (⟍ ⟨ ⟩ …)
    (0x27F0, 0x27FF),    # supplemental arrows-A
    (0x2900, 0x297F),    # supplemental arrows-B
    (0x2980, 0x29FF),    # misc mathematical symbols-B (⦶ …)
    (0x2A00, 0x2AFF),    # supplemental mathematical operators
    (0x2B00, 0x2BFF),    # misc symbols and arrows (⬚ …)
    (0x1D400, 0x1D7FF),  # mathematical alphanumeric symbols (𝑥 𝐀 ℎ-block…)
]


def cells_wide(cp: int) -> int:
    """Terminal cells occupied by the char (wcwidth approximation)."""
    return 2 if unicodedata.east_asian_width(chr(cp)) in ("W", "F") else 1


def mono_advance(font: TTFont) -> int:
    """Most common advance width over printable ASCII = the cell width."""
    cmap = font.getBestCmap()
    hmtx = font["hmtx"]
    widths = Counter(
        hmtx[cmap[cp]][0] for cp in range(0x21, 0x7F) if cp in cmap
    )
    if not widths:
        sys.exit("base font has no ASCII glyphs; is this a text font?")
    return widths.most_common(1)[0][0]


def copy_glyph(src: TTFont, dst: TTFont, src_name: str, dst_name: str,
               scale: float, target_advance: int) -> None:
    """Draw the source glyph, uniformly scaled, into the destination font.

    Drawing through a pen flattens composite glyphs, so no component
    bookkeeping is needed.
    """
    src_glyphset = src.getGlyphSet()
    pen = TTGlyphPen(None)  # flatten: no component references
    src_glyphset[src_name].draw(TransformPen(pen, (scale, 0, 0, scale, 0, 0)))
    glyph = pen.glyph()
    dst["glyf"][dst_name] = glyph
    lsb = 0
    if glyph.numberOfContours:
        glyph.recalcBounds(dst["glyf"])
        lsb = glyph.xMin
    dst["hmtx"][dst_name] = (target_advance, lsb)


def rebuild_cmap(font: TTFont, mapping: dict) -> None:
    """Replace the cmap with format-4 (BMP) + format-12 (full) subtables."""
    cmap = font["cmap"]
    bmp = {cp: n for cp, n in mapping.items() if cp <= 0xFFFF}

    sub4 = CmapSubtable.getSubtableClass(4)(4)
    sub4.platformID, sub4.platEncID, sub4.language = 3, 1, 0
    sub4.cmap = bmp

    sub12 = CmapSubtable.getSubtableClass(12)(12)
    sub12.platformID, sub12.platEncID, sub12.language = 3, 10, 0
    sub12.format, sub12.reserved, sub12.length, sub12.nGroups = 12, 0, 0, 0
    sub12.cmap = dict(mapping)

    cmap.tableVersion = 0
    cmap.tables = [sub4, sub12]


def rename_font(font: TTFont, family: str) -> None:
    name = font["name"]
    ps = family.replace(" ", "")
    for nid, value in ((1, family), (3, f"{ps}:mascii-merged"), (4, family),
                       (6, ps), (16, family)):
        name.removeNames(nameID=nid)
        name.setName(value, nid, 3, 1, 0x409)  # windows
        name.setName(value, nid, 1, 0, 0)      # mac


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("base", help="base monospace font (.ttf/.ttc, glyf outlines)")
    ap.add_argument("-j", "--julia", default="JuliaMono-Regular.ttf",
                    help="JuliaMono TTF path (default: ./JuliaMono-Regular.ttf)")
    ap.add_argument("-o", "--output", default=None,
                    help="output path (default: <base stem>-Math.ttf)")
    ap.add_argument("--font-number", type=int, default=0,
                    help="face index inside a .ttc collection")
    ap.add_argument("--all-missing", action="store_true",
                    help="copy every codepoint the base font lacks")
    ap.add_argument("--family", default=None,
                    help="output family name (default: '<Base> Math')")
    args = ap.parse_args()

    base = TTFont(args.base, fontNumber=args.font_number
                  if args.base.lower().endswith(".ttc") else -1)
    julia = TTFont(args.julia)

    if "glyf" not in base:
        sys.exit("base font has no glyf table (CFF/OTF is not supported)")

    base_cmap = base.getBestCmap()
    julia_cmap = julia.getBestCmap()
    base_adv = mono_advance(base)
    julia_adv = mono_advance(julia)

    if args.all_missing:
        candidates = set(julia_cmap)
    else:
        candidates = {
            cp
            for lo, hi in MATH_RANGES
            for cp in range(lo, hi + 1)
            if cp in julia_cmap
        }
    todo = sorted(candidates - set(base_cmap))
    if not todo:
        sys.exit("nothing to merge: the base font already covers the ranges")

    existing = set(base.getGlyphOrder())
    mapping = dict(base_cmap)
    julia_hmtx = julia["hmtx"]

    added = 0
    for cp in todo:
        src_name = julia_cmap[cp]
        src_adv = julia_hmtx[src_name][0]
        if src_adv == 0:
            continue  # combining marks etc.: zero-width, skip
        target = cells_wide(cp) * base_adv
        scale = target / src_adv
        dst_name = f"u{cp:04X}"
        if dst_name in existing:
            dst_name = f"u{cp:04X}.jm"
        # glyf.__setitem__ also appends the name to the glyph order.
        copy_glyph(julia, base, src_name, dst_name, scale, target)
        existing.add(dst_name)
        mapping[cp] = dst_name
        added += 1

    base.setGlyphOrder(base["glyf"].glyphOrder)
    rebuild_cmap(base, mapping)
    # Glyph names are only cosmetic; format 3 avoids post-table bloat.
    base["post"].formatType = 3.0
    base["post"].glyphOrder = None

    stem = args.base.rsplit("/", 1)[-1].rsplit(".", 1)[0]
    family = args.family or f"{stem} Math"
    rename_font(base, family)

    out = args.output or f"{stem}-Math.ttf"
    base.save(out)
    print(f"added {added} glyphs from JuliaMono "
          f"(cell width {base_adv}, JuliaMono {julia_adv}) -> {out}")


if __name__ == "__main__":
    main()
