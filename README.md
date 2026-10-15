# wayshare

Screen Sharing server for Linux (Wayland/Hyprland) that the stock macOS Screen Sharing.app
connects to, including High Performance mode.

## Setup

1. Build and install:

   ```sh
   cargo build --release
   install -m 755 target/release/wayshare ~/.local/bin/wayshare
   ```

2. Store a login. wayshare keeps an SRP verifier (salt and verifier), never the password:

   ```sh
   wayshare set-password --user "$USER"      # prompts; writes ~/.config/wayshare/srp-$USER (mode 0600)
   ```

   Screen Sharing logs in with this user name and password (security type 36, no prompt
   about server identity).

3. Configure: copy `contrib/config.example.toml` to `~/.config/wayshare/config.toml` and set
   `output` to the Wayland output to share. Command-line flags override the file
   (`wayshare --help`).

4. Run as a user service:

   ```sh
   cp contrib/wayshare.service ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user enable --now wayshare
   ```

5. Connect from a Mac: `vnc://<user>@<host>` in Finder (Go > Connect to Server) or Screen
   Sharing.app. For High Performance, choose it in Screen Sharing, or open
   `vnc://<user>@<host>?quality=high&numVirtualDisplays=1`.

## Firewall

| Port | Needed | Why |
|---|---|---|
| TCP 5900 from the LAN | yes | RFB control connection |
| UDP 5353 (mDNS) from the LAN | optional | lets Finder's sidebar and Screen Sharing list the machine |
| UDP 5960/5962 | no | wayshare sends media first, and the viewer's RTCP returns through connection tracking (verified with ufw's default deny on 2026-10-14) |

With ufw:

```sh
sudo ufw allow from 192.168.1.0/24 to any port 5900 proto tcp
sudo ufw allow from 192.168.1.0/24 to any port 5353 proto udp   # optional, Bonjour
```

## Behaviour

- One High Performance viewer at a time. Later viewers get Standard mode, except a reconnect
  from the same address, which takes over.
- If the viewer's RTCP goes silent for 10 s, HP stops and the session falls back to
  Standard mode.
- Every session exit, including a crash or a dropped network (TCP keepalive notices within about
  30 s), releases held keys and buttons, stops HP media and restores the output's mode.
