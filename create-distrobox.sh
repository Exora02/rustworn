#!/bin/bash

# Exit immediately if a command fails
set -e

CONTAINER_NAME="rust-dev"
IMAGE="fedora:latest"

echo "========================================"
echo "📦 1/3: Creating Distrobox Container..."
echo "========================================"
# The --yes flag auto-confirms the creation prompts
distrobox create -n "$CONTAINER_NAME" --image "$IMAGE" --yes

echo "========================================"
echo "🛠️ 2/3: Installing Dependencies..."
echo "========================================"
# We use 'distrobox enter --' to execute commands inside the container automatically.
# The -y flag tells dnf to auto-install without asking "Is this ok [y/N]?"

distrobox enter "$CONTAINER_NAME" -- sudo dnf groupinstall -y "Development Tools"

distrobox enter "$CONTAINER_NAME" -- sudo dnf install -y \
    gcc binutils glibc-devel \
    mold lld \
    wayland-devel libxkbcommon-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel \
    alsa-lib-devel pulseaudio-libs-devel systemd-devel

echo "========================================"
echo "🦀 3/3: Installing Rust & Cargo..."
echo "========================================"
# The -y flag at the end skips the Rustup installation menu
distrobox enter "$CONTAINER_NAME" -- sh -c 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'

echo "========================================"
echo "✅ Setup Complete!"
echo "========================================"
echo "Your environment is ready. Enter it by running:"
echo "distrobox enter $CONTAINER_NAME"
