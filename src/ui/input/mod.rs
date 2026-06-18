pub(crate) mod cursor;
mod pickers;

pub use cursor::cursor_to_line_col;
pub use cursor::{
    count_lines, line_col_to_cursor, line_end, line_start, next_char_boundary, prev_char_boundary,
};
pub use pickers::Picker;

use compact_str::CompactString;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::io::Write;

use crate::config::types::InputMode;
use crate::ui::pickers::file::FilePicker;
use crate::ui::pickers::list::ListPicker;
use crate::ui::pickers::models::ModelsPicker;

const MAX_KILL_RING: usize = 30;

#[cfg(feature = "vi-mode")]
pub(crate) mod vi;
#[cfg(feature = "vi-mode")]
use vi::*;

pub struct InputEditor {
    pub buffer: CompactString,
    pub cursor: usize,
    pub input_mode: InputMode,
    pub mode_label: compact_str::CompactString,
    history: Vec<CompactString>,
    history_pos: Option<usize>,
    draft: Option<CompactString>,
    pub picker: Option<Picker>,
    monochrome: bool,
    prompt_names: Vec<String>,
    theme_names: Vec<String>,
    quick_model_names: Vec<String>,
    live_model_names: Vec<String>,
    provider_names: Vec<String>,
    editor: Option<String>,
    kill_ring: Vec<CompactString>,
    yank_pos: Option<usize>,
    yank_len: usize,
    #[cfg(feature = "vi-mode")]
    vi_state: ViState,
}

impl InputEditor {
    pub fn new(input_mode: InputMode) -> Self {
        let mut this = InputEditor {
            buffer: CompactString::new(""),
            cursor: 0,
            input_mode,
            mode_label: compact_str::CompactString::new("> "),
            history: Vec::new(),
            history_pos: None,
            draft: None,
            picker: None,
            monochrome: false,
            prompt_names: Vec::new(),
            theme_names: Vec::new(),
            quick_model_names: Vec::new(),
            live_model_names: Vec::new(),
            provider_names: Vec::new(),
            editor: None,
            kill_ring: Vec::with_capacity(MAX_KILL_RING),
            yank_pos: None,
            yank_len: 0,
            #[cfg(feature = "vi-mode")]
            vi_state: ViState::new(),
        };
        #[cfg(feature = "vi-mode")]
        this.update_vi_mode_label();
        this
    }

    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.history_pos = None;
        self.draft = None;
        self.yank_pos = None;
    }

    pub fn set_quick_model_names(&mut self, names: Vec<String>) {
        self.quick_model_names = names;
    }

    pub fn set_live_model_names(&mut self, names: Vec<String>) {
        self.live_model_names = names;
    }

    pub fn set_provider_names(&mut self, names: Vec<String>) {
        self.provider_names = names;
    }

    pub fn set_editor(&mut self, editor: String) {
        self.editor = Some(editor);
    }

    pub fn set_monochrome(&mut self, monochrome: bool) {
        self.monochrome = monochrome;
        if let Some(ref mut picker) = self.picker {
            picker.set_monochrome(monochrome);
        }
    }

    pub fn set_prompt_names(&mut self, names: Vec<String>) {
        self.prompt_names = names;
    }

    pub fn set_theme_names(&mut self, names: Vec<String>) {
        self.theme_names = names;
    }

    pub fn load_global_history(&mut self) {
        if let Ok(entries) = crate::session::chat_history::load_history() {
            self.history = entries
                .into_iter()
                .map(|e| CompactString::new(e.content))
                .collect();
            self.history_pos = None;
        }
    }

    pub fn start_file_picker(&mut self) {
        let mut picker = FilePicker::new();
        picker.set_monochrome(self.monochrome);
        picker.activate();
        self.picker = Some(Picker::File(picker));
    }

    pub fn start_command_picker(&mut self) {
        let mut picker = ListPicker::with_static_commands();
        picker.set_monochrome(self.monochrome);
        picker.activate();
        self.picker = Some(Picker::Command(picker));
    }

    pub fn start_models_picker(&mut self) {
        let mut picker = ModelsPicker::new();
        picker.set_monochrome(self.monochrome);
        picker.set_groups(
            self.quick_model_names.clone(),
            self.live_model_names.clone(),
        );
        picker.activate();
        self.picker = Some(Picker::Models(picker));
    }

    pub fn start_provider_picker(&mut self) {
        let mut picker = ListPicker::new();
        picker.set_monochrome(self.monochrome);
        if !self.provider_names.is_empty() {
            picker.set_items(self.provider_names.clone());
        }
        picker.activate();
        self.picker = Some(Picker::Prefixed(picker, "/provider "));
    }

    pub fn start_prompt_picker(&mut self) {
        let mut picker = ListPicker::new();
        picker.set_monochrome(self.monochrome);
        if !self.prompt_names.is_empty() {
            picker.set_items(self.prompt_names.clone());
        }
        picker.activate();
        self.picker = Some(Picker::Prefixed(picker, "/prompt "));
    }

    pub fn start_dot_picker(&mut self) {
        let mut picker = ListPicker::new();
        picker.set_monochrome(self.monochrome);
        if !self.prompt_names.is_empty() {
            picker.set_items(self.prompt_names.clone());
        }
        picker.activate();
        self.picker = Some(Picker::Prefixed(picker, "."));
    }

    pub fn start_theme_picker(&mut self) {
        let mut picker = ListPicker::new();
        picker.set_monochrome(self.monochrome);
        if !self.theme_names.is_empty() {
            picker.set_items(self.theme_names.clone());
        }
        picker.activate();
        self.picker = Some(Picker::Prefixed(picker, "/theme "));
    }

    pub fn open_in_editor(&mut self) {
        let editor = self
            .editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "editor".to_string());

        let tmp = std::env::temp_dir().join(format!("zerostack-{}.md", std::process::id()));

        let _ = std::fs::write(&tmp, self.buffer.as_bytes());

        let _ = crossterm::terminal::disable_raw_mode();
        let mut stdout = std::io::stdout();
        let _ = crossterm::ExecutableCommand::execute(
            &mut stdout,
            crossterm::event::DisableMouseCapture,
        );
        let _ = crossterm::ExecutableCommand::execute(
            &mut stdout,
            crossterm::terminal::LeaveAlternateScreen,
        );
        let _ = stdout.flush();

        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("{} \"$1\"", editor))
            .arg("sh")
            .arg(&tmp)
            .status();

        let _ = crossterm::ExecutableCommand::execute(
            &mut stdout,
            crossterm::terminal::EnterAlternateScreen,
        );
        let _ = crossterm::ExecutableCommand::execute(
            &mut stdout,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        );
        let _ = crossterm::ExecutableCommand::execute(
            &mut stdout,
            crossterm::event::EnableMouseCapture,
        );
        let _ = crossterm::terminal::enable_raw_mode();

        if let Ok(content) = std::fs::read_to_string(&tmp) {
            self.buffer = CompactString::new(content.trim_end());
            self.cursor = self.buffer.len();
        }

        let _ = std::fs::remove_file(&tmp);
    }

    pub fn handle_paste(&mut self, data: String) {
        self.buffer.insert_str(self.cursor, &data);
        self.cursor += data.len();
        self.history_pos = None;
        self.draft = None;
        self.yank_pos = None;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        #[cfg(feature = "vi-mode")]
        if self.input_mode == InputMode::Vi {
            return self.handle_vi_key(key);
        }
        self.handle_key_impl(key)
    }

    fn handle_key_impl(&mut self, key: KeyEvent) -> Option<CompactString> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if ctrl {
            match key.code {
                KeyCode::Char('a') => {
                    let current = line_start(&self.buffer, self.cursor);
                    if self.cursor == current {
                        let (line, _) = cursor_to_line_col(&self.buffer, self.cursor);
                        if line > 0 {
                            self.cursor = line_end(&self.buffer, current - 1);
                        }
                    } else {
                        self.cursor = current;
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('e') => {
                    let current = line_end(&self.buffer, self.cursor);
                    if self.cursor == current {
                        let (line, _) = cursor_to_line_col(&self.buffer, self.cursor);
                        let total = count_lines(&self.buffer);
                        if line + 1 < total {
                            self.cursor = line_start(&self.buffer, self.cursor + 1);
                        }
                    } else {
                        self.cursor = current;
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('b') => {
                    if self.cursor > 0 {
                        self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('f') => {
                    if self.cursor < self.buffer.len() {
                        self.cursor = next_char_boundary(&self.buffer, self.cursor);
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('p') => {
                    return self.cursor_up();
                }
                KeyCode::Char('n') => {
                    return self.cursor_down();
                }
                KeyCode::Char('w') => {
                    let deleted = self.delete_prev_word();
                    if !deleted.is_empty() {
                        self.push_kill(deleted);
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('u') => {
                    if self.cursor > 0 {
                        let deleted: String = self.buffer.chars().take(self.cursor).collect();
                        let remaining: String = self.buffer.chars().skip(self.cursor).collect();
                        self.buffer = CompactString::new(&remaining);
                        self.cursor = 0;
                        if !deleted.is_empty() {
                            self.push_kill(CompactString::new(&deleted));
                        }
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('k') => {
                    if self.cursor < self.buffer.len() {
                        let deleted: String = self.buffer.chars().skip(self.cursor).collect();
                        let before: String = self.buffer.chars().take(self.cursor).collect();
                        self.buffer = CompactString::new(&before);
                        if !deleted.is_empty() {
                            self.push_kill(CompactString::new(&deleted));
                        }
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('d') => {
                    if self.cursor < self.buffer.len() {
                        self.buffer.remove(self.cursor);
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('y') => {
                    if self.kill_ring.is_empty() {
                        return None;
                    }
                    let pos = self.yank_pos.unwrap_or(0);
                    let text = &self.kill_ring[pos];
                    self.buffer.insert_str(self.cursor, text);
                    self.yank_len = text.len();
                    self.cursor += text.len();
                    self.yank_pos = Some(pos);
                    return None;
                }
                KeyCode::Char('t') => {
                    // Transpose chars: swap char before cursor with char at cursor
                    if self.cursor > 0 && self.cursor <= self.buffer.len() {
                        let prev = prev_char_boundary(&self.buffer, self.cursor);
                        if prev > 0 {
                            let prev2 = prev_char_boundary(&self.buffer, prev);
                            let mut s = String::with_capacity(self.buffer.len());
                            s.push_str(&self.buffer[..prev2]);
                            s.push_str(&self.buffer[prev..self.cursor]);
                            s.push_str(&self.buffer[prev2..prev]);
                            s.push_str(&self.buffer[self.cursor..]);
                            self.buffer = CompactString::new(&s);
                            self.cursor = next_char_boundary(&self.buffer, prev);
                        }
                    }
                    self.yank_pos = None;
                    return None;
                }
                _ => {}
            }
        }

        if alt {
            match key.code {
                KeyCode::Char('b') => {
                    self.cursor = self.prev_word_start();
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('f') => {
                    self.cursor = self.next_word_end();
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('d') => {
                    let deleted = self.delete_next_word();
                    if !deleted.is_empty() {
                        self.push_kill(deleted);
                    }
                    self.yank_pos = None;
                    return None;
                }
                KeyCode::Char('y') => {
                    if let Some(pos) = self.yank_pos
                        && self.kill_ring.len() > 1
                    {
                        let start = self.cursor.saturating_sub(self.yank_len);
                        if start <= self.cursor {
                            let before: String = self.buffer.chars().take(start).collect();
                            let after: String = self.buffer.chars().skip(self.cursor).collect();
                            self.buffer = CompactString::new(format!("{}{}", before, after));
                            self.cursor = start;
                        }
                        let new_pos = if pos == 0 {
                            self.kill_ring.len() - 1
                        } else {
                            pos - 1
                        };
                        self.yank_pos = Some(new_pos);
                        let text = &self.kill_ring[new_pos];
                        self.buffer.insert_str(self.cursor, text);
                        self.yank_len = text.len();
                        self.cursor += text.len();
                    }
                    return None;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Enter
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if self.picker.as_ref().is_some_and(|p| p.active()) {
                    return None;
                }
                self.buffer.insert(self.cursor, '\n');
                self.cursor += 1;
                None
            }
            KeyCode::Enter => {
                if self.picker.as_ref().is_some_and(|p| p.active()) {
                    return None;
                }
                let text = self.buffer.clone();
                let is_blank = text.trim().is_empty();
                if !is_blank {
                    self.history.push(text.clone());
                }
                self.history_pos = None;
                self.draft = None;
                self.buffer.clear();
                self.cursor = 0;
                self.yank_pos = None;
                if text.is_empty() { None } else { Some(text) }
            }
            KeyCode::Char(c)
                if c == '\x08' || (c == 'h' && key.modifiers.contains(KeyModifiers::CONTROL)) =>
            {
                if self.cursor > 0 {
                    self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                    self.buffer.remove(self.cursor);
                }
                None
            }
            KeyCode::Char(c) => {
                if c == '@' {
                    let at_word_start = self.cursor == 0
                        || self.buffer[..self.cursor]
                            .chars()
                            .next_back()
                            .is_some_and(|prev| prev == ' ');
                    if at_word_start {
                        self.start_file_picker();
                    }
                }
                if c == '/' && self.cursor == 0 {
                    self.start_command_picker();
                }
                if c == '.' && self.cursor == 0 {
                    self.buffer.insert(self.cursor, c);
                    self.cursor += c.len_utf8();
                    self.start_dot_picker();
                    self.yank_pos = None;
                    return None;
                }
                self.buffer.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                self.history_pos = None;
                self.draft = None;
                self.yank_pos = None;

                if (self.picker.is_none() || !self.picker.as_ref().is_some_and(|p| p.active()))
                    && self.buffer.starts_with("/prompt ")
                {
                    let after_prefix: String = self.buffer.chars().skip("/prompt ".len()).collect();
                    if !after_prefix.is_empty() && c != ' ' {
                        let query_len = after_prefix.len();
                        if query_len == 1 {
                            self.start_prompt_picker();
                            if let Some(Picker::Prefixed(ref mut pp, _)) = self.picker {
                                pp.char_input(c);
                            }
                        }
                    }
                }
                if (self.picker.is_none() || !self.picker.as_ref().is_some_and(|p| p.active()))
                    && self.buffer.starts_with("/models ")
                {
                    let after_prefix: String = self.buffer.chars().skip("/models ".len()).collect();
                    if !after_prefix.is_empty() && c != ' ' {
                        let query_len = after_prefix.len();
                        if query_len == 1 {
                            self.start_models_picker();
                            if let Some(Picker::Models(ref mut mp)) = self.picker {
                                mp.char_input(c);
                            }
                        }
                    }
                }
                if (self.picker.is_none() || !self.picker.as_ref().is_some_and(|p| p.active()))
                    && self.buffer.starts_with("/theme ")
                {
                    let after_prefix: String = self.buffer.chars().skip("/theme ".len()).collect();
                    if !after_prefix.is_empty() && c != ' ' {
                        let query_len = after_prefix.len();
                        if query_len == 1 {
                            self.start_theme_picker();
                            if let Some(Picker::Prefixed(ref mut tp, _)) = self.picker {
                                tp.char_input(c);
                            }
                        }
                    }
                }
                if (self.picker.is_none() || !self.picker.as_ref().is_some_and(|p| p.active()))
                    && self.buffer.starts_with("/provider ")
                {
                    let after_prefix: String =
                        self.buffer.chars().skip("/provider ".len()).collect();
                    if !after_prefix.is_empty() && c != ' ' {
                        let query_len = after_prefix.len();
                        if query_len == 1 {
                            self.start_provider_picker();
                            if let Some(Picker::Prefixed(ref mut pp, _)) = self.picker {
                                pp.char_input(c);
                            }
                        }
                    }
                }

                None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                    self.buffer.remove(self.cursor);
                }
                self.yank_pos = None;
                None
            }
            KeyCode::Delete => {
                if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                }
                self.yank_pos = None;
                None
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                }
                self.yank_pos = None;
                None
            }
            KeyCode::Right => {
                if self.cursor < self.buffer.len() {
                    self.cursor = next_char_boundary(&self.buffer, self.cursor);
                }
                self.yank_pos = None;
                None
            }
            KeyCode::Up => {
                self.yank_pos = None;
                self.cursor_up()
            }
            KeyCode::Down => {
                self.yank_pos = None;
                self.cursor_down()
            }
            KeyCode::Home => {
                self.cursor = 0;
                self.yank_pos = None;
                None
            }
            KeyCode::End => {
                self.cursor = self.buffer.len();
                self.yank_pos = None;
                None
            }
            KeyCode::Tab => {
                self.buffer.insert_str(self.cursor, "  ");
                self.cursor += 2;
                self.yank_pos = None;
                None
            }
            _ => None,
        }
    }

    fn history_up(&mut self) -> Option<CompactString> {
        let hist_len = self.history.len();
        if hist_len == 0 || self.history_pos == Some(0) {
            return None;
        }
        if self.history_pos.is_none() {
            self.draft = Some(self.buffer.clone());
        }
        let pos = match self.history_pos {
            Some(p) if p > 0 => p - 1,
            Some(_) => unreachable!(),
            None => hist_len - 1,
        };
        self.history_pos = Some(pos);
        self.buffer = self.history[pos].clone();
        self.cursor = 0;
        None
    }

    fn history_down(&mut self) -> Option<CompactString> {
        match self.history_pos {
            Some(pos) if pos + 1 < self.history.len() => {
                let new_pos = pos + 1;
                self.history_pos = Some(new_pos);
                self.buffer = self.history[new_pos].clone();
                self.cursor = self.buffer.len();
            }
            Some(_) => {
                self.history_pos = None;
                if let Some(draft) = self.draft.take() {
                    self.buffer = draft.clone();
                    self.cursor = self.buffer.len();
                } else {
                    self.buffer.clear();
                    self.cursor = 0;
                }
            }
            None => {}
        }
        None
    }

    fn cursor_up(&mut self) -> Option<CompactString> {
        let (line, col) = cursor_to_line_col(&self.buffer, self.cursor);
        if line > 0 {
            let line_len =
                line_end(&self.buffer, self.cursor) - line_start(&self.buffer, self.cursor);
            let target = line_col_to_cursor(
                &self.buffer,
                line - 1,
                if col >= line_len { usize::MAX } else { col },
            );
            self.cursor = target;
            None
        } else {
            self.history_up()
        }
    }

    fn cursor_down(&mut self) -> Option<CompactString> {
        let (line, col) = cursor_to_line_col(&self.buffer, self.cursor);
        let total = count_lines(&self.buffer);
        if line + 1 < total {
            let line_len =
                line_end(&self.buffer, self.cursor) - line_start(&self.buffer, self.cursor);
            let target = line_col_to_cursor(
                &self.buffer,
                line + 1,
                if col >= line_len { usize::MAX } else { col },
            );
            self.cursor = target;
            None
        } else {
            self.history_down()
        }
    }

    fn push_kill(&mut self, text: CompactString) {
        if text.is_empty() {
            return;
        }
        if self.kill_ring.first() == Some(&text) {
            return;
        }
        self.kill_ring.insert(0, text);
        if self.kill_ring.len() > MAX_KILL_RING {
            self.kill_ring.pop();
        }
    }

    fn prev_word_start(&self) -> usize {
        if self.cursor == 0 {
            return 0;
        }
        let pairs: Vec<(usize, char)> = self.buffer.char_indices().collect();
        if pairs.is_empty() {
            return 0;
        }
        let char_idx = pairs
            .iter()
            .position(|&(bi, _)| bi >= self.cursor)
            .unwrap_or(pairs.len());
        let mut pos = char_idx;
        while pos > 0 && pairs[pos - 1].1 == ' ' {
            pos -= 1;
        }
        while pos > 0 && pairs[pos - 1].1 != ' ' {
            pos -= 1;
        }
        if pos < pairs.len() {
            pairs[pos].0
        } else {
            self.buffer.len()
        }
    }

    fn next_word_end(&self) -> usize {
        let pairs: Vec<(usize, char)> = self.buffer.char_indices().collect();
        let len = pairs.len();
        if len == 0 {
            return 0;
        }
        let char_idx = pairs
            .iter()
            .position(|&(bi, _)| bi >= self.cursor)
            .unwrap_or(len);
        let mut pos = char_idx;
        while pos < len && pairs[pos].1 == ' ' {
            pos += 1;
        }
        while pos < len && pairs[pos].1 != ' ' {
            pos += 1;
        }
        if pos < len {
            pairs[pos].0
        } else {
            self.buffer.len()
        }
    }

    fn delete_prev_word(&mut self) -> CompactString {
        if self.cursor == 0 || self.buffer.is_empty() {
            return CompactString::new("");
        }
        let start = self.prev_word_start();
        let deleted: CompactString = self.buffer[start..self.cursor].into();
        let before = &self.buffer[..start];
        let after = &self.buffer[self.cursor..];
        let mut new_buf = String::with_capacity(before.len() + after.len());
        new_buf.push_str(before);
        new_buf.push_str(after);
        self.buffer = CompactString::new(&new_buf);
        self.cursor = start;
        deleted
    }

    fn delete_next_word(&mut self) -> CompactString {
        if self.cursor >= self.buffer.len() {
            return CompactString::new("");
        }
        let end = self.next_word_end();
        let deleted: CompactString = self.buffer[self.cursor..end].into();
        let before = &self.buffer[..self.cursor];
        let after = &self.buffer[end..];
        let mut new_buf = String::with_capacity(before.len() + after.len());
        new_buf.push_str(before);
        new_buf.push_str(after);
        self.buffer = CompactString::new(&new_buf);
        deleted
    }

    // ---- VI mode handlers ----

    #[cfg(feature = "vi-mode")]
    fn update_vi_mode_label(&mut self) {
        self.mode_label = compact_str::CompactString::from(match self.vi_state.mode {
            ViMode::Insert => "-- INSERT --",
            ViMode::Normal => "-- NORMAL --",
            ViMode::Visual(VisualType::Char) => "-- VISUAL --",
            ViMode::Visual(VisualType::Line) => "-- VISUAL LINE --",
            ViMode::Visual(VisualType::Block) => "-- VISUAL BLOCK --",
            ViMode::CommandLine => ":",
            ViMode::SearchForward => "/",
            ViMode::SearchBackward => "?",
        });
    }

    #[cfg(feature = "vi-mode")]
    fn save_vi_undo_point(&mut self) {
        self.vi_state.insert_start_buf = self.buffer.clone();
        self.vi_state.insert_start_cursor = self.cursor;
    }

    #[cfg(feature = "vi-mode")]
    fn commit_vi_undo(&mut self) {
        let old_buf = self.vi_state.insert_start_buf.clone();
        let old_cursor = self.vi_state.insert_start_cursor;
        let new_buf = self.buffer.clone();
        let new_cursor = self.cursor;
        self.vi_state
            .push_undo(&old_buf, &new_buf, old_cursor, new_cursor);
    }

    #[cfg(feature = "vi-mode")]
    fn handle_vi_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        if self.picker.as_ref().is_some_and(|p| p.active()) {
            return self.handle_key_impl(key);
        }
        match self.vi_state.mode {
            ViMode::Insert => self.handle_vi_insert_key(key),
            ViMode::Normal => self.handle_vi_normal_key(key),
            ViMode::Visual(_) => self.handle_vi_visual_key(key),
            ViMode::CommandLine => self.handle_vi_cmdline_key(key),
            ViMode::SearchForward | ViMode::SearchBackward => self.handle_vi_search_key(key),
        }
    }

    #[cfg(feature = "vi-mode")]
    fn handle_vi_insert_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        if key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            let old_buf =
                std::mem::replace(&mut self.vi_state.insert_start_buf, self.buffer.clone());
            let old_cursor = self.vi_state.insert_start_cursor;
            if old_buf != self.buffer || old_cursor != self.cursor {
                self.vi_state
                    .push_undo(&old_buf, &self.buffer, old_cursor, self.cursor);
            }
            self.vi_state.last_insert = Some(self.buffer[old_cursor..self.cursor].into());
            self.vi_state.mode = ViMode::Normal;
            self.vi_state.pending_op = None;
            self.update_vi_mode_label();
            None
        } else {
            self.handle_key_impl(key)
        }
    }

    #[cfg(feature = "vi-mode")]
    /// Shared motion dispatch for normal and visual mode.
    /// Handles pending_prefixes (pending_g, pending_fchar_dir), key→motion
    /// Map a KeyCode to a motion string for direct motions (no pending state).
    fn key_to_motion(code: KeyCode) -> Option<&'static str> {
        match code {
            KeyCode::Char('h') | KeyCode::Left => Some("h"),
            KeyCode::Char('j') => Some("j"),
            KeyCode::Char('k') => Some("k"),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => Some("l"),
            KeyCode::Char('w') => Some("w"),
            KeyCode::Char('W') => Some("W"),
            KeyCode::Char('b') => Some("b"),
            KeyCode::Char('B') => Some("B"),
            KeyCode::Char('e') => Some("e"),
            KeyCode::Char('E') => Some("E"),
            KeyCode::Char('$') | KeyCode::End => Some("$"),
            KeyCode::Char('^') => Some("^"),
            KeyCode::Home => Some("home"),
            KeyCode::Char('G') => Some("G"),
            KeyCode::Char('{') => Some("{"),
            KeyCode::Char('}') => Some("}"),
            KeyCode::Char('%') => Some("%"),
            KeyCode::Char(';') => Some(";"),
            KeyCode::Char(',') => Some(","),
            _ => None,
        }
    }

    /// mapping, and applies the motion. Reads pending_count internally.
    /// Returns true if the key was consumed (motion handled or pending state set).
    fn handle_vi_motion(&mut self, key: KeyCode) -> bool {
        // Handle pending gg/gv prefix (always consumes pending_count)
        if self.vi_state.pending_g {
            let cnt = self.vi_state.pending_count.max(1) as usize;
            self.vi_state.pending_count = 0;
            self.vi_state.pending_g = false;
            if let KeyCode::Char('g') = key {
                if let Some(c) = apply_motion(&self.buffer, self.cursor, cnt, "gg", &self.vi_state)
                {
                    self.cursor = c;
                }
            } else if let KeyCode::Char('v') = key {
                // gv: reselect last visual selection
                if let (Some(s), Some(e)) = (
                    self.vi_state.last_visual_start,
                    self.vi_state.last_visual_end,
                ) {
                    self.vi_state.visual_start = s;
                    self.cursor = e;
                    self.vi_state.mode = ViMode::Visual(VisualType::Char);
                    self.update_vi_mode_label();
                }
            }
            return true;
        }

        // Handle pending f/F/t/T prefix (always consumes pending_count)
        if self.vi_state.pending_fchar_dir.is_some() {
            let count = self.vi_state.pending_count.max(1) as usize;
            self.vi_state.pending_count = 0;
            let dir = self.vi_state.pending_fchar_dir.take();
            let is_t = self.vi_state.pending_fchar_is_t;
            self.vi_state.pending_fchar_is_t = false;
            if let KeyCode::Char(c) = key {
                self.vi_state.last_fchar = Some(c);
                self.vi_state.last_fchar_dir = dir;
                self.vi_state.last_fchar_is_t = is_t;
                let motion = match (dir, is_t) {
                    (Some(Direction::Forward), true) => "t",
                    (Some(Direction::Forward), false) => "f",
                    (Some(Direction::Backward), true) => "T",
                    (Some(Direction::Backward), false) => "F",
                    _ => "f",
                };
                if let Some(c) =
                    apply_motion(&self.buffer, self.cursor, count, motion, &self.vi_state)
                {
                    self.cursor = c;
                }
            }
            return true;
        }

        // Map key to motion and apply
        // Only consume pending_count when the key is actually a motion
        let motion = match Self::key_to_motion(key) {
            Some(m) => m,
            None => {
                match key {
                    // Keys that set pending state for next call (don't consume count yet)
                    KeyCode::Char('g') => {
                        self.vi_state.pending_g = true;
                        return true;
                    }
                    KeyCode::Char('f') => {
                        self.vi_state.pending_fchar_dir = Some(Direction::Forward);
                        return true;
                    }
                    KeyCode::Char('F') => {
                        self.vi_state.pending_fchar_dir = Some(Direction::Backward);
                        return true;
                    }
                    KeyCode::Char('t') => {
                        self.vi_state.pending_fchar_dir = Some(Direction::Forward);
                        self.vi_state.pending_fchar_is_t = true;
                        return true;
                    }
                    KeyCode::Char('T') => {
                        self.vi_state.pending_fchar_dir = Some(Direction::Backward);
                        self.vi_state.pending_fchar_is_t = true;
                        return true;
                    }
                    // 0 is a motion only when not accumulating a count
                    KeyCode::Char('0') => {
                        let count = self.vi_state.pending_count.max(1) as usize;
                        self.vi_state.pending_count = 0;
                        if let Some(c) =
                            apply_motion(&self.buffer, self.cursor, count, "0", &self.vi_state)
                        {
                            self.cursor = c;
                        }
                        return true;
                    }
                    _ => return false,
                }
            }
        };
        let count = self.vi_state.pending_count.max(1) as usize;
        self.vi_state.pending_count = 0;
        if let Some(c) = apply_motion(&self.buffer, self.cursor, count, motion, &self.vi_state) {
            self.cursor = c;
        }
        true
    }

    #[cfg(feature = "vi-mode")]
    fn handle_vi_normal_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        // Arrow keys and Ctrl+N/P navigate history in all modes
        if matches!(key.code, KeyCode::Up | KeyCode::Down)
            || (matches!(key.code, KeyCode::Char('n') | KeyCode::Char('p'))
                && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return self.handle_key_impl(key);
        }

        // Ctrl-g opens $EDITOR
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('g') {
            self.open_in_editor();
            return None;
        }

        // Handle register prefix: "x waits for register char
        if self.vi_state.pending_register.is_some()
            && matches!(
                key.code,
                KeyCode::Char('a'..='z')
                    | KeyCode::Char('0'..='9')
                    | KeyCode::Char('"')
                    | KeyCode::Char('+')
            )
        {
            if let KeyCode::Char(c) = key.code {
                self.vi_state.pending_register = Some(c);
            }
            return None;
        }

        // Handle pending text object (i/a after operator)
        if let Some(inner) = self.vi_state.pending_text_object.take() {
            if let Some(op) = self.vi_state.pending_op.take() {
                let register = self.vi_state.pending_register.take();
                let obj = match key.code {
                    KeyCode::Char('w') => Some(TextObject::Word),
                    KeyCode::Char('W') => Some(TextObject::WORD),
                    KeyCode::Char('"') => Some(TextObject::Quotes('"')),
                    KeyCode::Char('\'') => Some(TextObject::Quotes('\'')),
                    KeyCode::Char('`') => Some(TextObject::Quotes('`')),
                    KeyCode::Char('(') | KeyCode::Char(')') => Some(TextObject::Brackets('(', ')')),
                    KeyCode::Char('[') | KeyCode::Char(']') => Some(TextObject::Brackets('[', ']')),
                    KeyCode::Char('{') | KeyCode::Char('}') => Some(TextObject::Brackets('{', '}')),
                    KeyCode::Char('<') | KeyCode::Char('>') => Some(TextObject::Brackets('<', '>')),
                    KeyCode::Char('p') => Some(TextObject::Paragraph),
                    KeyCode::Char('s') => Some(TextObject::Sentence),
                    KeyCode::Char('t') => Some(TextObject::Tag),
                    _ => None,
                };
                if let Some(obj) = obj {
                    let (start, end) =
                        ViState::resolve_text_object(&self.buffer, self.cursor, obj, inner);
                    if end > start {
                        match op {
                            ViOperator::Delete | ViOperator::Change => {
                                self.save_vi_undo_point();
                                let (new_buf, deleted) = delete_range(&self.buffer, start, end);
                                if !deleted.is_empty() {
                                    self.vi_state.registers.set('"', deleted.clone());
                                    if let Some(r) = register {
                                        self.vi_state.registers.set(r, deleted);
                                    }
                                }
                                self.buffer = new_buf;
                                self.cursor = start.min(self.buffer.len());
                                self.commit_vi_undo();
                                if op == ViOperator::Change {
                                    self.vi_state.mode = ViMode::Insert;
                                    self.update_vi_mode_label();
                                }
                            }
                            ViOperator::Yank => {
                                let yanked = yank_range(&self.buffer, start, end);
                                self.vi_state.registers.set('"', yanked.clone());
                                if let Some(r) = register {
                                    self.vi_state.registers.set(r, yanked);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            return None;
        }

        // Handle pending r char (r{char} replacement)
        if self.vi_state.pending_r_char.is_some() {
            if let KeyCode::Char(replace_with) = key.code {
                let replace_with = replace_with;
                self.vi_state.pending_r_char = None;
                let cnt = self.vi_state.pending_count.max(1) as usize;
                self.vi_state.pending_count = 0;
                self.save_vi_undo_point();
                for _ in 0..cnt {
                    if self.cursor < self.buffer.len() {
                        let end = next_char_boundary(&self.buffer, self.cursor);
                        let mut s = String::with_capacity(self.buffer.len() - (end - self.cursor));
                        s.push_str(&self.buffer[..self.cursor]);
                        let mut rep = String::with_capacity(replace_with.len_utf8());
                        rep.push(replace_with);
                        s.push_str(&rep);
                        s.push_str(&self.buffer[end..]);
                        self.buffer = CompactString::new(&s);
                    }
                }
                self.commit_vi_undo();
            } else {
                self.vi_state.pending_r_char = None;
            }
            return None;
        }

        // If pending_op/g/fchar is set, try operator+motion path
        if self.vi_state.pending_op.is_some()
            || self.vi_state.pending_g
            || self.vi_state.pending_fchar_dir.is_some()
        {
            let motion = Self::key_to_motion(key.code).or(match key.code {
                KeyCode::Char('0') if self.vi_state.pending_count == 0 => Some("0"),
                _ => None,
            });
            if let Some(m) = motion {
                self.apply_vi_motion_or_op(m);
                return None;
            }
            // Don't clear pending_op here — let the main match handle
            // double-operator keys (dd, yy, cc, etc.) and operator arms.
            self.vi_state.pending_g = false;
            self.vi_state.pending_fchar_dir = None;
            self.vi_state.pending_fchar_is_t = false;
        }

        // Pure motion (no operator) or other command
        if self.handle_vi_motion(key.code) {
            return None;
        }

        match key.code {
            KeyCode::Esc => {
                self.vi_state.pending_op = None;
                self.vi_state.pending_register = None;
                self.vi_state.pending_text_object = None;
                self.vi_state.pending_r_char = None;
                self.vi_state.pending_g = false;
                self.vi_state.pending_fchar_dir = None;
                self.vi_state.pending_fchar_is_t = false;
                None
            }

            // ---- Text object prefix (i/a after operator) ----
            KeyCode::Char('i') if self.vi_state.pending_op.is_some() => {
                self.vi_state.pending_text_object = Some(true);
                None
            }
            KeyCode::Char('a') if self.vi_state.pending_op.is_some() => {
                self.vi_state.pending_text_object = Some(false);
                None
            }

            // ---- Enter insert ----
            KeyCode::Char('i') => {
                self.save_vi_undo_point();
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('I') => {
                self.save_vi_undo_point();
                self.cursor = ViState::first_non_whitespace(&self.buffer, self.cursor);
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('a') => {
                self.save_vi_undo_point();
                if self.cursor < self.buffer.len() {
                    self.cursor = next_char_boundary(&self.buffer, self.cursor);
                }
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('A') => {
                self.save_vi_undo_point();
                self.cursor = self.buffer.len();
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('o') => {
                self.save_vi_undo_point();
                self.buffer.insert(self.cursor, '\n');
                self.cursor += 1;
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('O') => {
                self.save_vi_undo_point();
                let (_line, _) = cursor_to_line_col(&self.buffer, self.cursor);
                let start = line_start(&self.buffer, self.cursor);
                self.buffer.insert(start, '\n');
                self.cursor = start;
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('s') => {
                self.save_vi_undo_point();
                self.vi_state.pending_op = Some(ViOperator::Change);
                self.apply_vi_motion_or_op("l");
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('S') => {
                self.save_vi_undo_point();
                self.vi_state.pending_op = Some(ViOperator::Change);
                self.apply_vi_motion_or_op("_");
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('C') => {
                self.save_vi_undo_point();
                self.vi_state.pending_op = Some(ViOperator::Change);
                self.apply_vi_motion_or_op("$");
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('D') => {
                self.save_vi_undo_point();
                self.vi_state.pending_op = Some(ViOperator::Delete);
                self.apply_vi_motion_or_op("$");
                self.commit_vi_undo();
                None
            }

            // ---- Operators ----
            KeyCode::Char('d') => {
                if let Some(ViOperator::Delete) = self.vi_state.pending_op {
                    // dd
                    self.vi_state.pending_op = None;
                    self.save_vi_undo_point();
                    self.apply_vi_operator("_");
                    self.commit_vi_undo();
                } else {
                    self.vi_state.pending_op = Some(ViOperator::Delete);
                }
                None
            }
            KeyCode::Char('y') => {
                if let Some(ViOperator::Yank) = self.vi_state.pending_op {
                    self.vi_state.pending_op = None;
                    self.save_vi_undo_point();
                    self.apply_vi_operator("_");
                } else {
                    self.vi_state.pending_op = Some(ViOperator::Yank);
                }
                None
            }
            KeyCode::Char('c') => {
                if let Some(ViOperator::Change) = self.vi_state.pending_op {
                    self.vi_state.pending_op = None;
                    self.save_vi_undo_point();
                    self.apply_vi_operator("_");
                    self.vi_state.mode = ViMode::Insert;
                    self.update_vi_mode_label();
                } else {
                    self.vi_state.pending_op = Some(ViOperator::Change);
                }
                None
            }
            KeyCode::Char('>') => {
                if let Some(ViOperator::IndentRight) = self.vi_state.pending_op {
                    self.vi_state.pending_op = None;
                    self.save_vi_undo_point();
                    self.apply_vi_operator("_");
                    self.commit_vi_undo();
                } else {
                    self.vi_state.pending_op = Some(ViOperator::IndentRight);
                }
                None
            }
            KeyCode::Char('<') => {
                if let Some(ViOperator::IndentLeft) = self.vi_state.pending_op {
                    self.vi_state.pending_op = None;
                    self.save_vi_undo_point();
                    self.apply_vi_operator("_");
                    self.commit_vi_undo();
                } else {
                    self.vi_state.pending_op = Some(ViOperator::IndentLeft);
                }
                None
            }

            // ---- Other commands ----
            KeyCode::Char('x') => {
                self.save_vi_undo_point();
                if self.cursor < self.buffer.len() {
                    let saved_cursor = self.cursor;
                    self.vi_state.insert_start_cursor = saved_cursor;
                    let end = next_char_boundary(&self.buffer, self.cursor);
                    let mut s = String::with_capacity(self.buffer.len() - (end - self.cursor));
                    s.push_str(&self.buffer[..self.cursor]);
                    s.push_str(&self.buffer[end..]);
                    self.buffer = CompactString::new(&s);
                    self.commit_vi_undo();
                }
                None
            }
            KeyCode::Char('X') => {
                self.save_vi_undo_point();
                if self.cursor > 0 {
                    let saved = self.cursor;
                    self.vi_state.insert_start_cursor = saved;
                    self.cursor = prev_char_boundary(&self.buffer, self.cursor);
                    self.buffer.remove(self.cursor);
                    self.commit_vi_undo();
                }
                None
            }
            KeyCode::Char('p') => {
                self.save_vi_undo_point();
                let register = self.vi_state.pending_register.unwrap_or('"');
                let text = self
                    .vi_state
                    .registers
                    .get(register)
                    .unwrap_or_default()
                    .to_string();
                self.vi_state.pending_register = None;
                if !text.is_empty() {
                    let insert_pos = next_char_boundary(&self.buffer, self.cursor);
                    self.buffer.insert_str(insert_pos, &text);
                    self.cursor = insert_pos + text.len();
                }
                self.commit_vi_undo();
                None
            }
            KeyCode::Char('P') => {
                self.save_vi_undo_point();
                let register = self.vi_state.pending_register.unwrap_or('"');
                let text = self
                    .vi_state
                    .registers
                    .get(register)
                    .unwrap_or_default()
                    .to_string();
                self.vi_state.pending_register = None;
                if !text.is_empty() {
                    self.buffer.insert_str(self.cursor, &text);
                    self.cursor += text.len();
                }
                self.commit_vi_undo();
                None
            }
            KeyCode::Char('u') => {
                if let Some(entry) = self.vi_state.undo_stack.pop() {
                    let redo = UndoEntry {
                        old_buffer: entry.new_buffer.clone(),
                        new_buffer: entry.old_buffer.clone(),
                        old_cursor: entry.new_cursor,
                        new_cursor: entry.old_cursor,
                    };
                    if self.vi_state.redo_stack.len() >= 100 {
                        self.vi_state.redo_stack.remove(0);
                    }
                    self.vi_state.redo_stack.push(redo);
                    self.buffer = entry.old_buffer;
                    self.cursor = entry.old_cursor;
                }
                None
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(entry) = self.vi_state.redo_stack.pop() {
                    let undo = UndoEntry {
                        old_buffer: entry.old_buffer.clone(),
                        new_buffer: entry.new_buffer.clone(),
                        old_cursor: entry.old_cursor,
                        new_cursor: entry.new_cursor,
                    };
                    if self.vi_state.undo_stack.len() >= 100 {
                        self.vi_state.undo_stack.remove(0);
                    }
                    self.vi_state.undo_stack.push(undo);
                    self.buffer = entry.new_buffer;
                    self.cursor = entry.new_cursor;
                }
                None
            }
            KeyCode::Char('r') => {
                self.vi_state.pending_r_char = Some(' '); // will be overwritten by next char
                None
            }
            KeyCode::Char('J') => {
                let cnt = self.vi_state.pending_count.max(1) as usize;
                self.vi_state.pending_count = 0;
                self.save_vi_undo_point();
                for _ in 0..cnt {
                    let rest = &self.buffer[self.cursor..];
                    if let Some(nl_pos) = rest.find('\n') {
                        let abs_pos = self.cursor + nl_pos;
                        self.buffer.remove(abs_pos);
                        if abs_pos < self.buffer.len() && !self.buffer[abs_pos..].starts_with(' ') {
                            self.buffer.insert(abs_pos, ' ');
                            self.cursor = abs_pos + 1;
                        } else {
                            self.cursor = abs_pos;
                        }
                    }
                }
                self.commit_vi_undo();
                None
            }
            KeyCode::Char('~') => {
                self.save_vi_undo_point();
                if self.cursor < self.buffer.len() {
                    let c = self.buffer[self.cursor..].chars().next().unwrap();
                    let toggled: String = if c.is_uppercase() {
                        c.to_lowercase().collect()
                    } else {
                        c.to_uppercase().collect()
                    };
                    let before: String = self.buffer.chars().take(self.cursor).collect();
                    let after: String = self.buffer.chars().skip(self.cursor + 1).collect();
                    self.buffer = CompactString::new(format!("{}{}{}", before, toggled, after));
                    self.cursor = next_char_boundary(&self.buffer, self.cursor);
                }
                self.commit_vi_undo();
                None
            }
            KeyCode::Char('.') => {
                if let Some(ref repeat) = self.vi_state.repeat.clone() {
                    let cnt = self.vi_state.pending_count.max(1) as usize;
                    self.vi_state.pending_count = 0;
                    for _ in 0..cnt {
                        self.save_vi_undo_point();
                        match &repeat.op {
                            RepeatOp::Change(new_text, new_cursor) => {
                                self.buffer = new_text.clone();
                                self.cursor = *new_cursor;
                            }
                            RepeatOp::Delete(saved_cursor, saved_len) => {
                                let end = *saved_cursor + saved_len;
                                if end <= self.buffer.len() {
                                    let mut s =
                                        String::with_capacity(self.buffer.len() - saved_len);
                                    s.push_str(&self.buffer[..*saved_cursor]);
                                    s.push_str(&self.buffer[end..]);
                                    self.buffer = CompactString::new(&s);
                                    self.cursor = *saved_cursor;
                                }
                            }
                            RepeatOp::Paste => {
                                let text = self
                                    .vi_state
                                    .registers
                                    .get('"')
                                    .unwrap_or_default()
                                    .to_string();
                                if !text.is_empty() {
                                    let insert_pos = next_char_boundary(&self.buffer, self.cursor);
                                    self.buffer.insert_str(insert_pos, &text);
                                    self.cursor = insert_pos + text.len();
                                }
                            }
                        }
                        self.commit_vi_undo();
                    }
                }
                None
            }
            KeyCode::Char('"') => {
                self.vi_state.pending_register = Some('"'); // will be overwritten by next char
                None
            }
            KeyCode::Char('m') => {
                // Set mark: wait for next char a-z
                // For now, store at marks[0] as placeholder
                None
            }
            KeyCode::Char('\'') | KeyCode::Char('`') => {
                // Jump to mark: wait for next char
                None
            }

            // ---- Visual mode ----
            KeyCode::Char('v') => {
                self.vi_state.visual_start = self.cursor;
                self.vi_state.mode = ViMode::Visual(VisualType::Char);
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('V') => {
                self.vi_state.visual_start = self.cursor;
                self.vi_state.mode = ViMode::Visual(VisualType::Line);
                self.update_vi_mode_label();
                None
            }

            // ---- Command line / search ----
            KeyCode::Char(':') => {
                self.vi_state.cmdline_buffer = CompactString::new("");
                self.vi_state.cmdline_cursor = 0;
                self.vi_state.mode = ViMode::CommandLine;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('/') => {
                self.vi_state.cmdline_buffer = CompactString::new("");
                self.vi_state.cmdline_cursor = 0;
                self.vi_state.mode = ViMode::SearchForward;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('?') => {
                self.vi_state.cmdline_buffer = CompactString::new("");
                self.vi_state.cmdline_cursor = 0;
                self.vi_state.mode = ViMode::SearchBackward;
                self.update_vi_mode_label();
                None
            }

            // ---- g prefix ----
            KeyCode::Char('g') => {
                self.vi_state.pending_g = true;
                None
            }

            // ---- # insert-comment (prepend # and accept) ----
            KeyCode::Char('#') => {
                self.buffer.insert_str(0, "# ");
                let text = self.buffer.clone();
                self.history.push(text.clone());
                self.history_pos = None;
                self.draft = None;
                self.buffer.clear();
                self.cursor = 0;
                self.yank_pos = None;
                return Some(text);
            }

            // ---- * word-under-cursor search forward ----
            KeyCode::Char('*') => {
                let word = extract_word_at_cursor(&self.buffer, self.cursor);
                if let Some(word) = word {
                    self.vi_state.last_search = Some(word.clone());
                    let start = self.cursor + 1;
                    if start < self.buffer.len() {
                        if let Some(pos) = self.buffer[start..].find(&word) {
                            self.cursor = start + pos;
                        }
                    }
                }
                None
            }

            // ---- _ yank last arg from history ----
            KeyCode::Char('_') => {
                let cnt = self.vi_state.pending_count.max(1) as usize;
                self.vi_state.pending_count = 0;
                let text = self.history.last().and_then(|last_entry| {
                    let words: Vec<&str> = last_entry.split_whitespace().collect();
                    if words.is_empty() {
                        None
                    } else {
                        let idx = words.len().saturating_sub(cnt);
                        Some(words[idx].to_string())
                    }
                });
                if let Some(text) = text {
                    self.save_vi_undo_point();
                    self.buffer.insert_str(self.cursor, &text);
                    self.cursor += text.len();
                    self.commit_vi_undo();
                }
                None
            }

            // ---- f/F/t/T prefix ----
            KeyCode::Char('f') => {
                self.vi_state.pending_fchar_dir = Some(Direction::Forward);
                None
            }
            KeyCode::Char('F') => {
                self.vi_state.pending_fchar_dir = Some(Direction::Backward);
                None
            }
            KeyCode::Char('t') => {
                self.vi_state.pending_fchar_dir = Some(Direction::Forward);
                self.vi_state.pending_fchar_is_t = true;
                None
            }
            KeyCode::Char('T') => {
                self.vi_state.pending_fchar_dir = Some(Direction::Backward);
                self.vi_state.pending_fchar_is_t = true;
                None
            }

            // ---- Count digits ----
            KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                self.vi_state.pending_count =
                    self.vi_state.pending_count * 10 + (c as u32 - b'0' as u32);
                None
            }

            // ---- n/N for search repeat (with count support) ----
            KeyCode::Char('n') => {
                if let Some(ref search) = self.vi_state.last_search {
                    let cnt = self.vi_state.pending_count.max(1) as usize;
                    self.vi_state.pending_count = 0;
                    let start = self.cursor + 1;
                    if start < self.buffer.len() {
                        let mut pos = start;
                        for _ in 0..cnt {
                            if let Some(found) = self.buffer[pos..].find(search.as_str()) {
                                pos = pos + found + 1;
                            } else {
                                break;
                            }
                        }
                        if pos > start {
                            self.cursor = pos - 1;
                        }
                    }
                }
                None
            }
            KeyCode::Char('N') => {
                if let Some(ref search) = self.vi_state.last_search {
                    let cnt = self.vi_state.pending_count.max(1) as usize;
                    self.vi_state.pending_count = 0;
                    let end = self.cursor;
                    let mut pos = end;
                    for _ in 0..cnt {
                        if pos > 0 {
                            if let Some(found) = self.buffer[..pos].rfind(search.as_str()) {
                                pos = found;
                            } else {
                                break;
                            }
                        }
                    }
                    if pos < end {
                        self.cursor = pos;
                    }
                }
                None
            }

            _ => {
                self.vi_state.pending_op = None;
                self.vi_state.pending_count = 0;
                None
            }
        }
    }

    #[cfg(feature = "vi-mode")]
    fn handle_vi_visual_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        // Arrow keys and Ctrl+N/P navigate history in all modes
        if matches!(key.code, KeyCode::Up | KeyCode::Down)
            || (matches!(key.code, KeyCode::Char('n') | KeyCode::Char('p'))
                && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return self.handle_key_impl(key);
        }

        // Accumulate digit counts, like normal mode
        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() && c != '0' {
                self.vi_state.pending_count =
                    self.vi_state.pending_count * 10 + (c as u32 - b'0' as u32);
                return None;
            }
            if c == '0' && self.vi_state.pending_count > 0 {
                self.vi_state.pending_count *= 10;
                return None;
            }
        }

        // Let shared method handle motions and pending prefixes (gg, f/F/t/T)
        if self.handle_vi_motion(key.code) {
            return None;
        }

        match key.code {
            KeyCode::Esc => {
                self.vi_state.mode = ViMode::Normal;
                self.vi_state.pending_op = None;
                self.vi_state.pending_g = false;
                self.vi_state.pending_fchar_dir = None;
                self.vi_state.pending_fchar_is_t = false;
                self.update_vi_mode_label();
                None
            }

            // Visual operators
            KeyCode::Char('d') => {
                self.save_last_visual_selection();
                self.save_vi_undo_point();
                let (start, end) = self.vi_selection_range();
                if end > start {
                    let (new_buf, deleted) = delete_range(&self.buffer, start, end);
                    self.vi_state.registers.set('"', deleted);
                    self.buffer = new_buf;
                    self.cursor = start.min(self.buffer.len());
                }
                self.commit_vi_undo();
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('y') => {
                self.save_last_visual_selection();
                let (start, end) = self.vi_selection_range();
                if end > start {
                    let yanked = yank_range(&self.buffer, start, end);
                    self.vi_state.registers.set('"', yanked);
                }
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('c') => {
                // Don't save visual selection for gv (we enter insert mode)
                self.save_vi_undo_point();
                let (start, end) = self.vi_selection_range();
                if end > start {
                    let new_buf = {
                        let mut s = String::with_capacity(self.buffer.len() - (end - start));
                        s.push_str(&self.buffer[..start]);
                        s.push_str(&self.buffer[end..]);
                        CompactString::new(&s)
                    };
                    self.buffer = new_buf;
                    self.cursor = start;
                }
                self.vi_state.mode = ViMode::Insert;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Char('~') => {
                self.save_last_visual_selection();
                self.save_vi_undo_point();
                let (start, end) = self.vi_selection_range();
                let toggled: String = self.buffer[start..end]
                    .chars()
                    .map(|c| {
                        if c.is_uppercase() {
                            c.to_lowercase().next().unwrap_or(c)
                        } else {
                            c.to_uppercase().next().unwrap_or(c)
                        }
                    })
                    .collect();
                let mut s = String::with_capacity(self.buffer.len());
                s.push_str(&self.buffer[..start]);
                s.push_str(&toggled);
                s.push_str(&self.buffer[end..]);
                self.buffer = CompactString::new(&s);
                self.commit_vi_undo();
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }

            // Visual o — swap cursor and visual_start
            KeyCode::Char('o') => {
                let tmp = self.vi_state.visual_start;
                self.vi_state.visual_start = self.cursor;
                self.cursor = tmp;
                None
            }

            // Visual > — indent
            KeyCode::Char('>') => {
                self.save_last_visual_selection();
                self.save_vi_undo_point();
                let (start, end) = self.vi_selection_range();
                let (line_s, _) = cursor_to_line_col(&self.buffer, start);
                let (line_e, _) =
                    cursor_to_line_col(&self.buffer, if end > 0 { end - 1 } else { 0 });
                let start_byte = line_start(&self.buffer, start);
                let mut byte_pos = start_byte;
                for _ in line_s..=line_e {
                    self.buffer.insert_str(byte_pos, "  ");
                    byte_pos += 2;
                    if byte_pos < self.buffer.len() {
                        byte_pos = next_char_boundary(&self.buffer, byte_pos);
                    }
                }
                if self.cursor >= start && end > start {
                    self.cursor += (line_e - line_s + 1) * 2;
                }
                self.commit_vi_undo();
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }

            // Visual < — dedent
            KeyCode::Char('<') => {
                self.save_last_visual_selection();
                self.save_vi_undo_point();
                let (start, end) = self.vi_selection_range();
                let (line_s, _) = cursor_to_line_col(&self.buffer, start);
                let (line_e, _) =
                    cursor_to_line_col(&self.buffer, if end > 0 { end - 1 } else { 0 });
                let start_byte = line_start(&self.buffer, start);
                let mut byte_pos = start_byte;
                for _ in line_s..=line_e {
                    if byte_pos + 2 <= self.buffer.len()
                        && &self.buffer[byte_pos..byte_pos + 2] == "  "
                    {
                        let mut s = String::with_capacity(self.buffer.len() - 2);
                        s.push_str(&self.buffer[..byte_pos]);
                        s.push_str(&self.buffer[byte_pos + 2..]);
                        self.buffer = CompactString::new(&s);
                    }
                    if byte_pos < self.buffer.len() {
                        byte_pos = next_char_boundary(&self.buffer, byte_pos);
                    }
                }
                if self.cursor >= start && end > start {
                    self.cursor = self.cursor.saturating_sub((line_e - line_s + 1) * 2);
                }
                self.commit_vi_undo();
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                None
            }

            _ => None,
        }
    }

    #[cfg(feature = "vi-mode")]
    fn vi_selection_range(&self) -> (usize, usize) {
        let start = self.vi_state.visual_start;
        let end = self.cursor;
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }

    #[cfg(feature = "vi-mode")]
    pub fn vi_visual_selection(&self) -> Option<(usize, usize)> {
        if matches!(self.vi_state.mode, ViMode::Visual(_)) {
            Some(self.vi_selection_range())
        } else {
            None
        }
    }

    #[cfg(feature = "vi-mode")]
    fn handle_vi_cmdline_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        match key.code {
            KeyCode::Esc => {
                self.vi_state.mode = ViMode::Normal;
                self.vi_state.pending_op = None;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Enter => {
                let cmd = self.vi_state.cmdline_buffer.clone();
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                // Handle basic commands
                match cmd.trim() {
                    "q" | "quit" => {
                        // Signal quit — return a special value?
                        // For now, no-op in the input handler
                    }
                    "w" | "write" => {
                        // Session save is handled externally
                    }
                    "wq" => {
                        // Save + quit
                    }
                    _ => {}
                }
                None
            }
            KeyCode::Backspace => {
                if self.vi_state.cmdline_cursor > 0 {
                    self.vi_state.cmdline_cursor -= 1;
                    let idx = self
                        .vi_state
                        .cmdline_buffer
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.vi_state.cmdline_buffer.truncate(idx);
                }
                None
            }
            KeyCode::Char(c) => {
                self.vi_state.cmdline_buffer.push(c);
                self.vi_state.cmdline_cursor += 1;
                None
            }
            _ => None,
        }
    }

    #[cfg(feature = "vi-mode")]
    fn handle_vi_search_key(&mut self, key: KeyEvent) -> Option<CompactString> {
        match key.code {
            KeyCode::Esc => {
                self.vi_state.mode = ViMode::Normal;
                self.vi_state.pending_op = None;
                self.update_vi_mode_label();
                None
            }
            KeyCode::Enter => {
                let search = self.vi_state.cmdline_buffer.clone();
                let forward = matches!(self.vi_state.mode, ViMode::SearchForward);
                self.vi_state.last_search = Some(search.to_string());
                self.vi_state.mode = ViMode::Normal;
                self.update_vi_mode_label();
                if !search.is_empty() {
                    if forward && self.cursor < self.buffer.len() {
                        if let Some(pos) = self.buffer[self.cursor + 1..].find(search.as_str()) {
                            self.cursor = self.cursor + 1 + pos;
                        }
                    } else if !forward {
                        if let Some(pos) = self.buffer[..self.cursor].rfind(search.as_str()) {
                            self.cursor = pos;
                        }
                    }
                }
                None
            }
            KeyCode::Backspace => {
                if self.vi_state.cmdline_cursor > 0 {
                    self.vi_state.cmdline_cursor -= 1;
                    let idx = self
                        .vi_state
                        .cmdline_buffer
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.vi_state.cmdline_buffer.truncate(idx);
                }
                None
            }
            KeyCode::Char(c) => {
                self.vi_state.cmdline_buffer.push(c);
                self.vi_state.cmdline_cursor += 1;
                None
            }
            _ => None,
        }
    }

    #[cfg(feature = "vi-mode")]
    fn apply_vi_motion_or_op(&mut self, motion: &str) {
        let count = self.vi_state.pending_count.max(1) as usize;
        self.vi_state.pending_count = 0;

        if let Some(op) = self.vi_state.pending_op {
            let register = self.vi_state.pending_register.take();
            if let Some((new_buf, new_cursor, deleted)) = apply_operator(
                &self.buffer,
                self.cursor,
                op,
                motion,
                count as u32,
                &mut self.vi_state,
            ) {
                self.vi_state.pending_op = None; // only clear on success
                if !deleted.is_empty() {
                    self.vi_state.registers.set('"', deleted.clone());
                    if let Some(r) = register {
                        self.vi_state.registers.set(r, deleted);
                    }
                }
                self.buffer = new_buf;
                self.cursor = new_cursor;
            } // else: keep pending_op for retry
        } else if let Some(new_cursor) =
            apply_motion(&self.buffer, self.cursor, count, motion, &self.vi_state)
        {
            self.cursor = new_cursor;
        }
        self.vi_state.pending_register = None;
    }

    #[cfg(feature = "vi-mode")]
    fn apply_vi_operator(&mut self, motion: &str) {
        let op = match self.vi_state.pending_op {
            Some(o) => o,
            None => return,
        };
        self.vi_state.pending_op = None;

        let register = self.vi_state.pending_register.take();
        if let Some((new_buf, new_cursor, deleted)) =
            apply_operator(&self.buffer, self.cursor, op, motion, 1, &mut self.vi_state)
        {
            if !deleted.is_empty() {
                self.vi_state.registers.set('"', deleted.clone());
                if let Some(r) = register {
                    self.vi_state.registers.set(r, deleted);
                }
            }
            self.buffer = new_buf;
            self.cursor = new_cursor;
        }
    }

    #[cfg(feature = "vi-mode")]
    fn save_last_visual_selection(&mut self) {
        if matches!(self.vi_state.mode, ViMode::Visual(_)) {
            let (s, e) = self.vi_selection_range();
            self.vi_state.last_visual_start = Some(s);
            self.vi_state.last_visual_end = Some(e);
        }
    }
}

#[cfg(feature = "vi-mode")]
fn extract_word_at_cursor(buf: &str, cursor: usize) -> Option<String> {
    use crate::ui::input::cursor::next_char_boundary;
    if buf.is_empty() || cursor >= buf.len() {
        return None;
    }
    let bytes = buf.as_bytes();
    let mut start = cursor;
    // Scan backward to find word start (alphanumeric or _)
    while start > 0 {
        let c = bytes[start];
        if !c.is_ascii_alphanumeric() && c != b'_' {
            break;
        }
        start -= 1;
    }
    // Check the char we stopped at
    if start < buf.len() {
        let c = bytes[start];
        if !c.is_ascii_alphanumeric() && c != b'_' {
            start = start.saturating_add(1);
        }
    }
    // Scan forward to find word end
    let mut end = start;
    while end < buf.len() {
        let c = bytes[end];
        if c.is_ascii_alphanumeric() || c == b'_' {
            end += 1;
        } else {
            break;
        }
    }
    if end > start {
        Some(buf[start..end].to_string())
    } else {
        None
    }
}
