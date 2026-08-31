#!/usr/bin/env python3
"""Generate MD Previewer icons for desktop and mobile platforms."""

from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
IOS = ROOT / "mobile/ios/MDPreviewerMobile/Assets.xcassets/AppIcon.appiconset"
ANDROID = ROOT / "mobile/android/app/src/main/res"


def draw_icon(size: int, transparent: bool) -> Image.Image:
    scale = size / 1024
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0) if transparent else (15, 23, 42, 255))
    layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)

    margin = round(48 * scale) if transparent else 0
    box = (margin, margin, size - margin - 1, size - margin - 1)
    radius = round(220 * scale) if transparent else 0
    draw.rounded_rectangle(box, radius=radius, fill=(18, 32, 56, 255))
    if transparent:
        mask = Image.new("L", image.size, 0)
        ImageDraw.Draw(mask).rounded_rectangle(box, radius=radius, fill=255)
        layer.putalpha(mask)
    image.alpha_composite(layer)
    draw = ImageDraw.Draw(image)

    # A paper sheet with a folded corner.
    page = tuple(round(value * scale) for value in (250, 175, 774, 849))
    page_radius = round(56 * scale)
    draw.rounded_rectangle(page, radius=page_radius, fill=(248, 250, 252, 255))
    fold = [
        (round(625 * scale), round(175 * scale)),
        (round(774 * scale), round(324 * scale)),
        (round(625 * scale), round(324 * scale)),
    ]
    draw.polygon(fold, fill=(203, 213, 225, 255))
    draw.polygon(
        [fold[0], fold[1], (round(774 * scale), round(175 * scale))],
        fill=(45, 212, 191, 255),
    )

    # Two Markdown-like text lines.
    line_radius = max(1, round(12 * scale))
    draw.rounded_rectangle(
        tuple(round(value * scale) for value in (340, 360, 610, 386)),
        radius=line_radius,
        fill=(148, 163, 184, 255),
    )
    draw.rounded_rectangle(
        tuple(round(value * scale) for value in (340, 412, 680, 438)),
        radius=line_radius,
        fill=(203, 213, 225, 255),
    )

    # Preview eye: the product's distinct mark.
    eye = [
        (round(330 * scale), round(610 * scale)),
        (round(410 * scale), round(532 * scale)),
        (round(512 * scale), round(500 * scale)),
        (round(614 * scale), round(532 * scale)),
        (round(694 * scale), round(610 * scale)),
        (round(614 * scale), round(688 * scale)),
        (round(512 * scale), round(720 * scale)),
        (round(410 * scale), round(688 * scale)),
    ]
    draw.polygon(eye, fill=(20, 184, 166, 255))
    draw.ellipse(
        tuple(round(value * scale) for value in (430, 528, 594, 692)),
        fill=(240, 253, 250, 255),
    )
    draw.ellipse(
        tuple(round(value * scale) for value in (474, 572, 550, 648)),
        fill=(15, 23, 42, 255),
    )
    return image


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    desktop = draw_icon(1024, transparent=True)
    mobile = draw_icon(1024, transparent=False)

    desktop.save(ASSETS / "icon_1024.png")
    desktop.save(ASSETS / "icon.icns", format="ICNS")
    windows_sizes = [16, 32, 48, 64, 128, 256]
    windows_icons = [draw_icon(size, transparent=True) for size in windows_sizes]
    windows_icons[-1].save(
        ASSETS / "icon.ico",
        format="ICO",
        append_images=windows_icons[:-1],
        sizes=[(size, size) for size in windows_sizes],
    )

    ios_sizes = {
        "Icon-App-20x20@2x.png": 40,
        "Icon-App-20x20@3x.png": 60,
        "Icon-App-29x29@2x.png": 58,
        "Icon-App-29x29@3x.png": 87,
        "Icon-App-40x40@2x.png": 80,
        "Icon-App-40x40@3x.png": 120,
        "Icon-App-60x60@2x.png": 120,
        "Icon-App-60x60@3x.png": 180,
        "Icon-App-76x76@1x.png": 76,
        "Icon-App-76x76@2x.png": 152,
        "Icon-App-83.5x83.5@2x.png": 167,
        "Icon-App-1024x1024@1x.png": 1024,
    }
    IOS.mkdir(parents=True, exist_ok=True)
    for name, pixels in ios_sizes.items():
        mobile.resize((pixels, pixels), Image.Resampling.LANCZOS).convert("RGB").save(IOS / name)

    android_sizes = {
        "mipmap-mdpi": 48,
        "mipmap-hdpi": 72,
        "mipmap-xhdpi": 96,
        "mipmap-xxhdpi": 144,
        "mipmap-xxxhdpi": 192,
    }
    for directory, pixels in android_sizes.items():
        output = ANDROID / directory / "ic_launcher.png"
        output.parent.mkdir(parents=True, exist_ok=True)
        desktop.resize((pixels, pixels), Image.Resampling.LANCZOS).save(output)

    (ROOT / "docs").mkdir(exist_ok=True)
    desktop.save(ROOT / "docs/icon.png")
    print("Generated MD Previewer icons")


if __name__ == "__main__":
    main()
