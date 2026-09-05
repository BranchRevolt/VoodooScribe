# Contributing to VoodooScribe

Thanks for considering it. This is a small project with strong opinions; this document explains them
so your pull request does not run into surprises.

## Before you start

- Read **[docs/BUILDING.md](docs/BUILDING.md)** to get a working build.
- Read **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** if you are touching the transcription
  pipeline, model lifecycle or IPC. Several parts look strange until you know why they exist, and
  the document says why.
- For anything larger than a bug fix, open an issue first. It is a shame to write a feature that
  does not fit the project's direction — see [Deliberate non-goals](docs/ARCHITECTURE.md#deliberate-non-goals).

## Ground rules

**The build stays warning-free.** Both `cargo build` and `npx tsc --noEmit` must be clean. This is
not negotiable; warnings accumulate and then nobody reads them.

**Tests must pass.** `cd src-tauri && cargo test` runs 42 offline tests. Add tests for logic you
introduce, especially parsing, formatting and arithmetic. Tests requiring real models are
environment-gated and must keep skipping cleanly when those variables are unset.

**Everything in the codebase is in English** — code, comments, commit messages, documentation, and
the LLM prompts in `src-tauri/resources/prompts/`. User-facing strings are the exception: they go
through i18n with both `en` and `ru` translations.

**Every new source file gets an SPDX header**, matching the existing files:

```rust
// SPDX-FileCopyrightText: 2026 WarpCoreDev
// SPDX-License-Identifier: GPL-3.0-or-later
```

**Heavy work never blocks the UI.** Anything that decodes, resamples, transcribes or generates runs
in `spawn_blocking`, reports progress through a Tauri event, and honours the cancellation flag at
every stage.

**IPC is typed.** Add commands through `src/lib/ipc.ts`. There is no `invoke("some-string")` in this
codebase and there should not be.

**Errors are for humans.** New failure modes get an `AppError` variant with a code and translations
in both `en.ts` and `ru.ts`. "Error code: -6" is what we are trying to avoid.

## Adding user-facing text

Add the key to **both** `src/i18n/en.ts` and `src/i18n/ru.ts`. An untranslated key is a bug, not a
follow-up task.

## Commit messages

Conventional-commit style, imperative mood, and the subject says what changed for the user rather
than which function was edited:

```
fix: derive the silence threshold from the recording instead of hardcoding it
perf: keep the LLM loaded between operations
feat: real app icon, and make it show up under Wayland
```

Prefixes in use: `feat`, `fix`, `perf`, `refactor`, `chore`, `docs`, `test`.

Keep one logical change per commit. If a change touches many files mechanically (a formatting or
header pass), give it its own commit so the substantive diff stays readable.

## Pull requests

Say what you changed, why, and how you verified it. "Tested on Linux with a 40-minute Russian
interview" is worth more than a paragraph of description.

If you build on **Windows or macOS**, please say so explicitly. Neither platform has been built yet,
and a report either way is genuinely useful.

## Reporting bugs

Include:

- OS, GPU and driver (`vulkaninfo --summary` on Linux)
- The model you used and the language setting
- File format and roughly how long the recording is
- What the error toast said — it has a copy button, please use it
- Whether it happens on the first file of a session or only on later ones (that distinction has
  found real bugs before)

Please do not attach recordings containing anything private. A description of the audio is enough.

## Licence

VoodooScribe is GPL-3.0-or-later. By contributing you agree that your contribution is licensed under
the same terms. There is no CLA.
