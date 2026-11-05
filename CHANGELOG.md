# Changelog

## Unreleased

- Add asynchronous metadata lookup and a quality selector labeled by resolution (for example, 1080p · Full HD).
- Show preview connection, buffering, playback, and completion states while using MPV or FFplay.
- Show export preparation, download, processing, finalization, and outcome with live progress details.
- Polish the create workflow with clearer hierarchy, typography, and button styles.
- Add fixture-backed metadata and quality-selection tests.
- Replace the output filename with the new video's title when loading another video, keeping the chosen folder; reloading the same video keeps a custom name.

## Limitations

Preview opens in external MPV or FFplay; playback controls and seeking are not embedded or synchronized with the timeline. Stream-copy preview can start at a nearby keyframe, and export boundaries are not guaranteed to be frame-exact.
