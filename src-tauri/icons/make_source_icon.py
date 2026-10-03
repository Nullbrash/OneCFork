"""Рисует исходную иконку 1024×1024 с надписью «1CF» без сторонних библиотек.

Из неё `npx tauri icon src-tauri/icons/source.png` генерирует все размеры.
"""

import struct
import zlib

SIZE = 1024
RADIUS = 180
BG = (31, 78, 121, 255)
FG = (255, 255, 255, 255)
CELL = 48
GLYPHS = {
    "1": ["..#..", ".##..", "#.#..", "..#..", "..#..", "..#..", "#####"],
    "C": [".####", "#....", "#....", "#....", "#....", "#....", ".####"],
    "F": ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
}
TEXT = "1CF"


def inside_rounded(x: int, y: int) -> bool:
    cx = min(max(x, RADIUS), SIZE - 1 - RADIUS)
    cy = min(max(y, RADIUS), SIZE - 1 - RADIUS)
    return (x - cx) ** 2 + (y - cy) ** 2 <= RADIUS**2


def main() -> None:
    cols = len(TEXT) * 5 + (len(TEXT) - 1)
    left = (SIZE - cols * CELL) // 2
    top = (SIZE - 7 * CELL) // 2
    lit = set()
    for i, ch in enumerate(TEXT):
        for row, line in enumerate(GLYPHS[ch]):
            for col, mark in enumerate(line):
                if mark == "#":
                    lit.add((i * 6 + col, row))

    raw = bytearray()
    for y in range(SIZE):
        raw.append(0)
        for x in range(SIZE):
            if not inside_rounded(x, y):
                raw += bytes((0, 0, 0, 0))
                continue
            gx, gy = (x - left) // CELL, (y - top) // CELL
            inside_text = 0 <= x - left and 0 <= y - top
            raw += bytes(FG if inside_text and (gx, gy) in lit else BG)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open("src-tauri/icons/source.png", "wb") as f:
        f.write(png)


if __name__ == "__main__":
    main()
