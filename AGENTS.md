# Cereal: rules for contributors and coding agents

Cereal is a free, open-source, local-first AI meeting recorder inspired by Granola.
It's part of Toolbay, where every tool is a free version of a paid app and opens from the
Toolbay hub. These rules apply to every change; the "Toolbay tool rules" apply to every
Toolbay app (the same list lives in toolbay-flow/AGENTS.md).

## Toolbay tool rules (every Toolbay app)

1. **Consumer-first.** Built for people who never open a terminal. Double-click the installer and
   it works: no Python, CUDA, drivers or model choices, no admin prompt (per-user install).
   Onboarding is plain language and walks through permissions. It runs on an average 8 GB laptop
   with no GPU. It updates itself, recovers from crashes, and every error says what happened and
   what to do, in plain words.
2. **Simplicity first.** The smallest design that works. No setting where a good default will do.
3. **One-step install, models included.** Whatever the app needs to work comes with the one install.
   Notes bundles the speech, notes and speaker engines plus a starter speech model and both speaker
   models in the installer; `toolbay.json` → `files` adds a notes model that works on any computer,
   so a hub install is complete. Better models for the hardware download quietly in the background.
4. **Automatic model choice.** Picks models for the hardware (GPU vendor and memory, RAM, CPU
   cores) in `src-tauri/src/engine.rs` (`recommend`), re-checks when hardware changes. Manual
   choice exists only under Settings > AI models > Advanced.
5. **1:1 parity with the original is the bar.** Every screen, setting, state and flow of the
   original, same order and behaviour. The checklist is `docs/FEATURES.md`; nothing ships as
   "done" while items are missing. Research: `toolbay-research/granola-report.pdf`.
6. **Everything free.** No plans, paywalls, meeting limits, trials or required sign-in.
7. **Local by default, no telemetry.** Recordings, transcripts and notes never leave the computer
   unless the user adds their own cloud key. No analytics, no crash uploads, no phoning home.
   Only settings and custom templates sync, and only through the user's Toolbay account.
8. **Clean-room only.** Rebuild behaviour from observation and public information. Never copy the
   original's code, binaries, sounds, fonts, icons, screenshots or wording.
9. **Branding.** Own logo (`app-icon.svg`); the credit is "Inspired by Granola" with Granola's
   official logo, small and unmodified, plus "Independent open-source project, not affiliated with
   or endorsed by Granola." Both come from `brand.json` → `inspiredBy`; the original's logo appears
   only inside that credit (onboarding, Settings > About, README header).
10. **Theme.** Shared Toolbay tokens (`src/theme/toolbay-theme.css`, same file as the hub) with
    the original app's layout and feel.
11. **Lives under the hub.** `toolbay.json` ships with each release. Supports `--from-toolbay`,
    `--show`, `--hide`, `--quit`, `--start`, `--stop`, `--open <page>`, single instance,
    `running.json` ({pid, version, visible, window, updatedAt, hub, recording}) and the hub's
    `TOOLBAY_TOKEN` (memory only, never written to disk).

## Notes-specific rules

- **No bot joins the call.** Audio comes from two local tracks: the microphone ("You") and the
  computer's sound ("Them": WASAPI loopback on Windows, ScreenCaptureKit on macOS).
- **Never mute, duck or pause other apps' audio.** Shared-mode capture only.
- **Nothing records until the user clicks.** Detection only offers; it never starts by itself.
- **Consent.** Keep the recording-consent reminder and the "let others know" message.
- **Don't fork Toolbay Flow's engine.** Shared engine code lives in `crates/toolbay-engine`
  (catalog, downloads, whisper/llama/sherpa runners, hardware detection, cloud engines). Keep it
  free of Tauri types so Flow can depend on it too. Models/engines another Toolbay tool already
  downloaded are reused read-only (`Store::shared`).

## Map

- `crates/toolbay-engine/` shared engine (no Tauri).
- `src-tauri/src/capture.rs` (+ `capture_mac.rs`) two-track capture on one clock, live chunking.
- `recorder.rs` sessions + live transcription. `process.rs` after-call pipeline: silence
  compaction, full-quality pass, diarization (parallel), echo removal. `notes.rs` templates,
  notes (map-reduce for long meetings), chat. `detect.rs` call detection. `sync.rs` account sync.
  `hub.rs` hub control. `cli.rs` headless test modes.
- `src/app/` React UI (Home, Meeting, Ask, Templates, Settings, Onboarding, Popup, Pill).

## Working on Notes

- `npm install`, then `npm run tauri dev` to run; `npm run tauri build` for the installer (fetches the
  bundled engines and models via `scripts/fetch-bundled.mjs`).
- `cargo test --workspace` must pass.
- Headless (debug build, it has a console): `cereal --record <secs> <dir>` (capture test),
  `--detect` (who's using the mic), `--diarize <wav> <threshold>`, and
  `--process <mic.wav> <system.wav> [template] [--notes "..."]` (full pipeline with timings).
  `CEREAL_DATA=<dir>` uses another data folder; `KEEP=1` keeps the meeting in the app.
- Claude desktop is MSIX: its shells virtualize %APPDATA% writes. Launch real installs via
  `explorer.exe some.bat` when testing the installed app.
- Release: bump `version` in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`,
  tag `vX.Y.Z`; CI builds Windows + macOS, publishes the release and attaches `toolbay.json`.
