# wayshare

Share your Hyprland desktop with a Mac, using the Screen Sharing app it already has.
There's nothing to install on the Mac, and its High Performance mode works too, for sharp,
low-latency video and sound.

Needs Hyprland 0.56 or newer and systemd.

## Install

From the AUR, on Arch-based distros:

```sh
yay -S wayshare
```

On Omarchy:

```sh
omarchy pkg add wayshare
```

From source, after installing the [build prerequisites](docs/development.md#build-prerequisites):

```sh
git clone https://github.com/wesleygrimes/wayshare
cd wayshare
cargo build --release
tools/install.sh
```

This puts `wayshare` in `/usr/local/bin` and asks for sudo. To install somewhere else, see
[Installing from source](docs/development.md#installing-from-source).

## Set up

1. Pick a password. Screen Sharing asks for this user name and password when you connect.

   ```sh
   wayshare set-password --user "$USER"
   ```

2. Tell wayshare which monitor to share. `hyprctl monitors` lists them; replace `DP-1` with
   yours.

   ```sh
   mkdir -p ~/.config/wayshare
   echo 'output = "DP-1"' > ~/.config/wayshare/config.toml
   ```

3. Start it with your session:

   ```sh
   systemctl --user enable --now wayshare
   ```

   On Omarchy that's all. Elsewhere, if wayshare doesn't come back after you log in again,
   add [one line to `hyprland.conf`](docs/troubleshooting.md#the-service-doesnt-start-at-login).

4. On the Mac, open Finder, choose **Go > Connect to Server** and enter `vnc://` followed
   by the Linux machine's address, like `vnc://192.168.1.20`. It also shows up in Finder's
   sidebar under **Network**.

Connection trouble? See [Troubleshooting](docs/troubleshooting.md).

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
systemctl --user daemon-reload
systemctl --user restart wayshare
```

## Uninstall

```sh
systemctl --user disable --now wayshare
```

Then remove the package, or run `tools/install.sh --uninstall` for a source install. Your
password and settings stay in your config directory (usually `~/.config/wayshare`) until you
delete it.

## More

- [Configuration](docs/configuration.md): every setting, keyboard mapping, the sidebar name
- [High Performance mode](docs/high-performance.md): what it needs, encoders, audio
- [Firewall](docs/firewall.md): which ports to open
- [Troubleshooting](docs/troubleshooting.md): logs and the usual problems
- [Packaging](docs/packaging.md): install layout and dependencies, for distro packagers
- [Development](docs/development.md): building, tests, live checks

## License

Apache-2.0. See [LICENSE](LICENSE).
