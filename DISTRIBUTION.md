# Linux distribution notes

People can run ClipForge from a GitHub release without installing Rust. Pushing a tag `vX.Y.Z` that matches `version` in `Cargo.toml` builds `clipforge` on Ubuntu 24.04 and attaches `clipforge-X.Y.Z-linux-x86_64.tar.gz` to the release. The binary links against glibc, so it expects Ubuntu 24.04 or a newer distribution such as current Fedora.

yt-dlp and FFmpeg stay external runtime dependencies. Do not bundle them until license obligations, updates, and security provenance are defined. A preview still needs VLC, MPV, or FFplay.

## Formats under evaluation

- **Flatpak:** attractive for desktop distribution, but yt-dlp/FFmpeg availability and network permissions need explicit runtime/extension policy. A sandboxed app should not silently bundle stale media tooling.
- **AppImage:** convenient single-file delivery, but multimedia libraries and platform integration increase bundle size and maintenance. Validate on multiple distributions before release.
- **Native packages:** best fit for using distribution-maintained yt-dlp/FFmpeg and graphics libraries; requires per-distribution packaging and release automation.

Flatpak, AppImage, and per-distribution packages are not release-ready. A Windows binary is not published either: the save dialog, opening an export, history paths, and preview volume still use Linux tools.
