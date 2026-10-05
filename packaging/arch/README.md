# Arch Linux packaging

Two ways to install Coucou on Arch-based systems (Arch, CachyOS, EndeavourOS,
Manjaro).

## From the repository (quickest)

    ./scripts/install-arch.sh

This installs the build dependencies with pacman, builds the app and the
coucou-hook relay, and copies them into /usr/local, along with a desktop entry
and the icons.

## As a pacman package

    cd packaging/arch
    makepkg -si

makepkg clones the fork named by _fork in the PKGBUILD (the default is upstream
branch main), builds, and produces an installable
coucou-<version>-<rel>-x86_64.pkg.tar.zst.

## Dependencies

Required: webkit2gtk-4.1, gtk3, glib2, librsvg, openssl, dbus,
libayatana-appindicator, gst-plugins-base, gst-plugins-good, xdg-utils.

Optional: gtk-layer-shell. The island loads it at runtime with dlopen, so the app
builds and runs without it; without it the island is a regular always-on-top
window instead of a top-edge overlay. It only adds the overlay behaviour on
compositors that support layer-shell (KDE Plasma, Hyprland, Sway and other
wlroots compositors). GNOME has no layer-shell either way.

## After installing

Open Settings > Codex > Install hooks to write ~/.codex/hooks.json, then run
/hooks once in the Codex CLI to review and trust them. Claude Code hooks are
still available under Settings > Claude Code.

