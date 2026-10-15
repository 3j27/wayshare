# Bugs

Review date: 2026-10-15. Reviewed the initial implementation; the repository has no commits yet.

## P1: Unknown users can authenticate with an empty password

Location: `src/rfb/handshake.rs:214`.

The SRP handshake generates a decoy verifier from an empty password for unknown users,
then accepts a matching proof without rejecting the unknown username. An isolated test
returned authentication success (`0`) for `bob` with an empty password.

Unknown users must fail authentication regardless of proof validity.

## P2: Unauthenticated connections hide the host cursor

Location: `src/rfb/session.rs:49`.

The session creates the cursor source before authentication. The Linux factory immediately
sets Hyprland's global cursor visibility. Repeated unauthenticated connections can keep
the cursor hidden.

Defer that side effect until authentication succeeds.

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
