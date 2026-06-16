# LOOP PLAN — vi-mode fixup

## Done
- [x] Config/CLI plumbing: `InputMode` enum, `--input-mode`, config field, default emacs
- [x] Mode indicator in status bar (`-- NORMAL --`, `-- INSERT --`, `-- VISUAL --`)
- [x] Normal mode: all core motions (h/j/k/l/w/b/e/W/B/E/0/$/^/gg/G/{/}/%/f/F/t/T/;,)
- [x] Normal mode: insert entry (i/I/a/A/o/O/s/S)
- [x] Normal mode: operators (d/y/c/</>) with motions, counts, text objects
- [x] Normal mode: x/X/p/P/J/~, marks, repeat (.)
- [x] Visual mode: v/V/Ctrl+V, motions extend selection
- [x] Visual mode: d/c/y/</>/~/u/U operators
- [x] Command-line mode: : / / / ?
- [x] Registers (a-z, 0-9, "", "0, "+, "1-"9)
- [x] Text object resolution (iw/aw/iW/aW/ip/ap/is/as/ib/aB/iB/aB/i"/a"/i'/a')
- [x] Undo/redo (u/Ctrl+R) with composite insert-session undo
- [x] Feature gate (`vi-mode` cargo feature, default off)
- [x] All 450 tests pass

## Fixes Needed (from code review)

### 🔴 Blocking
- [x] **Paste (`p`) placement off by one** — `cursor += 1` -> `next_char_boundary()`. Also fixed same bug in repeat `.` paste handler.
- [x] **`%` at position 0 panics** — `cursor.wrapping_sub(1)` wraps to `usize::MAX` when cursor is 0, causing OOB in `match_bracket`. Add early-return guard.
- [x] **Normal-mode + visual-mode ops not undoable** — Added `push_undo()` after all buffer-mutating normal/visual ops: `x`, `D`, `dd`, `>>`, `<<`, `p`, `P`, `J`, `~`, `.`, visual `d`, visual `~`. Also implemented previously-missing `x` (delete char under cursor). Simplified `X` to use same `insert_start_buf` pattern as everything else.
- [x] **Visual selection not highlighted** — Added `vi_selection: Option<(usize, usize)>` param to `draw_bottom()`. Input loop passes `vi_visual_selection()` (returns `Some(range)` in visual mode). Renderer writes each char individually with reverse-video for selected range.

### 🟡 Should Fix
- [x] **`t`/`T` motions broken** — Added `pending_fchar_is_t` / `last_fchar_is_t` flags to ViState. `t`/`T` now correctly set these and dispatch to `"t"`/`"T"` motions (cursor before/after the target char). `;` and `,` repeat respect `last_fchar_is_t`. Also fixed pre-existing borrow-checker error in `push_undo` calls by extracting `commit_vi_undo()` helper.
- [x] **Visual mode ignores `pending_count`** — `5j` works in normal mode but not in `v5j`. Visual motions should read and consume `pending_count`.
- [x] **`cmdline_buffer.pop()` breaks on multi-byte UTF-8** — `pop()` removes last byte, not last char. Use `truncate()` at char boundary instead.
- [x] **Missing tests for `vi.rs`** — 280+ lines of motion/operator/match_bracket/resolve_text_object logic with zero tests. Every function needs unit tests.

### 🟢 Nits
- [x] `unwrap_or("").to_string()` → `unwrap_or_default()` in vi-related register access
- [x] Replace dead backward-search branch with forward open-bracket search in `resolve_text_object::Brackets`
- [x] Verify `apply_motion("j"/"k")` — safe on empty buffer
- [ ] **WONTFIX**: `#![allow(dead_code)]` in vi.rs and `#[allow(dead_code)]` on `VisualType::Block` — `TextObject`, `resolve_text_object`, and `ViOperator::IndentRight/Left` are genuinely dead code; the annotation is correct.

## Build & Test
- [x] `cargo fmt` passes
- [x] `cargo test` passes (450 tests with `vi-mode` feature)
- [x] `cargo build` no warnings with `vi-mode` feature
- [x] Pre-existing compilation error in `main.rs:827` without `vi-mode` feature (unrelated `mut`)
