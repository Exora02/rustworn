#!/usr/bin/env bash
# setup-nuked-dev.sh — One-shot Bazzite / Fedora Atomic dev env setup
#
# Creates a distrobox named "nuked-dev" with Nix + devenv.sh. The
# devenv native shell hook (`devenv hook bash`, added to ~/.bashrc)
# auto-activates the project environment on `cd` — no direnv needed.
#
# Idempotent: safe to re-run. Already-installed components are skipped.
#
# Usage (on Bazzite):
#   chmod +x scripts/setup-nuked-dev.sh
#   ./scripts/setup-nuked-dev.sh
#
# After:  distrobox enter nuked-dev
#         cd /path/to/rustworn
#         devenv allow      # trust this project
#         cargo run         # devenv activates automatically

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

# 1. System packages.
if ! command -v git &>/dev/null; then
    log "Installing git + curl + sudo..."
    sudo dnf install -y git curl sudo
else
    log "git/curl already installed."
fi

# 2. Pre-create the nixbld group with 10 build users. The official Nix
# installer is strict: it requires the nixbld group to exist AND have
# at least 10 members AND none of them can be the install user.
# distrobox runs as root, so the install user = root.
# Workaround: create 10 dummy build users in the nixbld group.
if ! getent group nixbld > /dev/null; then
    log "Creating nixbld group and 10 build users (Nix installer needs this)..."
    sudo groupadd -g 30000 nixbld
    for i in $(seq 1 10); do
        sudo useradd -r -d /var/empty -s /sbin/nologin \
            -c "Nix build user $i" "nixbld$i" 2>/dev/null || true
        # useradd -g nixbld sets primary group, but the Nix installer
        # checks getent group which only shows supplementary members in
        # some container setups. Use usermod -aG to be safe.
        sudo usermod -aG nixbld "nixbld$i" 2>/dev/null || true
    done
    log "nixbld group + 10 users created."
else
    log "nixbld group already exists."
fi

# 3. Install Nix via the official installer. --no-daemon is the right
# mode for distrobox (no systemd available).
if ! command -v nix &>/dev/null; then
    log "Installing Nix via the official installer..."
    sh <(curl -L https://nixos.org/nix/install) --no-daemon
    # Make Nix available in this shell and future shells.
    if [ -f ~/.nix-profile/etc/profile.d/nix.sh ]; then
        . ~/.nix-profile/etc/profile.d/nix.sh
    fi
    if ! grep -q 'nix-profile/etc/profile.d/nix.sh' ~/.bashrc 2>/dev/null; then
        echo '' >> ~/.bashrc
        echo '# Nix' >> ~/.bashrc
        echo '. ~/.nix-profile/etc/profile.d/nix.sh' >> ~/.bashrc
    fi
else
    log "nix already installed."
fi

# 4. Enable the nix-command and flakes experimental features. The new
# `nix profile` CLI (which we use below) is gated behind nix-command.
# Without this, `nix profile install` errors with "experimental Nix
# feature 'nix-command' is disabled".
mkdir -p ~/.config/nix
if ! grep -q 'nix-command' ~/.config/nix/nix.conf 2>/dev/null; then
    log "Enabling nix-command + flakes experimental features..."
    cat > ~/.config/nix/nix.conf <<'NIXCONF'
# Required for `nix profile install` (used by devenv install).
experimental-features = nix-command flakes
NIXCONF
else
    log "nix experimental features already enabled."
fi

# 5. Install devenv (a Nix profile install — puts devenv in
# ~/.nix-profile/bin/ alongside nix itself).
if ! command -v devenv &>/dev/null; then
    log "Installing devenv via nix profile (this downloads ~500MB)..."
    nix profile install nixpkgs#devenv
else
    log "devenv already installed."
fi

# 6. Wire up the devenv native shell hook. This is the new (devenv
# 2.0+) way to auto-activate on `cd` — no direnv, no .envrc needed.
# The hook walks up from $PWD looking for devenv.yaml and starts a
# subshell when found.
if ! grep -q 'devenv hook bash' ~/.bashrc 2>/dev/null; then
    log "Adding devenv hook to ~/.bashrc..."
    {
        echo ''
        echo '# devenv auto-activation (project shells spawn on cd)'
        echo 'eval "$(devenv hook bash)"'
    } >> ~/.bashrc
else
    log "devenv hook already in ~/.bashrc."
fi

# 7. Verify
log "Versions:"
echo "  nix:    $(nix --version 2>/dev/null || echo 'NOT FOUND')"
echo "  devenv: $(devenv version 2>/dev/null || echo 'NOT FOUND')"

log "Inside-box setup done."
INNER_EOF

# ─── Done ───────────────────────────────────────────────────────────────
log ""
log "✓ '${BOX_NAME}' is ready."
log ""
log "  Enter it with:    distrobox enter ${BOX_NAME}"
log "  Or one-shot:      distrobox enter ${BOX_NAME} -- devenv shell cargo run"
log ""
log "Once inside:"
log "  cd ~/projects/rustworn   (or wherever you cloned it)"
log "  devenv allow             # trust the project (one time)"
log "  cargo run                # devenv activates automatically on cd"
log ""
log "Tip: add this alias to your Bazzite ~/.bashrc for quick access:"
log "  alias nuke='distrobox enter ${BOX_NAME}'"
