# Bugs

Review date: 2026-10-15. Reviewed the initial implementation; the repository has no commits yet.

## Verification

- `cargo test --all-targets`: 154 passed, 1 ignored, 7 failed because the sandbox denied
  socket binds.
- An isolated SRP handshake test confirmed the unknown-user authentication bypass.
- Linux media findings were reviewed statically; no live Linux verification was run.
- No source files were changed during the review.
