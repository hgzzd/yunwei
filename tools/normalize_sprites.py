#!/usr/bin/env python3
"""Normalize a four-cell generated strip into a 4 x 256px sprite atlas."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image


FRAME_SIZE = 256
INNER_SIZE = 232
ALPHA_CUTOFF = 8
LANCZOS = getattr(Image, "Resampling", Image).LANCZOS


def normalize(source: Path, destination: Path) -> None:
    sheet = Image.open(source).convert("RGBA")
    atlas = Image.new("RGBA", (FRAME_SIZE * 4, FRAME_SIZE), (0, 0, 0, 0))

    alpha = sheet.getchannel("A")
    column_counts = [
        sum(1 for value in alpha.crop((x, 0, x + 1, sheet.height)).getdata() if value > ALPHA_CUTOFF)
        for x in range(sheet.width)
    ]
    nominal_width = sheet.width / 4
    cuts = [0]
    for index in range(1, 4):
        target = round(index * nominal_width)
        radius = round(nominal_width * 0.32)
        candidates = range(max(cuts[-1] + 1, target - radius), min(sheet.width, target + radius + 1))
        cuts.append(min(candidates, key=lambda x: (column_counts[x], abs(x - target))))
    cuts.append(sheet.width)
    runs = list(zip(cuts, cuts[1:]))

    characters: list[Image.Image] = []
    for left, right in runs:
        cell = sheet.crop((left, 0, right, sheet.height))
        mask = cell.getchannel("A").point(lambda value: 255 if value > ALPHA_CUTOFF else 0)
        bounds = mask.getbbox()
        if bounds is None:
            raise ValueError(f"frame {len(characters)} in {source} has no visible pixels")
        characters.append(cell.crop(bounds))

    scale = min(
        INNER_SIZE / max(character.width for character in characters),
        INNER_SIZE / max(character.height for character in characters),
    )

    for index, character in enumerate(characters):
        size = (
            max(1, round(character.width * scale)),
            max(1, round(character.height * scale)),
        )
        character = character.resize(size, LANCZOS)
        x = index * FRAME_SIZE + (FRAME_SIZE - character.width) // 2
        y = FRAME_SIZE - 8 - character.height
        atlas.alpha_composite(character, (x, y))

    destination.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(destination, optimize=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    normalize(args.source, args.destination)


if __name__ == "__main__":
    main()
