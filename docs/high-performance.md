# High Performance mode

High Performance is Screen Sharing's low-latency mode. It streams HEVC video and audio over
UDP and adjusts the bitrate to your network. Connections that don't ask for it get the
regular Standard mode.

## Connecting

Open this from the Mac, with your machine's address:

```
vnc://192.168.1.20?quality=high&numVirtualDisplays=1
```

## What it needs

wayshare picks an encoder when it starts:

1. **VA-API**, when your GPU's driver can encode HEVC. Needs `libva` and the driver.
2. **x265** otherwise. Needs `libx265` with API build 217.

The log says which one it chose:

```
HP encoder backend=Vaapi
```

If neither works, wayshare still runs in Standard mode and logs why:

```
High Performance mode disabled: no hardware HEVC encoder, and the libx265 fallback is unusable: ...
```

NVIDIA's NVENC isn't supported yet, so NVIDIA machines use x265.

Audio needs `libfdk-aac` and a PulseAudio server (PipeWire's `pipewire-pulse` works). Without
them video still works, and the log says `HP audio stopped`.

## How it behaves

- One High Performance viewer at a time. Later viewers get Standard mode, except a reconnect
  from the same address, which takes over.
- If the viewer goes quiet for 10 seconds, the session falls back to Standard mode.
- When a session ends, even from a crash or a dropped network, wayshare releases held keys
  and buttons, stops the stream and restores the monitor.
