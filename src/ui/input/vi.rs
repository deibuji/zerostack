#![allow(dead_code)]

use compact_str::CompactString;

use crate::ui::input::cursor::{
    count_lines, cursor_to_line_col, line_col_to_cursor, line_end, line_start, next_char_boundary,
    prev_char_boundary,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViMode {
    Insert,
    Normal,
    Visual(VisualType),
    CommandLine,
    SearchForward,
    SearchBackward,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VisualType {
    Char,
    Line,
    #[allow(dead_code)]
    Block,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViOperator {
    Delete,
    Yank,
    Change,
    IndentRight,
    IndentLeft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    Word,
    WORD,
    Sentence,
    Paragraph,
    Quotes(char),
    Brackets(char, char),
    Tag,
}

#[derive(Clone)]
pub struct UndoEntry {
    pub old_buffer: CompactString,
    pub new_buffer: CompactString,
    pub old_cursor: usize,
    pub new_cursor: usize,
}

const MAX_UNDO: usize = 100;

#[derive(Clone)]
pub struct Registers {
    pub named: [Option<CompactString>; 36],
    pub unnamed: CompactString,
    pub yank: CompactString,
    pub small_delete: CompactString,
    pub clipboard: Option<CompactString>,
}

impl Registers {
    fn new() -> Self {
        Self {
            named: [const { None }; 36],
            unnamed: CompactString::new(""),
            yank: CompactString::new(""),
            small_delete: CompactString::new(""),
            clipboard: None,
        }
    }

    pub fn get(&self, name: char) -> Option<&str> {
        match name {
            '"' => Some(self.unnamed.as_str()),
            '0' => Some(self.yank.as_str()),
            '1' => Some(self.small_delete.as_str()),
            '+' => self.clipboard.as_deref(),
            'a'..='z' => self.named[name as usize - 'a' as usize].as_deref(),
            _ => None,
        }
    }

    pub fn set(&mut self, name: char, text: CompactString) {
        match name {
            '"' => self.unnamed = text,
            '0' => self.yank = text,
            '1' => self.small_delete = text,
            '+' => self.clipboard = Some(text),
            'a'..='z' => self.named[name as usize - 'a' as usize] = Some(text),
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct RepeatInfo {
    pub op: RepeatOp,
    pub text: CompactString,
    pub cursor: usize,
}

#[derive(Clone)]
pub enum RepeatOp {
    Change(CompactString, usize),
    Delete(usize, usize),
    Paste,
}

pub struct ViState {
    pub mode: ViMode,
    pub pending_op: Option<ViOperator>,
    pub pending_count: u32,
    pub pending_register: Option<char>,
    pub pending_text_object: bool,
    pub pending_g: bool,
    pub pending_fchar_dir: Option<Direction>,
    pub pending_fchar_is_t: bool,
    pub last_fchar: Option<char>,
    pub last_fchar_dir: Option<Direction>,
    pub last_fchar_is_t: bool,
    pub last_search: Option<String>,
    pub last_search_dir: Option<Direction>,
    pub visual_start: usize,
    pub registers: Registers,
    pub undo_stack: Vec<UndoEntry>,
    pub redo_stack: Vec<UndoEntry>,
    pub repeat: Option<RepeatInfo>,
    pub last_insert: Option<CompactString>,
    pub last_insert_cursor: usize,
    pub marks: [Option<usize>; 26],
    pub cmdline_buffer: CompactString,
    pub cmdline_cursor: usize,
    pub search_matches: Vec<usize>,
    pub search_match_idx: Option<usize>,
    pub insert_start_buf: CompactString,
    pub insert_start_cursor: usize,
}

impl ViState {
    pub fn new() -> Self {
        Self {
            mode: ViMode::Insert,
            pending_op: None,
            pending_count: 0,
            pending_register: None,
            pending_text_object: false,
            pending_g: false,
            pending_fchar_dir: None,
            pending_fchar_is_t: false,
            last_fchar: None,
            last_fchar_dir: None,
            last_fchar_is_t: false,
            last_search: None,
            last_search_dir: None,
            visual_start: 0,
            registers: Registers::new(),
            undo_stack: Vec::with_capacity(MAX_UNDO),
            redo_stack: Vec::with_capacity(MAX_UNDO),
            repeat: None,
            last_insert: None,
            last_insert_cursor: 0,
            marks: [None; 26],
            cmdline_buffer: CompactString::new(""),
            cmdline_cursor: 0,
            search_matches: Vec::new(),
            search_match_idx: None,
            insert_start_buf: CompactString::new(""),
            insert_start_cursor: 0,
        }
    }

    pub fn push_undo(&mut self, old: &str, new: &str, old_cursor: usize, new_cursor: usize) {
        if self.undo_stack.len() >= MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(UndoEntry {
            old_buffer: CompactString::new(old),
            new_buffer: CompactString::new(new),
            old_cursor,
            new_cursor,
        });
        self.redo_stack.clear();
    }

    pub fn mode_label(&self) -> &str {
        match self.mode {
            ViMode::Insert => "INSERT",
            ViMode::Normal => "NORMAL",
            ViMode::Visual(VisualType::Char) => "VISUAL",
            ViMode::Visual(VisualType::Line) => "VISUAL LINE",
            ViMode::Visual(VisualType::Block) => "VISUAL BLOCK",
            ViMode::CommandLine => "COMMAND",
            ViMode::SearchForward => "SEARCH",
            ViMode::SearchBackward => "SEARCH",
        }
    }

    pub fn cmdline_prompt(&self) -> &str {
        match self.mode {
            ViMode::CommandLine => ":",
            ViMode::SearchForward => "/",
            ViMode::SearchBackward => "?",
            _ => ":",
        }
    }

    /// Find matching bracket for ()[]{}
    pub fn match_bracket(buf: &str, cursor: usize) -> Option<usize> {
        let chars: Vec<char> = buf.chars().collect();
        let ch = chars.get(cursor)?;
        let (open, close, forward) = match ch {
            '(' => ('(', ')', true),
            ')' => (')', '(', false),
            '[' => ('[', ']', true),
            ']' => (']', '[', false),
            '{' => ('{', '}', true),
            '}' => ('}', '{', false),
            _ => return None,
        };
        let mut depth = 1usize;
        if forward {
            let mut i = cursor + 1;
            while i < chars.len() {
                if chars[i] == open {
                    depth += 1;
                } else if chars[i] == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                i += 1;
            }
        } else {
            if cursor == 0 {
                return None;
            }
            let mut i = cursor.wrapping_sub(1);
            loop {
                if chars[i] == open {
                    depth += 1;
                } else if chars[i] == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                if i == 0 {
                    break;
                }
                i -= 1;
            }
        }
        None
    }

    /// Find next word start
    pub fn next_word_start(buf: &str, cursor: usize, big: bool) -> usize {
        let chars: Vec<char> = buf.chars().collect();
        let len = chars.len();
        if cursor >= len {
            return len;
        }
        let mut i = cursor;
        // Skip current word
        while i < len
            && !chars[i].is_whitespace()
            && (big || chars[i].is_alphanumeric() || chars[i] == '_')
        {
            i += 1;
        }
        // Skip whitespace
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        i.min(len)
    }

    /// Find previous word start
    pub fn prev_word_start(buf: &str, cursor: usize, big: bool) -> usize {
        let chars: Vec<char> = buf.chars().collect();
        if cursor == 0 {
            return 0;
        }
        let mut i = cursor.min(chars.len().saturating_sub(1));
        // Skip whitespace before cursor
        while i > 0 && chars[i].is_whitespace() {
            i -= 1;
        }
        // Skip current word backwards
        while i > 0
            && !chars[i].is_whitespace()
            && (big || chars[i].is_alphanumeric() || chars[i] == '_')
        {
            i -= 1;
        }
        if chars[i].is_whitespace() || (!big && !chars[i].is_alphanumeric() && chars[i] != '_') {
            i += 1;
        }
        i
    }

    /// Find next word end
    pub fn next_word_end(buf: &str, cursor: usize, big: bool) -> usize {
        let chars: Vec<char> = buf.chars().collect();
        let len = chars.len();
        if cursor >= len {
            return len;
        }
        let mut i = cursor;
        // Skip current word
        while i < len
            && !chars[i].is_whitespace()
            && (big || chars[i].is_alphanumeric() || chars[i] == '_')
        {
            i += 1;
        }
        if i < len
            && !chars[i].is_whitespace()
            && (big || !chars[i].is_alphanumeric() && chars[i] != '_')
        {
            i += 1;
        }
        i
    }

    /// Find next paragraph boundary
    pub fn next_paragraph(buf: &str, cursor: usize) -> usize {
        let chars: Vec<char> = buf.chars().collect();
        let len = chars.len();
        let mut i = cursor;
        while i < len {
            if chars[i] == '\n' && i + 1 < len && chars[i + 1] == '\n' {
                return i + 2;
            }
            i += 1;
        }
        len
    }

    /// Find previous paragraph boundary
    pub fn prev_paragraph(buf: &str, cursor: usize) -> usize {
        let chars: Vec<char> = buf.chars().collect();
        if cursor == 0 {
            return 0;
        }
        let mut i = cursor.saturating_sub(1);
        while i > 0 {
            if chars[i] == '\n' && i > 0 && chars[i - 1] == '\n' {
                return i - 1;
            }
            i -= 1;
        }
        0
    }

    /// Find first non-whitespace in line
    pub fn first_non_whitespace(buf: &str, cursor: usize) -> usize {
        let start = line_start(buf, cursor);
        let end = line_end(buf, cursor);
        let text = &buf[start..end];
        let offset = text.len() - text.trim_start().len();
        start + offset
    }

    /// Resolve a text object range
    pub fn resolve_text_object(
        buf: &str,
        cursor: usize,
        obj: TextObject,
        inner: bool,
    ) -> (usize, usize) {
        let chars: Vec<char> = buf.chars().collect();
        let len = chars.len();
        match obj {
            TextObject::Word => {
                if inner {
                    let start = Self::prev_word_start(buf, cursor, false);
                    let end = Self::next_word_end(buf, cursor, false);
                    (start, end)
                } else {
                    let start = Self::prev_word_start(buf, cursor, false);
                    let end = Self::next_word_start(buf, cursor + 1, false);
                    (start, end)
                }
            }
            TextObject::WORD => {
                if inner {
                    let start = Self::prev_word_start(buf, cursor, true);
                    let end = Self::next_word_end(buf, cursor, true);
                    (start, end)
                } else {
                    let start = Self::prev_word_start(buf, cursor, true);
                    let end = Self::next_word_start(buf, cursor + 1, true);
                    (start, end)
                }
            }
            TextObject::Paragraph => {
                let start = Self::prev_paragraph(buf, cursor);
                let end = Self::next_paragraph(buf, cursor);
                if inner {
                    // Skip leading blank lines
                    let mut inner_start = start;
                    while inner_start < end && chars[inner_start] == '\n' {
                        inner_start += 1;
                    }
                    // Skip trailing blank lines
                    let mut inner_end = end;
                    while inner_end > inner_start && chars[inner_end - 1] == '\n' {
                        inner_end -= 1;
                    }
                    (inner_start, inner_end)
                } else {
                    // Skip leading blank lines only
                    let mut content_start = start;
                    while content_start < end && chars[content_start] == '\n' {
                        content_start += 1;
                    }
                    (content_start, end)
                }
            }
            TextObject::Sentence => {
                // Simple sentence: delimited by .!? followed by whitespace or end
                let mut start = cursor;
                while start > 0 {
                    let prev = chars[start.saturating_sub(1)];
                    if prev == '.' || prev == '!' || prev == '?' {
                        break;
                    }
                    start -= 1;
                }
                // Skip leading whitespace
                while start < len && chars[start].is_whitespace() {
                    start += 1;
                }
                let mut end = cursor;
                while end < len {
                    if (chars[end] == '.' || chars[end] == '!' || chars[end] == '?')
                        && (end + 1 >= len || chars[end + 1].is_whitespace())
                    {
                        end += 1;
                        break;
                    }
                    end += 1;
                }
                (start, end)
            }
            TextObject::Quotes(q) => {
                let mut start = cursor;
                let mut found_start = false;
                while start > 0 {
                    start -= 1;
                    if chars[start] == q {
                        found_start = true;
                        break;
                    }
                }
                let mut end = cursor;
                let mut found_end = false;
                while end < len {
                    end += 1;
                    if end < len && chars[end] == q {
                        found_end = true;
                        break;
                    }
                }
                if inner && found_start && found_end {
                    (start + 1, end)
                } else if found_start && found_end {
                    (start, end + 1)
                } else {
                    (cursor, cursor)
                }
            }
            TextObject::Brackets(open, close) => {
                let mut start = cursor;
                let mut depth = 0;
                let mut found_start = false;
                while start > 0 {
                    start -= 1;
                    if chars[start] == close {
                        depth += 1;
                    } else if chars[start] == open {
                        if depth == 0 {
                            found_start = true;
                            break;
                        }
                        depth -= 1;
                    }
                }
                // If we didn't find an opening bracket before cursor, search forward
                if !found_start {
                    let mut forward = cursor;
                    while forward < len {
                        if chars[forward] == open {
                            start = forward;
                            found_start = true;
                            break;
                        }
                        forward += 1;
                    }
                }
                if !found_start {
                    return (cursor, cursor);
                }
                // Find matching close
                let mut end = start + 1;
                depth = 1;
                while end < len && depth > 0 {
                    if chars[end] == open {
                        depth += 1;
                    } else if chars[end] == close {
                        depth -= 1;
                    }
                    if depth > 0 {
                        end += 1;
                    }
                }
                if inner {
                    (start + 1, end)
                } else {
                    (start, end + 1)
                }
            }
            TextObject::Tag => {
                // Simple HTML/XML tag inner/outer
                let text = &buf[..len];
                // Find previous <, skipping > and closing tags
                let mut start = cursor;
                while start > 0 {
                    start -= 1;
                    if chars[start] == '>' {
                        continue;
                    }
                    if chars[start] == '<' {
                        // Skip closing tags (</...>)
                        if start + 1 < len && chars[start + 1] == '/' {
                            continue;
                        }
                        // Found opening tag
                        let tag_end = text[start..]
                            .find('>')
                            .map(|p| start + p + 1)
                            .unwrap_or(len);
                        // Find closing tag
                        let tag_name = &text[start + 1..tag_end - 1]
                            .split_whitespace()
                            .next()
                            .unwrap_or("");
                        let close_tag = format!("</{}>", tag_name);
                        if let Some(close_pos) = text[tag_end..].find(&close_tag) {
                            let close_end = tag_end + close_pos + close_tag.len();
                            if inner {
                                return (tag_end, tag_end + close_pos);
                            } else {
                                return (start, close_end);
                            }
                        }
                        break;
                    }
                }
                (cursor, cursor)
            }
        }
    }
}

#[derive(Clone)]
pub struct ViMotionResult {
    pub cursor: usize,
    pub buffer: CompactString,
}

/// Apply a motion and return the new cursor position
pub fn apply_motion(
    buf: &str,
    cursor: usize,
    count: usize,
    motion: &str,
    vi: &ViState,
) -> Option<usize> {
    let chars: Vec<char> = buf.chars().collect();
    let len = chars.len();
    let cnt = count.max(1);

    match motion {
        "h" | "left" => {
            let mut c = cursor;
            for _ in 0..cnt {
                if c > 0 {
                    c = prev_char_boundary(buf, c);
                }
            }
            Some(c)
        }
        "j" | "down" => {
            let (line, col) = cursor_to_line_col(buf, cursor);
            let total = count_lines(buf);
            let target = (line + cnt).min(total.saturating_sub(1));
            Some(line_col_to_cursor(buf, target, col))
        }
        "k" | "up" => {
            let (line, col) = cursor_to_line_col(buf, cursor);
            let target = line.saturating_sub(cnt);
            Some(line_col_to_cursor(buf, target, col))
        }
        "l" | "right" | "space" => {
            let mut c = cursor;
            for _ in 0..cnt {
                if c < len {
                    c = next_char_boundary(buf, c);
                }
            }
            Some(c)
        }
        "w" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::next_word_start(buf, c, false);
            }
            Some(c)
        }
        "b" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::prev_word_start(buf, c, false);
            }
            Some(c)
        }
        "e" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::next_word_end(buf, c, false);
            }
            Some(c.saturating_sub(1).max(cursor))
        }
        "W" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::next_word_start(buf, c, true);
            }
            Some(c)
        }
        "B" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::prev_word_start(buf, c, true);
            }
            Some(c)
        }
        "E" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::next_word_end(buf, c, true);
            }
            Some(c.saturating_sub(1).max(cursor))
        }
        "0" | "home" => Some(0),
        "$" | "end" => Some(len),
        "^" => Some(ViState::first_non_whitespace(buf, cursor)),
        "gg" => Some(0),
        "G" => Some(len),
        "{" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::prev_paragraph(buf, c);
            }
            Some(c)
        }
        "}" => {
            let mut c = cursor;
            for _ in 0..cnt {
                c = ViState::next_paragraph(buf, c);
            }
            Some(c)
        }
        "%" => ViState::match_bracket(buf, cursor),
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
        "F" => {
            if let Some(ch) = vi.last_fchar {
                let mut c = cursor.wrapping_sub(1);
                let mut found = 0u32;
                loop {
                    if chars[c] == ch {
                        found += 1;
                        if found >= cnt as u32 {
                            return Some(c);
                        }
                    }
                    if c == 0 {
                        break;
                    }
                    c -= 1;
                }
            }
            None
        }
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
        "T" => {
            if let Some(ch) = vi.last_fchar {
                let mut c = cursor.wrapping_sub(1);
                loop {
                    if chars[c] == ch {
                        return Some(c + 1);
                    }
                    if c == 0 {
                        break;
                    }
                    c -= 1;
                }
            }
            None
        }
        ";" => {
            if let Some(dir) = vi.last_fchar_dir {
                let m = match (dir, vi.last_fchar_is_t) {
                    (Direction::Forward, true) => "t",
                    (Direction::Forward, false) => "f",
                    (Direction::Backward, true) => "T",
                    (Direction::Backward, false) => "F",
                };
                apply_motion(buf, cursor, cnt, m, vi)
            } else {
                None
            }
        }
        "," => {
            if let Some(dir) = vi.last_fchar_dir {
                let m = match (dir, vi.last_fchar_is_t) {
                    (Direction::Forward, true) => "T",
                    (Direction::Forward, false) => "F",
                    (Direction::Backward, true) => "t",
                    (Direction::Backward, false) => "f",
                };
                apply_motion(buf, cursor, cnt, m, vi)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Get a range from cursor to motion target for an operator
pub fn motion_to_range(
    _buf: &str,
    cursor: usize,
    target: usize,
    exclusive: bool,
) -> (usize, usize) {
    if target > cursor {
        (cursor, if exclusive { target } else { target + 1 })
    } else {
        (target, if exclusive { cursor } else { cursor + 1 })
    }
}

/// Delete text in range [start, end) from buffer, return deleted text
pub fn delete_range(buf: &str, start: usize, end: usize) -> (CompactString, CompactString) {
    let mut s = String::with_capacity(buf.len().saturating_sub(end - start));
    s.push_str(&buf[..start]);
    s.push_str(&buf[end..]);
    let deleted: CompactString = buf[start..end].into();
    (CompactString::new(&s), deleted)
}

/// Yank text in range [start, end)
pub fn yank_range(buf: &str, start: usize, end: usize) -> CompactString {
    buf[start..end].into()
}

/// Apply operator + motion to buffer
pub fn apply_operator(
    buf: &str,
    cursor: usize,
    op: ViOperator,
    motion: &str,
    count: u32,
    vi: &mut ViState,
) -> Option<(CompactString, usize, CompactString)> {
    let motion_is_line = matches!(motion, "dd" | "cc" | "yy" | "D" | "C" | "S");

    if motion_is_line {
        let (line, _) = cursor_to_line_col(buf, cursor);
        let start = line_start(buf, cursor);
        // For dd/cc/yy, delete the whole line including newline unless it's the last line
        if motion == "D" || motion == "C" || motion == "S" {
            let end = line_end(buf, cursor);
            match op {
                ViOperator::Delete | ViOperator::Change => {
                    let (new_buf, deleted) = delete_range(buf, cursor, end);
                    return Some((new_buf, cursor, deleted));
                }
                _ => return None,
            }
        }
        if line + 1 < count_lines(buf) {
            let end = line_end(buf, start) + 1; // include newline
            match op {
                ViOperator::Delete => {
                    let (new_buf, deleted) = delete_range(buf, start, end);
                    let new_buf_len = new_buf.len();
                    Some((new_buf, start.min(new_buf_len), deleted))
                }
                ViOperator::Yank => {
                    let yanked = yank_range(buf, start, end);
                    Some((CompactString::new(buf), cursor, yanked))
                }
                ViOperator::Change => {
                    let (new_buf, _deleted) = delete_range(buf, start, end);
                    // Insert a blank line for cc
                    let mut ins = String::with_capacity(new_buf.len() + 1);
                    ins.push_str(&new_buf[..start]);
                    ins.push('\n');
                    ins.push_str(&new_buf[start..]);
                    let saved = buf[start..end].into();
                    Some((CompactString::new(&ins), start, saved))
                }
                _ => None,
            }
        } else {
            // Last line: delete just the line content
            let end = line_end(buf, start);
            match op {
                ViOperator::Delete => {
                    let (new_buf, deleted) = delete_range(buf, start, end);
                    let nb_len = new_buf.len();
                    Some((new_buf, start.min(nb_len), deleted))
                }
                ViOperator::Yank => {
                    let yanked = yank_range(buf, start, end);
                    Some((CompactString::new(buf), cursor, yanked))
                }
                ViOperator::Change => {
                    let (new_buf, _deleted) = delete_range(buf, start, end);
                    let saved = buf[start..end].into();
                    Some((new_buf, start, saved))
                }
                _ => None,
            }
        }
    } else if motion == "_" {
        // Single line operator (dd without motion)
        let start = line_start(buf, cursor);
        let end = if line_end(buf, start) + 1 < buf.len() {
            line_end(buf, start) + 1
        } else {
            line_end(buf, start)
        };
        match op {
            ViOperator::Delete => {
                let (new_buf, deleted) = delete_range(buf, start, end);
                let nb_len = new_buf.len();
                Some((new_buf, start.min(nb_len), deleted))
            }
            ViOperator::Yank => {
                let yanked = yank_range(buf, start, end);
                Some((CompactString::new(buf), cursor, yanked))
            }
            ViOperator::Change => {
                let (new_buf, _deleted) = delete_range(buf, start, end);
                let saved = buf[start..end].into();
                Some((new_buf, start, saved))
            }
            _ => None,
        }
    } else if let Some(target) = apply_motion(buf, cursor, count as usize, motion, vi) {
        let inclusive = motion == "e" || motion == "E";
        let (start, end) = motion_to_range(buf, cursor, target, !inclusive);
        match op {
            ViOperator::Delete => {
                let (new_buf, deleted) = delete_range(buf, start, end);
                let new_cursor = start.min(new_buf.len());
                Some((new_buf, new_cursor, deleted))
            }
            ViOperator::Yank => {
                let yanked = yank_range(buf, start, end);
                Some((CompactString::new(buf), cursor, yanked))
            }
            ViOperator::Change => {
                let (new_buf, _deleted) = delete_range(buf, start, end);
                let saved = CompactString::new(&buf[start..end]);
                Some((new_buf, start, saved))
            }
            _ => None,
        }
    } else {
        None
    }
}
