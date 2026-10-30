# ClipForge engineering context

## Architecture
- Rust 2024 desktop app, Iced 0.14 UI. Keep UI state/update/view separate from domain rules and infrastructure as those arrive.
- Use Iced `Task`/`Subscription` for asynchronous work; never block the UI thread.
- Invoke yt-dlp, FFmpeg, and ffprobe with direct process arguments, never a shell. Treat URL-derived data as untrusted.
- Prefer yt-dlp JSON metadata. Request only the selected interval with `--download-sections` for the primary export path; fetched bytes and cut precision depend on source format/protocol and keyframes. Never silently fall back to downloading the complete source. Stream progress and retain cancellable child process handles.
- Model clip ranges in integer milliseconds/seconds with invariant `0 <= start < end <= duration`.
- Timeline starts with Iced canvas/input events; a bespoke low-level widget requires measured need.
- No embedded player in phase one. Investigate libmpv and GStreamer render-surface integration before selecting a player dependency.
- Default to downloading only content the user owns or is authorized to save; do not bypass DRM or access controls.

## UX decisions
Dark, restrained desktop workspace: persistent navigation, central URL-to-export workflow, metadata/selection card, and downloads history. Preserve clear loading, empty, error, and progress states. Keep keyboard navigation available.

## Development
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build`
- External tools are runtime dependencies; detect their absence and explain installation to the user.
