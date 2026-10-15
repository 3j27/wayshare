# wayshare

Share your Hyprland desktop with a Mac, using the Screen Sharing app it already has.
High Performance mode works too. There's nothing to install on the Mac.

Needs Hyprland 0.56 or newer.

## Install

**Arch-based distros**, from the AUR:

```sh
yay -S wayshare
```

**Omarchy:**

```sh
omarchy pkg add wayshare
```

**From source**, with Rust 1.89 or newer:

```sh
git clone https://github.com/wesleygrimes/wayshare
cd wayshare
cargo build --release
tools/install.sh          # or: mise run install
```

This puts `wayshare` in `/usr/local/bin` (it asks for sudo). Set `PREFIX` to install
somewhere else.

## Set up

1. Pick a password. Screen Sharing asks for this user name and password when you connect.

   ```sh
   wayshare set-password --user "$USER"
   ```

   wayshare stores a salted verifier, never the password itself.

2. Tell wayshare which monitor to share. `hyprctl monitors` lists them.

   ```sh
   mkdir -p ~/.config/wayshare
   echo 'output = "DP-1"' > ~/.config/wayshare/config.toml
   ```

3. Start it with your session:

   ```sh
   systemctl --user enable --now wayshare
   ```

4. On the Mac, open Finder, choose **Go > Connect to Server** and enter `vnc://` followed
   by the Linux machine's address, like `vnc://192.168.1.20`. It also shows up in Finder's
   sidebar under **Network**.

That's it. If something doesn't connect, see [Troubleshooting](docs/troubleshooting.md).

## Upgrade

With a package, upgrade as usual, then restart the service:

```sh
systemctl --user restart wayshare
```

From source:

```sh
git pull
cargo build --release
tools/install.sh
systemctl --user restart wayshare
```

## Uninstall

```sh
systemctl --user disable --now wayshare
```

Then remove the package, or run `tools/install.sh --uninstall` for a source install. Your
password and settings stay in `~/.config/wayshare` until you delete it.

## More

- [Configuration](docs/configuration.md): every setting, keyboard mapping, the sidebar name
- [High Performance mode](docs/high-performance.md): what it needs, encoders, audio
- [Firewall](docs/firewall.md): which ports to open
- [Troubleshooting](docs/troubleshooting.md): logs and the usual problems
- [Packaging](docs/packaging.md): install layout and dependencies, for distro packagers
- [Development](docs/development.md): building, tests, live checks

## License

Apache-2.0. See [LICENSE](LICENSE).
