Phase 4

Then:

Implement Subtitle Engine.

Input

story.mp3

Output

story.srt

Use whisper-rs.

Return timestamps.

Make it asynchronous.

Production ready.

Phase 5
Implement Background Engine.

Scan

assets/backgrounds/

Randomly select one.

Read video duration.

Loop if necessary.

Return background path.

Phase 6
Implement Render Engine.

Use FFmpeg.

Input

Background

Narration

Subtitle

Music

Logo

Output

1080x1920 mp4

60 fps

H264

AAC

Use hardware acceleration if available.

Everything configurable.


Phase 7
Implement Tauri UI.

Home page.

Story type.

Generate button.

Render progress.

Video preview.

Export button.

Dark theme.

Minimal UI.
