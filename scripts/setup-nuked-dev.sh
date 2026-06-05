#!/usr/bin/env bash
# setup-nuked-dev.sh — One-shot Bazzite / Fedora Atomic dev env setup
#
# Creates a distrobox named "nuked-dev" with Nix, devenv, direnv, and
# the system libs most Rust + Bevy projects need.
#
# Idempotent: safe to re-run. Already-installed components are skipped.
#
# Usage (on Bazzite):
#   chmod +x scripts/setup-nuked-dev.sh
#   ./scripts/setup-nuked-dev.sh
#
# After:  distrobox enter nuked-dev

set -euo pipefail

BOX_NAME="nuked-dev"
BOX_IMAGE="fedora:39"

# ─── Helpers ────────────────────────────────────────────────────────────
log()  { printf "\033[1;34m[nuked-dev]\033[0m %s\n" "$*"; }
warn() { printf "\033[1;33m[warn]\033[0m %s\n" "$*" >&2; }
die()  { printf "\033[1;31m[error]\033[0m %s\n" "$*" >&2; exit 1; }

# ─── Pre-flight: distrobox installed? ───────────────────────────────────
if ! command -v distrobox &>/dev/null; then
    log "distrobox not found. Installing via rpm-ostree..."
    if ! command -v rpm-ostree &>/dev/null; then
        die "Neither distrobox nor rpm-ostree is available. Are you on Bazzite?"
    fi
    rpm-ostree install distrobox
    warn "REBOOT REQUIRED to finish installing distrobox."
    warn "After reboot, re-run this script."
    exit 1
fi

# ─── Create distrobox if missing ────────────────────────────────────────
if distrobox list 2>/dev/null | grep -q "^${BOX_NAME}"; then
    log "distrobox '${BOX_NAME}' already exists, skipping creation."
else
    log "Creating distrobox '${BOX_NAME}' from ${BOX_IMAGE}..."
    distrobox create --image "${BOX_IMAGE}" --name "${BOX_NAME}" --yes
fi

# ─── Install everything inside the distrobox ────────────────────────────
log "Installing dev tools inside '${BOX_NAME}' (may take a few minutes)..."

distrobox enter "${BOX_NAME}" -- bash <<'INNER_EOF'
set -euo pipefail

log() { printf "  \033[1;34m[nuked-dev]\033[0m %s\n" "$*"; }

# 1. System packages
if ! command -v nix &>/dev/null; then
    log "Installing nix + direnv + git + curl + sudo..."
    sudo dnf install -y nix direnv git curl sudo
    . /etc/profile.d/nix.sh
else
    log "nix already installed."
fi

# 2. Make direnv auto-activate in future shells
if ! grep -q 'direnv hook bash' ~/.bashrc 2>/dev/null; then
    log "Adding direnv hook to ~/.bashrc..."
    {
        echo ''
        echo '# direnv auto-activation'
        echo 'eval "$(direnv hook bash)"'
    } >> ~/.bashrc
else
    log "direnv hook already in ~/.bashrc."
fi

# 3. Install devenv
if ! command -v devenv &>/dev/null; then
    log "Installing devenv via nix profile (this downloads ~500MB)..."
    . /etc/profile.d/nix.sh
    nix profile install nixpkgs#devenv
else
    log "devenv already installed."
fi

# 4. Verify
log "Versions:"
echo "  nix:    $(nix --version 2>/dev/null || echo 'NOT FOUND')"
echo "  devenv: $(devenv version 2>/dev/null || echo 'NOT FOUND')"
echo "  direnv: $(direnv --version 2>/dev/null || echo 'NOT FOUND')"
echo "  cargo:  $(cargo --version 2>/dev/null || echo 'NOT FOUND')"

log "Inside-box setup done."
INNER_EOF

# ─── Done ───────────────────────────────────────────────────────────────
log ""
log "✓ '${BOX_NAME}' is ready."
log ""
log "  Enter it with:    distrobox enter ${BOX_NAME}"
log "  Or one-shot:      distrobox enter ${BOX_NAME} -- cargo run"
log ""
log "Once inside:"
log "  cd ~/projects/rustworn   (or wherever you cloned it)"
log "  direnv allow"
log "  cargo run"
log ""
log "Tip: add this alias to your Bazzite ~/.bashrc for quick access:"
log "  alias nuke='distrobox enter ${BOX_NAME}'"
