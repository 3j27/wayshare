# Development

```sh
cargo test
```

Tests run on Linux and macOS. Capture, input and encoding only build on Linux.

## Build prerequisites

Rust 1.89 or newer ([rustup](https://rustup.rs) or your distro's package), a C linker,
`pkg-config`, and the EGL and GBM development files:

| Distro | Packages |
|---|---|
| Arch | `base-devel mesa libglvnd` |
| Debian, Ubuntu | `build-essential pkg-config libegl-dev libgbm-dev` |
| Fedora | `gcc pkgconf-pkg-config libglvnd-devel mesa-libgbm-devel` |

## Installing from source

`tools/install.sh` installs the release build under `PREFIX` (default `/usr/local`, using sudo
when it can't write there). For a user-local install with no sudo:

```sh
PREFIX=$HOME/.local SYSTEMD_USER_UNIT_DIR=$HOME/.local/share/systemd/user tools/install.sh
```

Pass the same variables when you upgrade or run `tools/install.sh --uninstall`. After
upgrading, run `systemctl --user daemon-reload` before restarting the service.

## mise

[mise](https://mise.jdx.dev) is optional. It reads the toolchain pin from
`rust-toolchain.toml` and installs `bindgen-cli`. Tasks:

| Task | Does |
|---|---|
| `mise run build` | release build |
| `mise run test` | `cargo test` |
| `mise run install` | build, then `tools/install.sh` |
| `mise run uninstall` | `tools/install.sh --uninstall` |
| `mise run gen-vaapi-sys` | regenerate the libva bindings (Linux) |

## Bindings

`src/encode/vaapi/sys.rs` is generated. Regenerate it with `tools/gen-vaapi-sys.sh` rather
than editing it.

## Style

Please don't run `cargo fmt` over the crate; it reflows every file. Match the formatting
around your change.
