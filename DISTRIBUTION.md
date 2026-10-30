# Linux distribution notes

People can run ClipForge from a GitHub release without installing Rust. In Actions, open the Release workflow and choose Run workflow. An empty version uses `version` in `Cargo.toml`; the workflow creates the `vX.Y.Z` tag and attaches two packages:

- `clipforge-X.Y.Z-linux-x86_64.AppImage`, built on Ubuntu 24.04. It needs that glibc or newer, which current Fedora releases provide. Mark it executable and run it. `--appimage-extract-and-run` avoids a FUSE dependency.
- `clipforge-X.Y.Z-windows-x86_64.zip`, containing `clipforge.exe`.

yt-dlp and FFmpeg stay external runtime dependencies. Do not bundle them until license obligations, updates, and security provenance are defined. A preview still needs VLC, MPV, or FFplay on `PATH`.

## Formats under evaluation

- **Flatpak:** attractive for desktop distribution, but yt-dlp/FFmpeg availability and network permissions need explicit runtime/extension policy. A sandboxed app should not silently bundle stale media tooling.
- **AppImage:** convenient single-file delivery, but multimedia libraries and platform integration increase bundle size and maintenance. Validate on multiple distributions before release.
- **Native packages:** best fit for using distribution-maintained yt-dlp/FFmpeg and graphics libraries; requires per-distribution packaging and release automation.

Flatpak and per-distribution packages are not release-ready. The AppImage is a single file around the same binary; it does not bundle yt-dlp, FFmpeg, or the GPU drivers.
