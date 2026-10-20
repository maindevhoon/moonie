# Moonie: Animated Desktop Companion MVP

## 1. Product and corrected stack

Build a Windows and macOS desktop app centered on Moonie, your animated cream cat with its golden crescent. Users capture thoughts or paste a webpage, review proposed tasks, and receive one clear next action with gentle reminders.

The first release is a team demo using your **OpenRouter key** and a separate **TinyFish key**.

| Layer | Choice | Responsibility |
|---|---|---|
| Desktop shell | Tauri 2 | Transparent mascot window, task panel, tray and shortcuts |
| Interface | React + TypeScript + Vite | Task capture, review, task list and settings |
| Animated mascot | Live2D Cubism SDK for Web | Render the custom Moonie rig using WebGL |
| Application logic | Rust inside Tauri | Task validation, priorities, reminders and API requests |
| Storage | SQLite through Rust | Approved tasks, dependencies, reminder state and preferences |
| AI | OpenRouter | Turn thoughts and extracted webpage details into task drafts |
| Web retrieval | TinyFish | Retrieve relevant information from a user-provided public URL |
| Secrets | OS credential storage | Store API keys outside SQLite and frontend storage |

**Replace FastAPI with Tauri’s Rust backend.** This app does not need a separate Python server, local HTTP port or packaged Python runtime. Rust commands provide the interface between React and local services.

Use the official Cubism Web SDK directly, behind a small renderer adapter. Its framework and proprietary Core have distinct distribution arrangements, which must be handled during setup. [Cubism SDK documentation](https://docs.live2d.com/en/cubism-sdk-manual/cubism-sdk-for-web/)

## 2. First milestone: complete Moonie’s custom rig

**Finish the mascot before starting the product application**, as requested. The workspace currently contains reference materials and the architecture output, but no app implementation or Live2D model.

### Artwork and rig

- Create original, separated artwork based on the supplied Moonie reference: cream fur, golden forehead crescent, dark collar, expressive eyes and fluffy tail.
- Prepare a layered PSD with separate eyes, eyelids, mouth, ears, head, body, paws, tail and accessories. Reconstruct areas hidden behind moving parts.
- Import the artwork into Cubism Editor, create meshes and deformers, and rig blinking, breathing, head tilt, gaze, ear movement and tail sway.
- Preserve the editable art and Cubism project alongside runtime exports: model, textures, physics, expressions and motions. [Live2D file formats](https://docs.live2d.com/en/cubism-editor-manual/file-type-and-extension/)

The agent owns artwork preparation, integration and validation. Cubism rigging is an explicit production step; it may require hands-on editor work. A generated PNG or renamed JSON file does not satisfy this milestone.

### Required animation states

| State | Trigger and behavior |
|---|---|
| Idle | Gentle breathing, blinking and occasional tail movement |
| Curious | User opens the panel or enters a thought |
| Thinking | An OpenRouter or TinyFish request is running |
| Focused | User starts a focus session |
| Reminder | A scheduled task becomes due |
| Encouraging | User accepts a task or resumes work |
| Celebration | User completes a task |
| Resting | User activates a break or quiet mode |

Use brief transition motions and return to the appropriate persistent state. Avoid continuous attention-seeking movement.

### Exit gate

Validate the custom rig in Cubism Viewer and a minimal transparent Tauri rendering harness on Windows and macOS. Check animation transitions, alpha edges, scaling, hit areas and rendering stability.

The harness is only for mascot validation. Product development begins after this gate passes. If rigging cannot be completed with the available tools, identify the exact missing editor work and deliver the prepared assets for that step; do not silently substitute sprites.

## 3. MVP behavior and interfaces

### Desktop experience

- Create two windows: a compact, frameless mascot window and a separate task panel.
- Let users drag Moonie, remember its position and restore it within a connected monitor’s bounds.
- Clicking Moonie opens the panel. Use `Ctrl+Shift+Space` on Windows and `Command+Shift+Space` on macOS, with a configurable shortcut if registration conflicts.
- Include tray actions for opening the panel, hiding Moonie, quiet mode and quitting.
- Keep the application running when the panel closes. Explicit Quit stops reminders.
- Provide mascot size, reduced motion and animation pause settings. Default to a 30 FPS animation cap and pause rendering while hidden.

Tauri supplies desktop shortcut and notification facilities; native behavior still needs verification on both target platforms. [Shortcuts](https://v2.tauri.app/reference/javascript/global-shortcut/), [notifications](https://v2.tauri.app/plugin/notification/)

### Task workflows

**Brain dump:** entered text → Rust → OpenRouter → validated draft → user edits and approves → SQLite.

**Web to task:** public URL → Rust → TinyFish → relevant page details and source URL → OpenRouter → validated checklist → user approval → SQLite.

**Manual task:** task form → user saves → validation → SQLite. This remains usable offline.

Drafts include task titles, optional notes, dates, estimated duration and dependencies. Missing or ambiguous dates remain unset and visibly require review. Preserve source links for webpage-derived tasks.

Support editing, completion, deletion, snoozing, scheduled tasks, flexible tasks and simple daily/weekly recurrence.

### Priorities and reminders

Implement deterministic selection in Rust:

1. Exclude completed tasks and tasks blocked by unfinished prerequisites.
2. Prefer tasks scheduled for now, then overdue tasks, then upcoming deadlines.
3. Consider estimated duration when the user supplies available time.
4. Use a stable creation-order tie-breaker and display a short explanation.

Keep reminder scheduling in Rust so it survives panel closure. Persist delivery state to prevent duplicate alerts. Reconcile missed reminders after sleep or restart with a single catch-up summary.

### Application interfaces

Expose typed Tauri commands for:

- Creating and cancelling task-draft requests.
- Approving drafts and managing tasks.
- Getting the next suggested task.
- Managing focus sessions, preferences and credentials.

Emit task-change, request-progress, reminder and mascot-state events. Both windows obtain authoritative state from Rust rather than maintaining separate task stores.

Use SQLite migrations from the first version. Separate task drafts from approved tasks; approval saves a batch transactionally.

### API integration

- Add OpenRouter key validation and a model selector filtered for structured-output support. Require a model selection during setup and persist its exact ID.
- Use schema-constrained responses and validate results again in Rust. OpenRouter exposes model capabilities through its model catalogue. [Models API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- Implement TinyFish through a dedicated adapter, including progress, timeout and cancellation handling. [Official examples](https://github.com/tinyfish-io/tinyfish-cookbook)
- Keep credentials and authenticated API calls in Rust. Never include keys in logs.
- Treat webpage content as source material, not executable instructions. The MVP retrieves information and proposes tasks; it does not submit applications or operate external accounts.
- Preserve user input on authentication failures, rate limits, malformed responses or network errors. Offer explicit retry and manual editing without creating duplicate tasks.

## 4. Delivery order and acceptance tests

| Milestone | Deliverable and acceptance gate |
|---|---|
| 1. Mascot | Custom artwork, editable rig and runtime exports pass the two-platform rendering harness |
| 2. Desktop shell | Moonie floats, drags, opens the panel and responds to tray controls and shortcuts |
| 3. Local task system | Manual tasks, persistence, dependencies, priorities and reminders work offline |
| 4. AI capture | OpenRouter produces editable drafts; only approved tasks enter the task list |
| 5. Web capture | TinyFish turns a public listing into a sourced, reviewable checklist |
| 6. Demo packaging | Installable Windows and macOS builds complete the end-to-end demonstration |

Test:

- Every mascot state, repeated transitions, missing assets and WebGL failure recovery.
- Monitor disconnection, display scaling, saved window position and shortcut conflicts.
- Ambiguous dates, time zones, recurrence, dependency cycles and blocked tasks.
- Approval, rejection, edited drafts, double-clicked approval and restart persistence.
- API failures, cancellation, invalid structured output and hostile webpage text.
- Reminder delivery after sleep, notification permission denial and quiet mode.
- No API keys in browser storage, the task database or logs.

The final demo must show: **open Moonie → paste an internship URL → review a checklist → approve → receive a next-step suggestion → complete a task → see Moonie celebrate**.

## 5. Defaults and boundaries

- Target Windows and macOS; use platform build jobs and native smoke tests.
- Follow the reference’s cream, lavender, slate and gold visual palette.
- Ship one bundled custom mascot. User-imported Live2D models are outside the MVP.
- Accounts, cloud sync, Gmail, Google Calendar, voice input and screen monitoring are later features.
- Focus and break modes are user-controlled; Moonie reacts to app events rather than observing unrelated applications.
- Distribution initially targets team testing. Confirm applicable Live2D release terms before broader distribution. [Live2D release licensing](https://www.live2d.com/en/sdk/license/)
- Update the architecture diagram and project documentation to show **Live2D + Tauri/Rust + OpenRouter**, replacing FastAPI and the unspecified LLM block.
