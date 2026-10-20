# Moonie Cubism production handoff

The supplied Moonie reference and `app/public/moonie.png` are flattened raster images. The current app animates that image with CSS and does **not** contain a Cubism rig or Live2D runtime. This is the outstanding release gate from the product plan.

A finished production asset needs:

1. Original separated artwork: head, ears, upper and lower eyelids, eyeballs, eyebrows, mouth shapes, body, forepaws, hind paws, tail, collar, moon charm, and forehead crescent. Reconstruct areas hidden behind moving pieces. Save a layered PSD and editable source.
2. A Cubism Editor project with ArtMeshes, deformers, parameters for gaze, head angle, blink, breathing, ears, paws, and tail, plus physics where appropriate.
3. Expressions and motions for idle, curious, thinking, focused, reminder, encouraging, celebration, and resting, with short transitions.
4. Runtime exports (`.moc3`, `.model3.json`, texture atlas, expressions, motions, and physics) and an integration of the official Cubism SDK for Web behind a renderer adapter in the transparent Tauri window. The proprietary Core must be obtained and distributed under the applicable Live2D terms.
5. Cubism Viewer checks and macOS/Windows Tauri checks for alpha edges, scaling, hit areas, animation transitions, missing assets, and WebGL recovery.

The Editor requires artwork separation and interactive rigging. A single generated PNG cannot be renamed or converted into a valid editable Cubism model. The current image provides a character direction and UI preview while this production work is pending.
