#!/usr/bin/env python3
"""Write the small .aseprite files the repository ships.

Aseprite itself is not needed: the files are written byte by byte following
https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md
(RGBA color depth, one layer, raw cels, one tags chunk).

    python3 scripts/gen_aseprite_fixtures.py

writes
  crates/amigo_assets/tests/fixtures/strip.aseprite   (loader tests)
  examples/animation_demo/assets/sprites/hero.aseprite (the demo's character)
"""

import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

FORWARD, REVERSE, PING_PONG = 0, 1, 2


def string(s):
    data = s.encode("utf-8")
    return struct.pack("<H", len(data)) + data


def chunk(kind, payload):
    return struct.pack("<IH", 6 + len(payload), kind) + payload


def layer_chunk(name):
    # flags=visible, normal layer, child level 0, default size, blend normal,
    # opacity 255, 3 reserved bytes, name
    return chunk(
        0x2004,
        struct.pack("<HHHHHHB3x", 1, 0, 0, 0, 0, 0, 255) + string(name),
    )


def cel_chunk(width, height, pixels):
    # layer 0 at (0, 0), opacity 255, cel type 0 (raw), z-index 0, 5 reserved
    header = struct.pack("<HhhBHh5x", 0, 0, 0, 255, 0, 0)
    return chunk(0x2005, header + struct.pack("<HH", width, height) + pixels)


def tags_chunk(tags):
    body = struct.pack("<H8x", len(tags))
    for name, start, end, direction, repeat in tags:
        # from, to, direction, repeat, 6 reserved, RGB + extra byte, name
        body += struct.pack("<HHBH6x4x", start, end, direction, repeat) + string(name)
    return chunk(0x2018, body)


def frame(duration_ms, chunks):
    data = b"".join(chunks)
    header = struct.pack("<IHHHxxI", 16 + len(data), 0xF1FA, len(chunks), duration_ms, len(chunks))
    return header + data


def aseprite(width, height, frames, tags):
    """frames: list of (duration_ms, rgba bytes); tags go into the first frame."""
    body = b""
    for i, (duration, pixels) in enumerate(frames):
        chunks = []
        if i == 0:
            chunks.append(layer_chunk("Layer 1"))
            if tags:
                chunks.append(tags_chunk(tags))
        chunks.append(cel_chunk(width, height, pixels))
        body += frame(duration, chunks)
    header = struct.pack(
        "<IHHHHHIHIIB3xHBBhhHH84x",
        128 + len(body),  # file size
        0xA5E0,           # magic
        len(frames),
        width,
        height,
        32,               # RGBA
        1,                # flags
        100,              # deprecated speed
        0,
        0,
        0,                # transparent index
        0,                # colors
        1,
        1,                # pixel ratio 1:1
        0,
        0,
        16,
        16,               # grid
    )
    assert len(header) == 128
    return header + body


def solid(width, height, rgba):
    return bytes(rgba) * (width * height)


def strip_fixture():
    # Four 4x2 frames of solid colour, so a test can tell frames apart by
    # sampling one pixel of the composed strip.
    colours = [(255, 0, 0, 255), (0, 255, 0, 255), (0, 0, 255, 255), (255, 255, 255, 255)]
    durations = [100, 100, 200, 50]
    frames = [(d, solid(4, 2, c)) for d, c in zip(durations, colours)]
    tags = [
        ("walk", 0, 2, FORWARD, 0),
        ("back", 1, 3, REVERSE, 0),
        ("bounce", 0, 2, PING_PONG, 0),
        ("twice", 3, 3, FORWARD, 2),
        ("overflow", 2, 9, FORWARD, 0),  # `to` past the last frame
        ("missing", 7, 9, FORWARD, 0),   # starts past the last frame
    ]
    return aseprite(4, 2, frames, tags)


def hero():
    # A 16x16 figure: head, body in the state's colour, legs that move.
    w = h = 16
    skin = (240, 200, 160, 255)
    clear = (0, 0, 0, 0)
    dark = (40, 40, 60, 255)

    def figure(body, left_leg, right_leg, lift=0):
        px = [[clear] * w for _ in range(h)]

        def rect(x0, y0, x1, y1, colour):
            for y in range(max(0, y0), min(h, y1)):
                for x in range(max(0, x0), min(w, x1)):
                    px[y][x] = colour

        top = 1 - lift
        rect(5, top, 11, top + 5, skin)          # head
        rect(9, top + 2, 10, top + 3, dark)      # eye, facing right
        rect(4, top + 5, 12, top + 11, body)     # body
        rect(5 + left_leg, top + 11, 7 + left_leg, top + 15, dark)
        rect(9 + right_leg, top + 11, 11 + right_leg, top + 15, dark)
        return b"".join(bytes(p) for row in px for p in row)

    green = (80, 180, 80, 255)
    blue = (80, 120, 220, 255)
    orange = (220, 160, 50, 255)
    frames = [
        (500, figure(green, 0, 0)),          # idle 0
        (500, figure(green, 0, 0, lift=1)),  # idle 1 (breathing)
        (100, figure(blue, -1, 1)),          # walk 2..5
        (100, figure(blue, 0, 0)),
        (100, figure(blue, 1, -1)),
        (100, figure(blue, 0, 0)),
        (80, figure(orange, -1, 1, lift=1)),  # jump 6..8
        (200, figure(orange, -1, 1, lift=2)),
        (120, figure(orange, 0, 0, lift=1)),
    ]
    tags = [
        ("idle", 0, 1, PING_PONG, 0),
        ("walk", 2, 5, FORWARD, 0),
        ("jump", 6, 8, FORWARD, 1),  # plays once
    ]
    return aseprite(w, h, frames, tags)


def write(rel, data):
    path = ROOT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    print(f"wrote {rel} ({len(data)} bytes)")


if __name__ == "__main__":
    write("crates/amigo_assets/tests/fixtures/strip.aseprite", strip_fixture())
    write("examples/animation_demo/assets/sprites/hero.aseprite", hero())
