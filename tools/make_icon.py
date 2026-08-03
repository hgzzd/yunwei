#!/usr/bin/env python3
"""Extract the front Cloudtail pose into a square transparent app icon source."""

from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "art/concept/cloudtail-reference.png"
DESTINATION = ROOT / "art/concept/cloudtail-icon-source.png"
LANCZOS = getattr(Image, "Resampling", Image).LANCZOS


def main() -> None:
    source = Image.open(SOURCE).convert("RGBA")
    # The first turnaround pose occupies the upper-left model-sheet cell.
    # Keep the crop inside that cell so neighboring poses/expressions cannot
    # leak into the generated Windows icon.
    subject = source.crop((0, 0, 520, 560))
    alpha = subject.getchannel("A").point(lambda value: 255 if value > 8 else 0)
    bounds = alpha.getbbox()
    if bounds is None:
        raise RuntimeError("front pose was not found in the concept sheet")
    subject = subject.crop(bounds)
    scale = min(840 / subject.width, 840 / subject.height)
    subject = subject.resize(
        (round(subject.width * scale), round(subject.height * scale)), LANCZOS
    )
    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    canvas.alpha_composite(
        subject,
        ((canvas.width - subject.width) // 2, (canvas.height - subject.height) // 2),
    )
    canvas.save(DESTINATION, optimize=True)


if __name__ == "__main__":
    main()
