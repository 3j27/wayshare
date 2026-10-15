# Troubleshooting

Start with the log:

```sh
journalctl --user -u wayshare -f
```

## The service doesn't start at login

The service starts with `graphical-session.target`. Omarchy and other setups that launch
Hyprland through UWSM reach that target; a plain `Hyprland` launch doesn't. Start it from
`hyprland.conf` instead:

```
exec-once = systemctl --user start wayshare
```

## "wayshare needs Hyprland 0.56 or newer"

Upgrade Hyprland to 0.56 or newer.

## The Mac can't connect

- Check the firewall: TCP 5900 has to be open. See [Firewall](firewall.md).
- Screen Sharing allows one window per address. Close any open window for this machine,
  including ones behind other windows, and dismiss any leftover alerts.

## The password is rejected

Set it again and restart:

```sh
wayshare set-password --user "$USER"
systemctl --user restart wayshare
```

## No High Performance

Look for `High Performance mode disabled` or `HP encoder` near the top of the log, then see
[High Performance mode](high-performance.md).

## The machine doesn't show up in Finder's sidebar

mDNS needs UDP 5353 open, and `[mdns] enabled = true` (the default). You can always connect
by address instead.
