#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""生成 Tauri 需要的 PNG 图标（纯标准库，无第三方依赖）。"""
import pathlib
import struct
import zlib

OUT = pathlib.Path(__file__).parent / "src-tauri" / "icons"
OUT.mkdir(parents=True, exist_ok=True)

SIZE = 512
BG = (47, 50, 55, 255)        # 深石板灰
FG = (255, 255, 255, 255)     # 白色裁剪框


def in_rounded_rect(px, py, x0, y0, x1, y1, r):
    if px < x0 or px > x1 or py < y0 or py > y1:
        return False
    cx = min(max(px, x0 + r), x1 - r)
    cy = min(max(py, y0 + r), y1 - r)
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r


def in_rect(px, py, x0, y0, x1, y1):
    return x0 <= px <= x1 and y0 <= py <= y1


def corner_mark(px, py, cx, cy, sx, sy, length, thick):
    """以 (cx, cy) 为拐点的 L 形角标；sx/sy 是向内的方向（+1 / -1）。"""
    if in_rect(px, py, min(cx, cx + sx * length), max(cx, cx + sx * length),
               cy - thick // 2, cy + thick // 2):
        return True
    if in_rect(px, py, cx - thick // 2, cx + thick // 2,
               min(cy, cy + sy * length), max(cy, cy + sy * length)):
        return True
    return False


def build(size=SIZE):
    s = size / SIZE
    buf = bytearray()
    r = int(110 * s)
    inset = int(120 * s)
    length = int(96 * s)
    thick = int(20 * s)
    for y in range(size):
        buf.append(0)  # filter type 0
        for x in range(size):
            if not in_rounded_rect(x, y, 0, 0, size - 1, size - 1, r):
                buf.extend((0, 0, 0, 0))
                continue
            if (corner_mark(x, y, inset, inset, 1, 1, length, thick)
                    or corner_mark(x, y, size - inset, inset, -1, 1, length, thick)
                    or corner_mark(x, y, inset, size - inset, 1, -1, length, thick)
                    or corner_mark(x, y, size - inset, size - inset, -1, -1, length, thick)):
                buf.extend(FG)
            else:
                buf.extend(BG)
    return bytes(buf)


def png(path, raw, size):
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    data = (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b""))
    path.write_bytes(data)
    print(f"{path}  {len(data)/1024:.1f} KB")


raw512 = build(512)
png(OUT / "icon.png", raw512, 512)
png(OUT / "128x128.png", build(128), 128)
png(OUT / "128x128@2x.png", build(256), 256)
png(OUT / "32x32.png", build(32), 32)

# Windows 打包需要 icon.ico（tauri-build 用它生成资源文件）
try:
    from PIL import Image

    src = Image.open(OUT / "icon.png").convert("RGBA")
    src.save(
        OUT / "icon.ico",
        format="ICO",
        sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)],
    )
    print(f"{OUT / 'icon.ico'}  {(OUT / 'icon.ico').stat().st_size / 1024:.1f} KB")
except ImportError:
    print("提示：未安装 Pillow，跳过 icon.ico（Windows 打包需要，请先 pip install pillow）")

print("icons ready ->", OUT)
