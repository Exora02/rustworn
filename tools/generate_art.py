#!/usr/bin/env python3
"""
Procedural pixel art generator for Slop Rustworn.
Creates colored sprite PNGs (40x40) with transparent backgrounds.
Each sprite is drawn as a simple pixel-art shape.

Usage:
    python tools/generate_art.py
"""

import os
from PIL import Image, ImageDraw

SIZE = 40
OUT = os.path.join(os.path.dirname(os.path.dirname(__file__)), "assets", "art")
os.makedirs(OUT, exist_ok=True)


def new_canvas():
    """Create a new 40x40 RGBA canvas with transparent background."""
    return Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))


def save(canvas, name):
    path = os.path.join(OUT, f"{name}.png")
    canvas.save(path)
    print(f"  Created: {path}")


# ── Color Palette ──────────────────────────────────────────────
C = {
    "player_skin":   (200, 160, 120, 255),  # light tan
    "player_armor":  (60,  120, 200, 255),  # blue
    "player_visor":  (0,   200, 255, 255),  # cyan
    "player_trim":   (40,  40,  60,  255),  # dark blue-grey

    "chaser_body":   (180, 60,  60,  255),  # red
    "chaser_eye":    (255, 255, 100, 255),  # yellow
    "chaser_horn":   (40,  20,  20,  255),  # dark red

    "dasher_body":   (160, 40,  160, 255),  # purple
    "dasher_trail":  (120, 30,  120, 200),  # translucent purple
    "dasher_eye":    (255, 100, 255, 255),  # pink

    "heart":         (255, 40,  40,  255),  # red
    "heart_highlight": (255, 150, 150, 255),

    "pistol_body":   (180, 180, 180, 255),  # light grey
    "pistol_grip":   (120, 80,  40,  255),  # brown
    "pistol_barrel": (100, 100, 120, 255),

    "shotgun_body":  (160, 120, 80,  255),  # brown
    "shotgun_barrel": (100, 80,  60,  255),
    "shotgun_band":  (200, 80,  50,  255),  # orange band

    "burst_body":    (80,  160, 200, 255),  # teal
    "burst_accent":  (50,  220, 220, 255),  # bright cyan

    "beam_body":     (200, 200, 80,  255),  # yellow
    "beam_core":     (255, 255, 150, 255),  # light yellow
    "beam_coil":     (80,  80,  200, 255),  # blue coil

    "rocket_body":   (80,  180, 80,  255),  # green
    "rocket_tip":    (255, 100, 50,  255),  # orange
    "rocket_fin":    (60,  60,  60,  255),  # dark grey

    "lich_robe":     (30,  20,  60,  255),  # dark purple
    "lich_skin":     (180, 220, 255, 255),  # pale blue
    "lich_eye_glow": (0,   200, 255, 255),  # cyan
    "lich_crown":    (200, 160, 40,  255),  # gold

    "yeti_fur":      (200, 220, 240, 255),  # white-blue
    "yeti_skin":     (100, 160, 200, 255),  # ice blue
    "yeti_eye":      (255, 50,  50,  255),  # red
    "yeti_claw":     (60,  80,  100, 255),  # grey-blue

    "inferno_skin":  (220, 80,  20,  255),  # orange
    "inferno_eye":   (255, 255, 200, 255),  # white-yellow
    "inferno_core":  (255, 200, 50,  255),  # yellow
    "inferno_horn":  (60,  20,  10,  255),  # dark red

    "treant_bark":   (80,  60,  40,  255),  # brown
    "treant_leaf":   (40,  160, 40,  255),  # green
    "treant_eye":    (150, 255, 150, 255),  # light green
    "treant_moss":   (60,  120, 60,  255),  # dark green

    "void_body":     (40,  10,  60,  255),  # deep purple
    "void_glow":     (120, 60,  200, 255),  # purple glow
    "void_eye":      (255, 200, 255, 255),  # white-purple
    "void_void":     (10,  0,   20,  255),  # near black
}


# ── Drawing Helpers ────────────────────────────────────────────

def rect(d, x1, y1, x2, y2, color, fill=True):
    """Draw a filled or outline rectangle."""
    if fill:
        d.rectangle([x1, y1, x2, y2], fill=color)
    else:
        d.rectangle([x1, y1, x2, y2], outline=color, width=1)


def ellipse(d, cx, cy, rx, ry, color, fill=True):
    """Draw a filled or outline ellipse."""
    bbox = [cx - rx, cy - ry, cx + rx, cy + ry]
    if fill:
        d.ellipse(bbox, fill=color)
    else:
        d.ellipse(bbox, outline=color, width=1)


def line(d, x1, y1, x2, y2, color, width=1):
    d.line([x1, y1, x2, y2], fill=color, width=width)


def polygon(d, points, color):
    d.polygon(points, fill=color)


# ── Sprite Definitions ─────────────────────────────────────────

def sprite_player():
    """40x40 pixel-art player character"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Body (torso)
    rect(d, 12, 16, 27, 30, C["player_armor"])
    # Head
    rect(d, 14, 6, 25, 17, C["player_skin"])
    # Helmet/visor
    rect(d, 15, 8, 24, 14, C["player_visor"])
    # Visor shine
    rect(d, 17, 9, 22, 11, (255, 255, 255, 180))
    # Legs
    rect(d, 13, 30, 19, 38, C["player_armor"])
    rect(d, 20, 30, 26, 38, C["player_armor"])
    # Arms (shoulder pads)
    rect(d, 6,  18, 12, 25, C["player_trim"])
    rect(d, 27, 18, 33, 25, C["player_trim"])
    # Boots
    rect(d, 13, 36, 19, 39, C["player_trim"])
    rect(d, 20, 36, 26, 39, C["player_trim"])
    # Belt
    rect(d, 13, 28, 26, 30, C["player_trim"])

    return img


def sprite_enemy_chaser():
    """Chaser enemy - red chompy monster"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Body
    polygon(d, [(8, 15), (20, 6), (32, 15), (32, 30), (20, 36), (8, 30)], C["chaser_body"])
    # Eyes
    rect(d, 12, 14, 17, 19, C["chaser_eye"])
    rect(d, 23, 14, 28, 19, C["chaser_eye"])
    # Pupils
    rect(d, 13, 15, 16, 17, (0, 0, 0, 255))
    rect(d, 24, 15, 27, 17, (0, 0, 0, 255))
    # Mouth/teeth
    rect(d, 13, 24, 27, 27, (255, 255, 255, 255))
    # Teeth gaps
    rect(d, 16, 24, 18, 27, C["chaser_body"])
    rect(d, 22, 24, 24, 27, C["chaser_body"])
    # Horns
    polygon(d, [(12, 14), (8, 4), (15, 10)], C["chaser_horn"])
    polygon(d, [(28, 14), (32, 4), (25, 10)], C["chaser_horn"])
    # Feet
    rect(d, 10, 32, 17, 38, C["chaser_body"])
    rect(d, 23, 32, 30, 38, C["chaser_body"])

    return img


def sprite_enemy_dasher():
    """Dasher enemy - purple speedster with trail"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Trail effect (semi-transparent)
    polygon(d, [(4, 16), (10, 10), (10, 30), (4, 24)], C["dasher_trail"])
    polygon(d, [(0, 18), (4, 12), (4, 28), (0, 22)], C["dasher_trail"])

    # Body - streamlined
    polygon(d, [(10, 10), (34, 16), (34, 24), (10, 30)], C["dasher_body"])
    # Head
    ellipse(d, 30, 20, 8, 10, C["dasher_body"])
    # Eye
    ellipse(d, 32, 18, 3, 3, C["dasher_eye"])
    # Core glow
    ellipse(d, 20, 20, 5, 5, C["dasher_eye"])
    # Fin
    polygon(d, [(16, 6), (22, 10), (18, 14)], C["dasher_body"])
    polygon(d, [(16, 34), (22, 30), (18, 26)], C["dasher_body"])

    return img


def sprite_ui_heart():
    """Heart icon for HP"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Heart shape using two circles + triangle
    ellipse(d, 12, 12, 7, 7, C["heart"])
    ellipse(d, 28, 12, 7, 7, C["heart"])
    polygon(d, [(5, 17), (20, 36), (35, 17)], C["heart"])
    # Highlight
    ellipse(d, 12, 12, 4, 4, C["heart_highlight"])
    ellipse(d, 28, 12, 4, 4, C["heart_highlight"])

    return img


def sprite_weapon_pistol():
    """Pistol weapon icon"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Barrel
    rect(d, 18, 14, 38, 20, C["pistol_barrel"])
    # Body/receiver
    rect(d, 10, 14, 22, 24, C["pistol_body"])
    # Grip
    polygon(d, [(10, 22), (16, 22), (14, 34), (8, 34)], C["pistol_grip"])
    # Trigger guard
    rect(d, 17, 22, 20, 28, (0, 0, 0, 0))
    polygon(d, [(16, 23), (20, 23), (20, 27), (16, 27)], (80, 80, 80, 255))
    # Muzzle
    rect(d, 36, 15, 38, 19, (60, 60, 60, 255))

    return img


def sprite_weapon_shotgun():
    """Shotgun weapon icon"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Long barrel
    rect(d, 16, 14, 38, 20, C["shotgun_barrel"])
    # Barrel band
    rect(d, 28, 13, 30, 21, C["shotgun_band"])
    rect(d, 36, 13, 38, 21, C["shotgun_band"])
    # Pump/forend
    rect(d, 16, 22, 24, 28, C["shotgun_body"])
    # Receiver
    rect(d, 8, 14, 18, 26, C["shotgun_body"])
    # Stock
    polygon(d, [(8, 16), (2, 22), (2, 28), (8, 26)], (120, 80, 40, 255))
    # Muzzle
    rect(d, 36, 15, 38, 19, (60, 60, 60, 255))

    return img


def sprite_weapon_burst():
    """Burst fire weapon icon"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Body
    polygon(d, [(12, 14), (34, 14), (34, 26), (12, 26)], C["burst_body"])
    # Barrel cluster (3 barrels)
    rect(d, 30, 12, 38, 15, C["burst_accent"])
    rect(d, 30, 17, 38, 19, C["burst_accent"])
    rect(d, 30, 22, 38, 25, C["burst_accent"])
    # Grip
    polygon(d, [(14, 24), (20, 24), (18, 36), (12, 36)], C["burst_body"])
    # Scope
    rect(d, 20, 8, 28, 14, (40, 40, 60, 255))
    rect(d, 22, 7, 26, 8, (200, 200, 200, 255))

    return img


def sprite_weapon_beam():
    """Beam weapon icon"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Body
    rect(d, 12, 16, 30, 28, C["beam_body"])
    # Coil
    ellipse(d, 16, 22, 4, 6, C["beam_coil"])
    ellipse(d, 22, 22, 4, 6, C["beam_coil"])
    ellipse(d, 28, 22, 4, 6, C["beam_coil"])
    # Emitter
    rect(d, 30, 16, 38, 20, C["beam_core"])
    rect(d, 30, 24, 38, 28, C["beam_core"])
    # Glow effect
    ellipse(d, 36, 20, 4, 8, (255, 255, 100, 120))
    # Grip
    polygon(d, [(14, 26), (20, 26), (18, 36), (12, 36)], (100, 100, 60, 255))
    # Energy indicator
    rect(d, 14, 18, 16, 20, (0, 255, 255, 255))

    return img


def sprite_weapon_rocket():
    """Rocket launcher weapon icon"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Tube/body
    rect(d, 10, 14, 36, 26, C["rocket_body"])
    # Rocket in tube (tip visible)
    rect(d, 34, 15, 38, 25, C["rocket_tip"])
    # Front sight
    rect(d, 22, 11, 24, 14, C["rocket_fin"])
    # Grip
    polygon(d, [(12, 24), (18, 24), (16, 36), (10, 36)], C["rocket_fin"])
    # Stock
    rect(d, 4, 16, 10, 24, C["rocket_fin"])
    # Warning stripes
    rect(d, 18, 14, 20, 18, (255, 200, 0, 255))
    rect(d, 18, 22, 20, 26, (255, 200, 0, 255))

    return img


def sprite_boss_lich():
    """Lich boss - dark purple necromancer"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Robe (wide base)
    polygon(d, [(4, 14), (36, 14), (36, 38), (4, 38)], C["lich_robe"])
    # Robe trim
    rect(d, 4, 30, 36, 32, (50, 40, 80, 255))
    # Head/skull
    polygon(d, [(12, 2), (28, 2), (30, 14), (10, 14)], C["lich_skin"])
    # Eye sockets (glowing)
    rect(d, 14, 6, 18, 10, C["lich_eye_glow"])
    rect(d, 22, 6, 26, 10, C["lich_eye_glow"])
    # Crown
    polygon(d, [(10, 3), (14, 0), (16, 3), (20, 0), (24, 3), (26, 0), (30, 3)], C["lich_crown"])
    # Arms coming out of robe
    rect(d, 4, 16, 8, 28, C["lich_robe"])
    rect(d, 32, 16, 36, 28, C["lich_robe"])
    # Hands
    rect(d, 2, 22, 6, 28, C["lich_skin"])
    rect(d, 34, 22, 38, 28, C["lich_skin"])
    # Sash
    rect(d, 14, 24, 26, 28, C["lich_crown"])

    return img


def sprite_boss_yeti():
    """Yeti boss - big ice monster"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Body (huge)
    polygon(d, [(6, 8), (34, 8), (38, 20), (36, 36), (4, 36), (2, 20)], C["yeti_fur"])
    # Belly
    polygon(d, [(12, 16), (28, 16), (30, 34), (10, 34)], C["yeti_skin"])
    # Eyes
    rect(d, 10, 12, 14, 16, C["yeti_eye"])
    rect(d, 26, 12, 30, 16, C["yeti_eye"])
    # Pupils
    rect(d, 11, 13, 13, 15, (0, 0, 0, 255))
    rect(d, 27, 13, 29, 15, (0, 0, 0, 255))
    # Snout / teeth
    rect(d, 14, 20, 26, 24, (255, 255, 255, 255))
    # Teeth
    rect(d, 16, 20, 18, 23, C["yeti_skin"])
    rect(d, 22, 20, 24, 23, C["yeti_skin"])
    # Arms
    rect(d, 0, 14, 6, 26, C["yeti_fur"])
    rect(d, 34, 14, 40, 26, C["yeti_fur"])
    # Claws
    rect(d, 0, 10, 3, 14, C["yeti_claw"])
    rect(d, 37, 10, 40, 14, C["yeti_claw"])
    # Feet
    rect(d, 6, 34, 14, 38, C["yeti_fur"])
    rect(d, 26, 34, 34, 38, C["yeti_fur"])
    # Fur tufts
    polygon(d, [(18, 4), (20, 0), (22, 4)], C["yeti_fur"])
    polygon(d, [(20, 4), (22, 0), (24, 4)], C["yeti_fur"])

    return img


def sprite_boss_inferno():
    """Inferno boss - fire demon"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Body
    polygon(d, [(8, 10), (32, 10), (36, 24), (32, 36), (8, 36), (4, 24)], C["inferno_skin"])
    # Core glow
    ellipse(d, 20, 22, 8, 10, C["inferno_core"])
    # Eyes
    rect(d, 12, 14, 16, 18, C["inferno_eye"])
    rect(d, 24, 14, 28, 18, C["inferno_eye"])
    # Pupils
    rect(d, 13, 15, 15, 17, (255, 100, 0, 255))
    rect(d, 25, 15, 27, 17, (255, 100, 0, 255))
    # Horns
    polygon(d, [(10, 10), (6, 0), (14, 8)], C["inferno_horn"])
    polygon(d, [(30, 10), (34, 0), (26, 8)], C["inferno_horn"])
    # Wings (small flame wings)
    polygon(d, [(4, 14), (0, 20), (6, 22)], C["inferno_skin"])
    polygon(d, [(36, 14), (40, 20), (34, 22)], C["inferno_skin"])
    # Flame aura
    polygon(d, [(14, 34), (20, 38), (26, 34)], C["inferno_core"])
    # Feet
    rect(d, 10, 34, 16, 38, C["inferno_horn"])
    rect(d, 24, 34, 30, 38, C["inferno_horn"])

    return img


def sprite_boss_treant():
    """Treant boss - tree monster"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Trunk
    rect(d, 12, 12, 28, 38, C["treant_bark"])
    # Roots (feet)
    rect(d, 6, 34, 14, 38, C["treant_bark"])
    rect(d, 26, 34, 34, 38, C["treant_bark"])
    rect(d, 4, 30, 8, 34, C["treant_bark"])
    rect(d, 32, 30, 36, 34, C["treant_bark"])
    # Canopy / leaves
    polygon(d, [(6, 12), (20, 0), (34, 12)], C["treant_leaf"])
    ellipse(d, 10, 4, 6, 5, C["treant_leaf"])
    ellipse(d, 20, 2, 6, 5, C["treant_leaf"])
    ellipse(d, 30, 4, 6, 5, C["treant_leaf"])
    # Eyes (glowing green)
    rect(d, 14, 16, 18, 20, C["treant_eye"])
    rect(d, 22, 16, 26, 20, C["treant_eye"])
    # Mouth (knot hole)
    ellipse(d, 20, 26, 5, 3, C["treant_moss"])
    # Branches (arms)
    polygon(d, [(12, 16), (4, 12), (10, 18)], C["treant_bark"])
    polygon(d, [(28, 16), (36, 12), (30, 18)], C["treant_bark"])
    # Moss patches
    rect(d, 16, 30, 24, 32, C["treant_moss"])
    rect(d, 10, 22, 14, 24, C["treant_moss"])
    rect(d, 26, 22, 30, 24, C["treant_moss"])

    return img


def sprite_boss_voidwalker():
    """Voidwalker boss - cosmic horror"""
    img = new_canvas()
    d = ImageDraw.Draw(img)

    # Dark void aura (big circle)
    ellipse(d, 20, 20, 20, 18, C["void_body"])
    # Inner void
    ellipse(d, 20, 20, 12, 11, C["void_void"])
    # Glow ring
    d.ellipse([4, 6, 36, 34], outline=C["void_glow"], width=2)
    # Eyes (three glowing)
    rect(d, 12, 16, 16, 20, C["void_eye"])
    rect(d, 24, 16, 28, 20, C["void_eye"])
    rect(d, 18, 22, 22, 26, C["void_eye"])
    # Pupils
    rect(d, 13, 17, 15, 19, C["void_body"])
    rect(d, 25, 17, 27, 19, C["void_body"])
    rect(d, 19, 23, 21, 25, C["void_body"])
    # Tendrils
    polygon(d, [(2, 28), (0, 36), (6, 32)], C["void_glow"])
    polygon(d, [(38, 28), (40, 36), (34, 32)], C["void_glow"])
    polygon(d, [(14, 34), (12, 38), (18, 36)], C["void_glow"])
    polygon(d, [(26, 34), (28, 38), (22, 36)], C["void_glow"])
    # Stars/sparkles
    rect(d, 8, 8, 10, 10, (255, 255, 255, 200))
    rect(d, 30, 6, 32, 8, (255, 255, 255, 200))
    rect(d, 6, 22, 8, 24, (200, 150, 255, 180))
    rect(d, 32, 26, 34, 28, (200, 150, 255, 180))

    return img


# ── Generate All Assets ────────────────────────────────────────

SPRITES = [
    ("player", sprite_player),
    ("enemy_chaser", sprite_enemy_chaser),
    ("enemy_dasher", sprite_enemy_dasher),
    ("ui_heart", sprite_ui_heart),
    ("weapon_pistol", sprite_weapon_pistol),
    ("weapon_shotgun", sprite_weapon_shotgun),
    ("weapon_burst", sprite_weapon_burst),
    ("weapon_beam", sprite_weapon_beam),
    ("weapon_rocket", sprite_weapon_rocket),
    ("boss_lich", sprite_boss_lich),
    ("boss_yeti", sprite_boss_yeti),
    ("boss_inferno", sprite_boss_inferno),
    ("boss_treant", sprite_boss_treant),
    ("boss_voidwalker", sprite_boss_voidwalker),
]

def main():
    print(f"Generating {len(SPRITES)} art assets to {OUT}...")
    for name, func in SPRITES:
        img = func()
        img.save(os.path.join(OUT, f"{name}.png"))
        print(f"  ✓ {name}.png  ({img.size[0]}x{img.size[1]})")
    print(f"\nDone! {len(SPRITES)} files created in {OUT}")


if __name__ == "__main__":
    main()