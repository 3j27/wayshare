# Bugs

Review date: 2026-10-15. Reviewed the initial implementation; the repository has no commits yet.

## P2: Failed HP renegotiation leaves framebuffer updates paused

Location: `src/rfb/session.rs:702`.

The configuration failure path sends an error without clearing `hp_restarting` or
`media_active`. After a resize stops HP, a configuration failure therefore prevents both
Standard framebuffer updates and automatic fallback.

Clear the restart state and resume Standard updates when renegotiation fails.

## P2: HP fallback leaves audio streaming

Location: `src/rfb/session.rs:871`.

The fallback path switches to Standard without calling `hp.stop(self.id)`. The retained
`Media` still owns a running audio sender, so capture and UDP transmission continue until
disconnect or replacement.

Stop the session's HP backend when falling back to Standard.

## Verification

- `cargo test --all-targets`: 154 passed, 1 ignored, 7 failed because the sandbox denied
  socket binds.
- An isolated SRP handshake test confirmed the unknown-user authentication bypass.
- Linux media findings were reviewed statically; no live Linux verification was run.
- No source files were changed during the review.
