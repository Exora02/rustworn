# devenv.nix — reproducible RUSTWORN dev environment.
# Works on Linux (Ubuntu, Fedora, Bazzite, NixOS), macOS, and WSL2.
# Activate with `devenv shell` or auto via `devenv hook bash` (one line
# in your shell config — see docs/devenv.md).
#
# First time: install devenv from https://devenv.sh/getting-started/
# Then: `cd` into this dir, run `devenv allow` once to trust, done.
{ pkgs, lib, config, ... }:

{
  # ─── Rust toolchain ──────────────────────────────────────────────────
  # devenv uses rustup under the hood. Components install on first shell.
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rustfmt"
      "clippy"
      "rust-src"
    ];

    # Set via the rust module (not the global `env`) so we don't conflict
    # with the type that the module already declares for RUSTFLAGS.
    rustflags = "-C link-arg=-fuse-ld=mold -C target-cpu=native";

    # Cross-compilation targets live here when we add them.
    # targets = [ "wasm32-unknown-unknown" ];
  };

  # ─── System packages ────────────────────────────────────────────────
  # Everything Bevy + cargo + dev tooling needs.
  # Linux picks up the wayland / xkb / udev / alsa set;
  # macOS ignores those and just uses the rust toolchain.
  packages = with pkgs; [
    # Fast linker — ~50% faster Bevy link times than default ld
    mold

    # Build essentials
    pkg-config
    gnumake

    # Bevy graphics & input on Linux
    wayland
    libxkbcommon
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    vulkan-loader
    vulkan-headers
    vulkan-tools
    mesa
    # Audio
    alsa-lib
    # Input devices
    udev
    libevdev
    # Shaders / SPIR-V (Bevy uses naga)
    shaderc
  ] ++ lib.optionals stdenv.isLinux [
    # Linux-only niceties
    bashInteractive
  ] ++ lib.optionals stdenv.isDarwin [
    # macOS frameworks (clang pulls these in transitively, listed
    # for clarity)
    darwin.apple_sdk.frameworks.Cocoa
    darwin.apple_sdk.frameworks.IOKit
    darwin.apple_sdk.frameworks.CoreVideo
    darwin.apple_sdk.frameworks.CoreAudio
    darwin.apple_sdk.frameworks.OpenGL
  ];

  # ─── Shell environment ──────────────────────────────────────────────
  env = {
    # Faster incremental builds
    CARGO_INCREMENTAL = "1";
    # Color output even in pipes
    CARGO_TERM_COLOR = "always";
    # Default log level
    RUST_LOG = "warn";
  };

  # ─── Convenience scripts ────────────────────────────────────────────
  # Run with: devenv shell build    OR    scripts are auto-exposed as
  # `rustworn-build`, etc. if you set scripts.<name>.exec.
  scripts = {
    check.exec  = "cargo check";
    build.exec  = "cargo build";
    run.exec    = "cargo run";
    fmt.exec    = "cargo fmt --all";
    clippy.exec = "cargo clippy --all-targets -- -W clippy::all";
    bench.exec  = "cargo bench";
  };

  # ─── Pre-commit hooks (devenv git-hooks module, opt-in) ─────────────
  # Uncomment to enable. First run installs the hooks into .git/hooks/.
  # git-hooks.hooks.pre-commit = {
  #   enable = true;
  #   text = ''
  #     cargo fmt --all -- --check
  #     cargo clippy --all-targets -- -W clippy::all
  #   '';
  # };

  # ─── Enter shell message ────────────────────────────────────────────
  enterShell = ''
    cat <<'EOF'
    ┌──────────────────────────────────────────────┐
    │  RUSTWORN dev env                            │
    │                                              │
    │  cargo check  cargo build  cargo run         │
    │  cargo fmt    cargo clippy                   │
    │                                              │
    │  scripts: check / build / run / fmt / clippy │
    └──────────────────────────────────────────────┘
    EOF
  '';

  # ─── Cleanup on shell exit ─────────────────────────────────────────
  # (Uncomment to nuke target/ on shell exit. Useful if disk is tight;
  # annoying if you iterate often. Default: keep target/.)
  # enterShellEnd = ''rm -rf target'';
}
