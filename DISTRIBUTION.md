# Linux distribution notes

ClipForge currently builds from source with Cargo. yt-dlp and FFmpeg are external runtime dependencies; do not bundle them until license obligations, updates, and security provenance are defined.

## Formats under evaluation

- **Flatpak:** attractive for desktop distribution, but yt-dlp/FFmpeg availability and network permissions need explicit runtime/extension policy. A sandboxed app should not silently bundle stale media tooling.
- **AppImage:** convenient single-file delivery, but multimedia libraries and platform integration increase bundle size and maintenance. Validate on multiple distributions before release.
- **Native packages:** best fit for using distribution-maintained yt-dlp/FFmpeg and graphics libraries; requires per-distribution packaging and release automation.

No package format is release-ready. The application must first complete media-operation validation and expose clear dependency diagnostics.
