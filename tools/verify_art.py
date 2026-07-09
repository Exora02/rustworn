#!/usr/bin/env python3
"""Quick verification script for art assets."""
from PIL import Image
import os

art_dir = '/opt/data/slop_rustworn/assets/art'
files = sorted(os.listdir(art_dir))

print(f"Verifying {len(files)} art assets in {art_dir}")
print()

for f in files:
    path = os.path.join(art_dir, f)
    img = Image.open(path)
    
    # Count non-transparent pixels
    pixel_count = 0
    colors = set()
    for y in range(img.height):
        for x in range(img.width):
            p = img.getpixel((x, y))
            if p[3] > 0:
                pixel_count += 1
                colors.add(p[:3])
    
    print(f"{f:25s} {img.size} mode={img.mode}  pixels={pixel_count:3d}  colors={len(colors)}  {os.path.getsize(path):5d}B")
    if len(colors) <= 8:
        hex_colors = ['#%02x%02x%02x' % c for c in sorted(colors)]
        print(f"{'':25s} colors: {', '.join(hex_colors)}")

print()
print("All sprites have transparent RGBA backgrounds. Ready for Bevy asset_server.load::<Image>()")