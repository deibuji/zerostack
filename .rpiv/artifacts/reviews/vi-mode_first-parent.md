---
template_version: 2
date: 2026-06-17T20:10:28+00:00
author: exedev
repository: zerostack
branch: vi-mode
commit: 91f81af
review_type: pr
scope: "first-parent vi-mode vs main"
scope_strategy: first-parent
in_scope_files_count: 16
status: ready
severity: { critical: 7, important: 6, suggestion: 4 }
verification: { verified: 17, weakened: 0, falsified: 1 }
blockers_count: 13
tags: [code-review, vi-mode, input, tui, rust]
---

# Code Review — vi-mode input system

**Commit:** `91f81af` · **Status:** `ready` · **Findings:** 7🔴 · 6🟡 · 4🔵 · **Verification:** 17✓ / 0− / 1✗

## Top Blockers

1. **I1** — `--input-mode vi` is accepted, but the vi key path is compiled out unless the `vi-mode` cargo feature is enabled. Currently `vi-mode` is **not** in the `default` feature set, so the default build silently falls back to emacs.
2. **I3** — `apply_vi_motion_or_op` consumes `pending_op` with `.take()` before checking whether `apply_operator`/`apply_motion` returns `None`; unsupported operators/motions vanish without feedback.
3. **Q1** — Forward search in `handle_vi_search_key` can panic when the cursor is at the end of the buffer (`self.buffer[self.cursor + 1..]`).
4. **Q16** — Backward `T` motion can panic at cursor position 0 because `cursor.wrapping_sub(1)` produces `usize::MAX` and indexes `chars[usize::MAX]`.

---

## Legend

```text
Severity    🔴 fix before merge   🟡 fix soon   🔵 nice to have   💭 discuss
ID prefix   I interaction   Q quality   S security   G gap
Verify      ✓ verified   − weakened (demoted)   ✗ falsified (dropped)
Annotate    [precedent-weighted]   [cascade: <kind>]   [subsumed-by <ID>]
```

---

## 🔴 Critical

### I1 🔴 Feature gating false-promise: `--input-mode vi` is silently ignored in default builds

**Where**
`src/cli.rs:120` / `src/config/mod.rs:171` / `Cargo.toml:14`

**Code**
```rust
src/cli.rs:120
    pub input_mode: Option<String>,

src/config/mod.rs:171
    pub input_mode: Option<types::InputMode>,

src/ui/input/mod.rs:251
        #[cfg(feature = "vi-mode")]
        if self.input_mode == InputMode::Vi {
            return self.handle_vi_key(key);
        }

Cargo.toml:14
    default = ['loop', 'git-worktree', 'mcp', 'subagents', 'archmd', 'status-signals']
```

**Why**
`InputMode::Vi` and the `--input-mode vi` CLI flag are compiled unconditionally, but the code path that actually implements vi handling is behind `#[cfg(feature = "vi-mode")]`. The `vi-mode` feature is declared but is **not** in the `default` feature set, so a normal `cargo install --path .` or `cargo test` build silently falls back to emacs. This is a cross-layer false promise: the CLI/config surface accepts vi mode while the runtime feature is absent.

**Fix**
Add `'vi-mode'` to the `default` feature list in `Cargo.toml`, or gate the CLI/config vi option behind the same feature and emit a warning/error when it is unavailable.

---

### I3 🔴 Operator state is lost when `apply_operator` or `apply_motion` returns `None`

**Where**
`src/ui/input/mod.rs:1704` / `src/ui/input/vi.rs:811` / `src/ui/input/vi.rs:857,877,903,923`

**Code**
```rust
src/ui/input/mod.rs:1704
        if let Some(op) = self.vi_state.pending_op.take() {

src/ui/input/mod.rs:1706
            if let Some((new_buf, new_cursor, deleted)) = apply_operator(
                &self.buffer,
                self.cursor,
                op,
                motion,
                count,
                &mut self.vi_state,
            ) {

src/ui/input/vi.rs:857
            _ => None,
```

**Why**
`pending_op` is consumed with `.take()` before the result of `apply_operator` is inspected. If the operator arm returns `None` (e.g., `IndentRight`/`IndentLeft` fall through to `_ => None`), the pending operator is gone and no feedback is given to the user. The same pattern applies to `apply_motion` returning `None`. A dropped operator is a silent command failure.

**Fix**
Check the result of `apply_operator`/`apply_motion` before consuming `pending_op`, or restore `pending_op` on `None` and log/feedback the unsupported operation.

---

### Q1 🔴 Forward search can panic when cursor is at end of buffer

**Where**
`src/ui/input/mod.rs:1388`

**Code**
```rust
src/ui/input/mod.rs:1388
                    if let Some(pos) = self.buffer[self.cursor + 1..].find(search) {
```

**Why**
When the cursor is at the last position of the buffer (`self.cursor == self.buffer.len()`), `self.cursor + 1` exceeds the valid string index range and `self.buffer[self.cursor + 1..]` panics. This is the same class of bug as the `%` panic fixed in commit `c63492c`.

**Fix**
Use `self.buffer[self.cursor.saturating_add(1).min(self.buffer.len())..]` or guard the slice before indexing.

---

### Q16 🔴 Backward `T` motion panics at cursor position 0

**Where**
`src/ui/input/vi.rs:737`

**Code**
```rust
src/ui/input/vi.rs:737
        "T" => {
            if let Some(ch) = vi.last_fchar {
                let mut i = cursor.wrapping_sub(1);
                loop {
                    if chars[i] == ch {
                        return Some(i + 1);
                    }
                    if i == 0 { break; }
                    i -= 1;
                }
            }
            None
        }
```

**Why**
When `cursor == 0`, `cursor.wrapping_sub(1)` becomes `usize::MAX`. The next iteration reads `chars[usize::MAX]`, causing an out-of-bounds panic. This is the same `wrapping_sub` pattern that caused the `%` panic at cursor 0 (commit `c63492c`).

**Fix**
Return `None` early if `cursor == 0` before calling `wrapping_sub(1)`.

---

### Q18 🔴 Multi-byte text object truncates buffer because `len` is a char count used as byte index

**Where**
`src/ui/input/vi.rs:539`

**Code**
```rust
src/ui/input/vi.rs:539
                let text = &buf[..len];
```

**Why**
`len` is set to `chars.len()` (the number of Unicode characters) but is used as a byte index into `buf`. For multi-byte UTF-8 text, the byte count is larger than the char count, so `&buf[..len]` truncates the string before the actual end.

**Fix**
Use the byte length of the string (`buf.len()`) or convert the char index to a byte boundary before slicing.

---

### I4 🔴 [precedent-weighted] Visual `h`/`l` ignore the accumulated count while other visual motions honor it

**Where**
`src/ui/input/mod.rs:1438` / `src/ui/input/mod.rs:1458` / `src/ui/input/vi.rs:596`

**Code**
```rust
src/ui/input/mod.rs:1438
            KeyCode::Char('h') | KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                }
                None
            }

src/ui/input/mod.rs:1458
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => {
                if self.cursor < self.buffer.len() {
                    self.cursor = next_char_boundary(&self.buffer, self.cursor);
                }
                None
            }

src/ui/input/vi.rs:596
        "h" | "left" => {
            let mut c = cursor;
            for _ in 0..cnt {
                if c > 0 {
                    c = prev_char_boundary(buf, c);
                }
            }
            Some(c)
        }
```

**Why**
Visual mode hand-rolls `h`/`l` movement and ignores the `count` extracted at the top of `handle_vi_visual_key`. Other visual motions (`j`, `k`, `w`, `b`, `e`) pass `count` to `apply_motion`. Worse, the lower-level `apply_motion("h")` does honor `count`, so normal-mode `5h` works but visual-mode `5h` does not. This is a contradictory-assumption defect within the same state machine.

**Fix**
Make visual `h`/`l` call `apply_motion` with `count`, or replicate the `for _ in 0..count` loop.

**Note:** Precedent weighting applied because both the vi-mode follow-up commits and the earlier Emacs-input CJK cursor fix touched this exact multi-byte/cursor-math area.

---

### I5 🔴 [precedent-weighted] `t`/`T` motions ignore count while `f`/`F` honor it

**Where**
`src/ui/input/vi.rs:725` / `src/ui/input/vi.rs:737` / `src/ui/input/vi.rs:690`

**Code**
```rust
src/ui/input/vi.rs:690
        "f" => {
            if let Some(ch) = vi.last_fchar {
                let mut c = cursor + 1;
                let mut found = 0u32;
                while c < len {
                    if chars[c] == ch {
                        found += 1;
                        if found >= cnt as u32 {
                            return Some(c);
                        }
                    }
                    c += 1;
                }
            }
            None
        }

src/ui/input/vi.rs:725
        "t" => {
            if let Some(ch) = vi.last_fchar {
                let mut c = cursor + 1;
                while c < len {
                    if chars[c] == ch {
                        return Some(c.saturating_sub(1));
                    }
                    c += 1;
                }
            }
            None
        }
```

**Why**
`f` loops until the `cnt`-th occurrence, but `t` returns on the first occurrence. `T` also ignores `cnt` and, combined with the `cursor.wrapping_sub(1)` issue, panics at cursor 0. The user expects `3t(` to find the 3rd occurrence, not the 1st.

**Fix**
Add a `found` counter to `t`/`T` matching the `f`/`F` implementation, and guard `T` against `cursor == 0`.

**Note:** Precedent weighting applied because `t`/`T` were explicitly fixed in the vi-mode follow-up commits and the same code area had multi-byte/cursor regressions before.

---

## 🟡 Important

### I2 🟡 Visual `>`/`<` are no-ops and `IndentRight`/`IndentLeft` are unimplemented

**Where**
`src/ui/input/mod.rs:1540` / `src/ui/input/mod.rs:1545` / `src/ui/input/vi.rs:33-34` / `src/ui/input/vi.rs:857`

**Code**
```rust
src/ui/input/mod.rs:1540
            KeyCode::Char('>') => {
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }

src/ui/input/mod.rs:1545
            KeyCode::Char('<') => {
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }

src/ui/input/vi.rs:33
    IndentRight,
    IndentLeft,

src/ui/input/vi.rs:857
            _ => None,
```

**Why**
Visual `>` and `<` switch back to Normal mode without performing any indent/unindent. The `ViOperator::IndentRight`/`IndentLeft` variants exist in `vi.rs` but every arm in `apply_operator` falls through to `_ => None`, so the operator is unreachable. The visual selection is silently discarded.

**Fix**
Either implement visual indent/unindent in `handle_vi_visual_key` or remove the `IndentRight`/`IndentLeft` variants until they are implemented. If left as stubs, they should at least provide feedback.

---

### Q22 🟡 `input-mode` parse errors are silently dropped

**Where**
`src/cli.rs:334` / `src/cli.rs:342`

**Code**
```rust
src/cli.rs:334
            .and_then(|s| s.parse().ok())
```

**Why**
`parse().ok()` converts `Err` to `None`. An invalid `--input-mode` value silently falls back to the config default (emacs) with no warning. While clap's `value_parser` currently limits the input to `["emacs", "vi"]`, the resolver is fragile if that restriction is ever relaxed or used elsewhere.

**Fix**
Use `parse().expect(...)` or return a clear error message for invalid input modes.

---

### Q6 🟡 Marks `m` is a stub that never records a mark

**Where**
`src/ui/input/mod.rs:1303`

**Code**
```rust
src/ui/input/mod.rs:1303
            KeyCode::Char('m') => {
                // Set mark: wait for next char a-z
                // For now, store at marks[0] as placeholder
                None
            }
```

**Why**
The `m` handler waits for a mark character but does not store it. The mark infrastructure is present but the recording path is missing.

**Fix**
Store the mark in `vi_state.marks` keyed by the following character, or remove the keybinding until marks are implemented.

---

### Q7 🟡 Jump-to-mark `'` / `` ` `` is a stub that never jumps

**Where**
`src/ui/input/mod.rs:1308`

**Code**
```rust
src/ui/input/mod.rs:1308
            KeyCode::Char('\'') | KeyCode::Char('`') => {
                // Jump to mark: wait for next char a-z
                None
            }
```

**Why**
The jump-to-mark handler waits for a character but does not read the mark map or move the cursor.

**Fix**
Look up the mark in `vi_state.marks` and move `self.cursor` to the recorded position, or remove the keybinding.

---

### Q8 🟡 `:`-mode quit/write commands are silently ignored

**Where**
`src/ui/input/mod.rs:1613`

**Code**
```rust
src/ui/input/mod.rs:1613
                    "q" | "quit" => {
                        // Quit
                    }
                    "w" | "write" => {
                        // Session save is handled externally
                    }
                    "wq" => {
                        // Save and quit
                    }
```

**Why**
The command-line mode recognizes `:q`, `:w`, `:wq`, etc., but the bodies are empty. The user gets no feedback that the command is unsupported or was ignored.

**Fix**
Either wire the commands to the appropriate session actions or return an error/info message saying these commands are not yet implemented.

---

### Q19 🟡 Quote text object skips a cursor sitting exactly on a quote

**Where**
`src/ui/input/vi.rs:460`

**Code**
```rust
src/ui/input/vi.rs:460
            TextObject::Quotes(q) => {
                let mut start = cursor;
                while start > 0 {
                    start -= 1;
                    if chars[start] == q {
                        break;
                    }
                }
```

**Why**
The search starts by decrementing `start` to `cursor - 1`, so a cursor positioned directly on the opening quote never inspects that character. This contradicts the expected behavior of text objects in vi.

**Fix**
Check `chars[cursor]` first before scanning backward, or use a helper that includes the current cursor position.

---

## 🔵 Suggestions

### Q15 🔵 No integration tests exercise the `InputEditor` vi handlers

**Where**
`src/ui/input/mod.rs:1413`

**Fix**
Add tests that dispatch `KeyEvent`s through `InputEditor::handle_key` for normal, visual, command-line, and search modes, and for `commit_vi_undo`. The current `vi_tests.rs` only tests the pure `vi.rs` helpers.

---

### PM1 🔵 Peer mirror: `vi_tests.rs` lacks a mixed-stream multibyte typing test

**Where**
`src/tests/input_tests.rs:39` / `src/tests/vi_tests.rs:1068`

**Fix**
Mirror the `typing_mixed_ascii_and_multibyte` test from `input_tests.rs` in `vi_tests.rs` to exercise insert-mode typing and cursor math in vi mode.

---

### PM2 🔵 Peer mirror: `vi.rs` lacks `clear_buffer` and `vi_visual_selection` helpers

**Where**
`src/ui/input/mod.rs:79` / `src/ui/input/mod.rs:1590`

**Fix**
Consider adding a `ViState::clear()` helper and a `visual_range()` helper so the pure vi module can be tested and used without relying on `InputEditor` orchestration.

---

### S1 🔵 Explicit-trust rendering: user input chars written directly to TTY

**Where**
`src/ui/renderer.rs:955`

**Code**
```rust
src/ui/renderer.rs:955
                        write!(stdout, "{}", ch)?;
```

**Why**
The renderer writes characters from the input line directly to the terminal. While this is normal for a TUI, a malicious or pasted control character (e.g., U+001B) could be interpreted as an ANSI escape sequence by the terminal. This is a low-confidence security concern because the user is also the viewer of the terminal, but it should be reviewed if the input line ever displays untrusted text.

**Fix**
Filter or escape control characters (especially ESC) when rendering user input, or verify that the terminal is in a mode that safely handles them.

---

## 💭 Discussion

### PT1 💭 `ViState::cmdline_prompt` is dead code in production

**Where**
`src/ui/input/vi.rs:210` / `src/ui/input/mod.rs:769`

**Why**
`cmdline_prompt` has a catch-all `_ => ":"` arm that returns `":"` for `Insert`, `Normal`, and `Visual` modes, which is semantically wrong. Production code uses `InputEditor::update_vi_mode_label` for the mode label and never calls `cmdline_prompt`. The function is only exercised by tests. This is intentional dead code (the file begins with `#![allow(dead_code)]`), but the catch-all is misleading and could confuse future maintainers.

**Fix**
Either remove `cmdline_prompt`, make it return `""` for non-command modes, or document that it is test-only.

---

## Pattern Analysis

| Peer            | Mirrored | Missing | Diverged | Intentional |
| --------------- | -------: | ------: | -------: | ----------: |
| `src/tests/vi_tests.rs ↔ src/tests/input_tests.rs` | 1 | 2 | 4 | 7 |
| `src/ui/input/vi.rs ↔ src/ui/input/mod.rs` | 4 | 2 | 8 | 18 |

**Missing/Diverged rows drive:** Q15, PM1, PM2

**Key divergences from peer**
- `InputEditor` owns the buffer/cursor and orchestrates all mode transitions; `ViState` is intentionally a passive state bag. This is a clean architectural split but leaves some peer helpers (`clear_buffer`, `vi_visual_selection`) without mirrors in the pure vi module.
- The new test file focuses on low-level motion/operator units rather than `KeyEvent` dispatch, so it misses integration-path coverage and mixed-stream typing scenarios that `input_tests.rs` exercises for emacs mode.

---

## Impact

| Consumer        | Change           | Findings |
| --------------- | ---------------- | -------- |
| `src/ui/mod.rs:447` | `InputEditor::new` now takes `InputMode` | I1, Q12 |
| `src/ui/renderer.rs:590` | `draw_bottom` gained `mode_label` and `vi_selection` | Q21, S1 |
| `src/ui/permission_handler.rs:42` | `draw_bottom` call updated for new signature | — |
| `src/tests/input_tests.rs:6` | `InputEditor::new(InputMode::Emacs)` | — |
| `src/tests/mod.rs:60` | `vi_tests` feature-gated registration | I1 |

---

## Precedents

| Commit    | Subject          | Follow-ups within 30 days |
| --------- | ---------------- | --------------------------- |
| `e87394a` | feat: vi-mode input system | 9 follow-up fixes (panic at cursor 0, missing undo, visual selection, t/T, UTF-8 backspace, e/E, text objects, registers) |
| `4020a47` | feat(input): add Emacs-style keybindings, kill ring, multi-line editing | 2 follow-up fixes (cursor flicker, CJK cursor math) |
| `68763f2` | feat(ui): configurable color themes | Entire PR reverted after broken theme reset |
| `d8c2f11` | implemented advisor | 3 follow-up fixes (race condition, config schema, permissions rendering) |
| `e9f310d` | implemented multimodal support | 2 follow-up fixes (compile-time gating, tests) |

**Recurring lessons (most → least frequent)**

1. Multi-byte / wide-character input is the highest-risk area; almost every major input change has needed follow-up cursor/boundary fixes.
2. New large UI modules need visual/selection rendering fixes immediately.
3. Feature-gated / cross-cutting features regress on compile-time gating and config wiring; the CLI/config surface and the feature gate must stay in sync.
4. Boundary conditions in motions and operators cause panics; `wrapping_sub` on cursor positions is a recurring anti-pattern.
5. Undo integration must be explicitly wired for every mutating operation.

---

## Recommendation

| # | ID     | Action                      | Alt / Note        |
| - | ------ | --------------------------- | ----------------- |
| 1 | I1     | Add `'vi-mode'` to the `default` feature set in `Cargo.toml` so `--input-mode vi` works in default builds. | Or gate the CLI/config vi option behind `#[cfg(feature = "vi-mode")]` and emit a clear error if it is used without the feature. |
| 2 | I3     | Do not consume `pending_op` until `apply_operator`/`apply_motion` returns a valid result; restore state on `None`. | — |
| 3 | Q1     | Guard the forward-search slice against cursor-at-end-of-buffer. | — |
| 4 | Q16    | Return early in backward `T` when `cursor == 0`. | — |
| 5 | Q18    | Use byte indices instead of char counts when slicing `buf` in `resolve_text_object`. | — |
| 6 | I4     | Make visual `h`/`l` honor `count` by reusing `apply_motion` or adding the same loop. | — |
| 7 | I5     | Add `found` counter to `t`/`T` and fix the `cursor == 0` panic. | — |
| 8 | I2     | Implement visual indent/unindent or remove the `>`/`<` keybindings and `IndentRight`/`IndentLeft` variants. | — |
| 9 | Q22    | Replace `parse().ok()` with an explicit error for invalid input modes. | — |
| 10 | Q15    | Add `InputEditor` integration tests for the vi handlers. | — |
