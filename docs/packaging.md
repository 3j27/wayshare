# Packaging

Notes for distro packagers.

## Build

```sh
cargo build --release --frozen
```

Build dependencies: Rust 1.89 or newer, a C linker, `pkg-config`, and the EGL and GBM
development files (`mesa` and `libglvnd` on Arch). The minimum Rust is 1.89 (`rust-version` in `Cargo.toml`); the pin in
`rust-toolchain.toml` is for contributors and can be ignored. `Cargo.lock` is committed.

## Install

```sh
DESTDIR="$pkgdir" PREFIX=/usr tools/install.sh
```

| File | Installed to |
|---|---|
| binary | `$PREFIX/bin/wayshare` |
| systemd user unit | `$PREFIX/lib/systemd/user/wayshare.service` |
| example config | `$PREFIX/share/doc/wayshare/config.example.toml` |

The unit is rendered from `contrib/wayshare.service.in` with `ExecStart=$PREFIX/bin/wayshare`.
`DESTDIR` never ends up in it. `SYSTEMD_USER_UNIT_DIR`, `BINDIR` and `DOCDIR` override the
defaults.

Don't enable the unit globally. Each user runs `systemctl --user enable --now wayshare` after
setting a password.

## Dependencies

| Dependency | Kind | For |
|---|---|---|
| Hyprland >= 0.56 | runtime | everything |
| libEGL, libgbm (Mesa) | linked | capture |
| libva and a driver that encodes HEVC | optional, loaded at runtime | High Performance via VA-API |
| x265 (API build 217) | optional, loaded at runtime | High Performance without VA-API |
| fdk-aac | optional, loaded at runtime | High Performance audio |
| libpulse, plus a PulseAudio-compatible server such as `pipewire-pulse` | optional, loaded at runtime | High Performance audio |

Optional libraries are opened by name (`libva-drm.so.2`, `libx265.so`, `libfdk-aac.so.2`,
`libpulse-simple.so.0`). On Nix, put them on the runtime library path.
