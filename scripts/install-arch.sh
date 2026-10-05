#!/usr/bin/env bash
# Coucou on Arch Linux: install dependencies, build, and install.
#
# Works on Arch, CachyOS, EndeavourOS, Manjaro and other Arch-based systems.
# Builds Coucou (the Tauri app in windows/) and the coucou-hook relay, then
# installs both into /usr/local. Safe to re-run: it rebuilds and overwrites.
#
#   ./scripts/install-arch.sh             install dependencies, build, install
#   ./scripts/install-arch.sh --no-deps   skip the pacman step and just build
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WIN="$ROOT/windows"

# Hard build/runtime dependencies. Rust is handled separately below: pacman's
# "rust" package conflicts with rustup, which many Arch developers use.
DEPS=(
  base-devel pkgconf nodejs npm
  webkit2gtk-4.1 gtk3 glib2 librsvg openssl dbus
  libayatana-appindicator patchelf xdg-utils
  gst-plugins-base gst-plugins-good
)

say() { echo "==> $*"; }
die() { echo "error: $*" >&2; exit 1; }

command -v pacman >/dev/null || die "this script is for Arch-based systems (pacman not found)"
[ -f "$WIN/src-tauri/Cargo.toml" ] || die "run this from the coucou repository"

if [ "${1:-}" != "--no-deps" ]; then
  # A repository listed in pacman.conf whose database has never been downloaded
  # makes every "pacman -S" fail with "could not find database". The Codex
  # desktop app auto-adds one such repository (openai-chatgpt), so refresh the
  # databases first.
  say "Refreshing pacman databases"
  sudo pacman -Sy --noconfirm ||
    say "Warning: a repository could not be refreshed - check the entries in /etc/pacman.conf"

  say "Installing build and runtime dependencies"
  sudo pacman -S --needed --noconfirm "${DEPS[@]}"

  # Use the Rust toolchain that is already here. pacman's "rust" package
  # conflicts with rustup, so it is only pulled in when no cargo is on PATH.
  if command -v cargo >/dev/null 2>&1; then
    say "Using the Rust toolchain already on PATH: $(cargo --version)"
  else
    say "No Rust toolchain found - installing rust from pacman"
    sudo pacman -S --needed --noconfirm rust
  fi

  # Optional: the island loads gtk-layer-shell at runtime and falls back to a
  # regular always-on-top window when it is missing, so this is not fatal.
  say "Optional: gtk-layer-shell (top-edge overlay on KDE, Hyprland, Sway)"
  sudo pacman -S --needed --noconfirm gtk-layer-shell ||
    say "gtk-layer-shell not installed - the island will be a regular window"
fi

say "Building the coucou-hook relay"
( cd "$WIN" && cargo build --release -p coucou-hook )

say "Building the front end"
( cd "$WIN" && { [ -d node_modules ] || npm install; } && npm run build )

say "Building Coucou"
( cd "$WIN" && cargo build --release -p coucou )

say "Installing into /usr/local"
sudo install -Dm755 "$WIN/target/release/coucou"      /usr/local/bin/coucou
sudo install -Dm755 "$WIN/target/release/coucou-hook" /usr/local/bin/coucou-hook
sudo install -Dm644 "$ROOT/packaging/arch/coucou.desktop" /usr/local/share/applications/coucou.desktop
for pair in icon.png:512 128x128@2x.png:256 128x128.png:128 32x32.png:32; do
  file="${pair%%:*}"; size="${pair##*:}"
  sudo install -Dm644 "$WIN/src-tauri/icons/$file" "/usr/local/share/icons/hicolor/${size}x${size}/apps/coucou.png"
done
command -v update-desktop-database >/dev/null && sudo update-desktop-database -q || true
command -v gtk-update-icon-cache >/dev/null && sudo gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true

echo
echo "Done. Coucou is installed at /usr/local/bin/coucou."
echo
echo "Next:"
echo "  1. Launch Coucou from your app menu, or run: coucou"
echo "  2. Settings... > Codex > Install hooks...  (writes ~/.codex/hooks.json)"
echo "  3. Run /hooks once in Codex to review and trust the hooks"
echo "  4. Start a Codex session - Mochi appears at the top of your screen"
echo
echo "Claude Code hooks are still available under Settings... > Claude Code."
