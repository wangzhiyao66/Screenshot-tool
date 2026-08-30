#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""生成一张带中英文的测试图，用于验证系统 OCR 链路。"""
import pathlib
from PIL import Image, ImageDraw, ImageFont

OUT = pathlib.Path("/tmp/shotly-ocr-test.png")

def find_font(size: int, prefer=("PingFang.ttc", "Hiragino Sans GB.ttc", "Arial.ttf", "Helvetica.ttc")):
    import subprocess
    out = subprocess.run(
        ["find", "/System/Library/Fonts", "/Library/Fonts", "-name", "*.tt[cf]", "-maxdepth", "3"],
        capture_output=True, text=True,
    ).stdout.split("\n")
    out = [p for p in out if p.strip()]
    for name in prefer:
        for p in out:
            if p.lower().endswith(name.lower()):
                try:
                    return ImageFont.truetype(p, size)
                except Exception:
                    pass
    for p in out:
        try:
            return ImageFont.truetype(p, size)
        except Exception:
            continue
    return ImageFont.load_default()


def main():
    W, H = 900, 420
    img = Image.new("RGB", (W, H), "white")
    d = ImageDraw.Draw(img)

    f_big = find_font(40)
    f_mid = find_font(28)
    f_en = find_font(32)

    d.text((60, 50), "Shotly 截图工具", font=f_big, fill="#1f2328")
    d.text((60, 120), "识别文字 · 翻译文本 · 贴图 · 缩放", font=f_mid, fill="#d85a30")
    d.text((60, 180), "Lightweight screenshot tool for macOS and Windows", font=f_en, fill="#185fa5")
    d.text((60, 250), "第二行中文测试内容 ABCDEFG 12345", font=f_mid, fill="#1f2328")
    d.text((60, 320), "错误码 E0241: connection refused", font=f_en, fill="#a32d2d")

    d.rectangle([40, 40, W - 40, H - 40], outline="#e3e6ea", width=2)
    img.save(OUT)
    print("生成:", OUT, img.size)


if __name__ == "__main__":
    main()
