#!/usr/bin/env python3
"""Generate MD Previewer icons for desktop and mobile platforms."""

from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
SOURCE = ASSETS / "icon_source.png"
IOS = ROOT / "mobile/ios/MDPreviewerMobile/Assets.xcassets/AppIcon.appiconset"
ANDROID = ROOT / "mobile/android/app/src/main/res"


def load_icon(size: int) -> Image.Image:
    with Image.open(SOURCE) as source:
        return source.convert("RGBA").resize((size, size), Image.Resampling.LANCZOS)


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    icon = load_icon(1024)

    icon.save(ASSETS / "icon_1024.png")
    icon.save(ASSETS / "icon.icns", format="ICNS")
    windows_sizes = [16, 32, 48, 64, 128, 256]
    windows_icons = [load_icon(size) for size in windows_sizes]
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
        load_icon(pixels).save(IOS / name)

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
        load_icon(pixels).save(output)

    (ROOT / "docs").mkdir(exist_ok=True)
    icon.save(ROOT / "docs/icon.png")
    print("Generated MD Previewer icons")


if __name__ == "__main__":
    main()
