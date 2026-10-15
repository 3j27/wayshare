# Development

```sh
cargo test
```

Tests run on Linux and macOS. Capture, input and encoding only build on Linux.

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
