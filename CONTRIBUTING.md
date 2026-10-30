# Contributing

Thanks for helping improve ClipForge. Contributions should preserve the core workflow: request only the user-selected interval, avoid shell command construction, and never silently fall back to downloading a complete source video.

## Development setup

Install Rust stable, Linux graphics development dependencies for Iced/wgpu, and (for media integration work) `yt-dlp` and FFmpeg. Build and run with `cargo build` and `cargo run`.

## Before submitting

Run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

For media changes, include deterministic fixture-based tests. Do not make tests depend on public video URLs. Report any manual test against content you are authorized to download.

## Pull requests

Explain user-visible behavior and compatibility limits. Keep changes focused, update documentation, and do not add credentials, downloaded media, or generated build output.
