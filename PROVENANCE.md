# Provenance of `kh2-combat-core` (clean-room rewrite)

This file records how the code in `src/kh2-combat-core/` was produced, what its author had
access to, and what the method does and does not guarantee.

## Summary

- **Date:** 10 October 2026 (one working session, roughly 20:30–22:00 local time).
- **Author of the code:** an AI coding assistant (Claude, model "Claude Opus 5.5", running
  in Claude Code), acting as the *clean side* of a clean-room rewrite, at the request of the
  project owner.
- **What was written:** the whole library, from an empty folder: the five modules
  (`catalog`, `rules`, `air`, `combo`, `fighter`), their unit tests, and the integration
  tests in `tests/`. No file from the previous implementation was copied, opened or searched.

## Method

The rewrite used two separate roles.

1. **Specification side.** A different session, which knew the previous implementation and
   the notes taken while studying the game program, wrote two documents and prepared the
   data:
   - a functional specification (`SPEC-FUNZIONALE.md`, in Italian): how the combat behaves,
     as conditions and effects, each rule tagged with its source (game data files,
     frame-by-frame recordings of the real game, public community documentation, behaviour
     learned by studying the program and described only as behaviour, or a design choice).
     It contains no code;
   - an interface description (`INTERFACCIA.md`, in Italian): names, types, fields,
     function signatures, events, the catalog file format and the recording columns.
     Declarations only, no function bodies;
   - data: a catalog generated from the game's data files, and four recordings of the real
     game (CSV, one row per frame).
2. **Clean side** (this code). A fresh session, opened in a separate folder that contained
   only the material above plus the working rules. It read the two documents in full, wrote
   the library to match the described behaviour and the exact interface, and wrote its own
   tests starting from the specification. Anything ambiguous or missing was written down as a
   numbered question in a file for the specification side, marked in the code with the
   question number, and implemented with a clearly labelled provisional choice.

The public interface (names, fields, signatures) is intentionally identical to the one the
rest of the project already uses: it was dictated by the interface document, not derived by
the clean side. The internal organisation and every function body are the clean side's own.

The question channel was used in two rounds during the session: the clean side wrote its
questions, the specification side answered in a file (behaviour only, in words) and updated
the two documents accordingly, and the clean side read the answers and the changed passages
of the documents and adjusted the code and tests.

A dated log of every step (what was read, what was done, which tests passed) is kept in
`DIARIO.md` (in Italian), and each milestone is a separate Git commit.

## What the clean side had available

- The working rules (`CLAUDE.md`), `SPEC-FUNZIONALE.md`, `INTERFACCIA.md`, the log
  `DIARIO.md` and the (initially empty) question and answer files.
- `data/catalog/` (the catalog and a human-readable summary of it) and `data/logs/` (the
  four recordings). These are data, not code.
- The Rust toolchain already installed on the PC (`cargo` and `rustc` 1.99.0) and the Rust
  standard library. The library has no external dependencies.
- The tool's permission settings for the folder (`.claude/settings.json`), read once.
- Python, used for one count on the catalog file (overlapping hit zones) and for mechanical
  edits to the session's own test files.

## What it did not have and did not look at

- The previous code of the library, its tests, and the Git history of the main project.
- Any notes taken while studying the game program; any decompiled or disassembled code; any
  Ghidra project or export.
- The game's own folder and files (other than the prepared catalog and recordings above).
- Any folder outside the working folder.
- The web: no searches, no downloads, and in particular no decompilation projects of
  Kingdom Hearts and no code from other projects that reimplement its combat.

No prohibited material was seen during the session; had that happened, the rules required
stopping and recording it in the log, and the log has no such entry.

One slip is recorded in the log for completeness: a temporary helper script, written by the
session itself, was once created in the operating system's temporary folder, outside the
working folder, by a command that then failed. It contained only that script; nothing else
in that folder was read, and the file was deleted immediately.

## Questions

**12 questions** were written to the specification side (`comunicazioni/DOMANDE.md`, D1–D12):
catalog fields that may be omitted (D1, D2), which commands trigger the two
situation-driven moves (D3), a corner case of move selection (D4), two ability flags (D5),
the fall animation frame counter (D6), jump key release (D7), three small edge cases (D8),
overlapping hit zones in the real data (D9, D10), a confirmation about how one expected
count is tallied (D11), and a rare corner case of D3 found while applying its answer (D12).

**All 12 were answered** during the session (R1–R12, `comunicazioni/RISPOSTE.md`) and the
answers were applied. Six confirmed the provisional choice; five replaced it (D2, D3, D9,
D10, D12); one (D8) confirmed it and also corrected a rule. Two of the questions (D8, D9)
pointed out consequences of the written rules on the real data that were not intended, and
the specification itself was corrected as a result. The code comments cite the answer they
follow (`R1`, `R2`, ...).

**No question is open** at the time of writing, and no provisional choice is left in the
code. None of the questions affects the replay of the recordings.

## Results at the end of the session

- `cargo test --manifest-path src/Cargo.toml`: 216 tests passed, 0 failed (181 unit tests on
  small invented catalogs, 24 on the real catalog, 7 on the shape of the interface, 4 on the
  recordings). Tests that need the real data skip themselves when the data is absent.
- Replay of the recordings of the real game, combo decisions (a move starts, a press is
  ignored, a move ends): **0 differences** over the 128 combos of the two recordings that
  have the required columns, with 133 matching continuations.
- Jumps: 45 compared, 36 within 2 units and 42 within 5 at every frame. Aerial lunges with a
  distant target: 28 of 30 cover the same distance within 8%. Both match the figures the
  specification expected.
- The first draft of the library reproduced the recordings with 0 differences without any
  change to the library code, and the result was the same after the answers were applied.
  As a check that this result is not vacuous, four deliberate one-line errors were
  introduced one at a time and then reverted: each made the replay tests fail (three
  changed the combo decisions, one the jump heights).

## What this does and does not guarantee

This was procedural isolation on the same PC (a separate folder, a fresh session, permission
rules that block reads outside it), not a sandbox enforced by the operating system, and it
is no guarantee about the AI model's training data.

In more detail:

- The separation relied on the session following its rules and on the tool's permission
  settings (reads outside the working folder blocked, web access denied). It was not
  enforced by the operating system, a virtual machine or a separate computer.
- The specification and interface documents were written by a session that knew the previous
  implementation. That is the purpose of the specification side, but it means the documents
  themselves are the channel through which knowledge passed, and they should be judged on
  their own content.
- The AI model has general knowledge from its training. Nothing in this process can show
  what that training did or did not include.
- The data files (catalog and recordings) derive from the game and are not part of this
  repository's source code.
