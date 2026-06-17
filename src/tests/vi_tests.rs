use crate::ui::input::vi::{
    Direction, TextObject, UndoEntry, ViMode, ViOperator, apply_motion, apply_operator,
    delete_range, motion_to_range, yank_range,
};
use compact_str::CompactString;

// ---------------------------------------------------------------------------
// ViState helpers (tested via the functions that use them)
// ---------------------------------------------------------------------------

fn make_vi() -> crate::ui::input::vi::ViState {
    crate::ui::input::vi::ViState::new()
}

// ---------------------------------------------------------------------------
// ViState::mode_label
// ---------------------------------------------------------------------------

#[test]
fn mode_label_insert() {
    let vi = make_vi(); // default is Insert
    assert_eq!(vi.mode_label(), "INSERT");
}

#[test]
fn mode_label_normal() {
    let mut vi = make_vi();
    vi.mode = ViMode::Normal;
    assert_eq!(vi.mode_label(), "NORMAL");
}

#[test]
fn mode_label_visual_char() {
    let mut vi = make_vi();
    vi.mode = crate::ui::input::vi::ViMode::Visual(crate::ui::input::vi::VisualType::Char);
    assert_eq!(vi.mode_label(), "VISUAL");
}

#[test]
fn mode_label_visual_line() {
    let mut vi = make_vi();
    vi.mode = crate::ui::input::vi::ViMode::Visual(crate::ui::input::vi::VisualType::Line);
    assert_eq!(vi.mode_label(), "VISUAL LINE");
}

#[test]
fn mode_label_cmdline() {
    let mut vi = make_vi();
    vi.mode = ViMode::CommandLine;
    assert_eq!(vi.mode_label(), "COMMAND");
}

#[test]
fn mode_label_search() {
    let mut vi = make_vi();
    vi.mode = ViMode::SearchForward;
    assert_eq!(vi.mode_label(), "SEARCH");
    vi.mode = ViMode::SearchBackward;
    assert_eq!(vi.mode_label(), "SEARCH");
}

// ---------------------------------------------------------------------------
// ViState::cmdline_prompt
// ---------------------------------------------------------------------------

#[test]
fn cmdline_prompt_colon() {
    let mut vi = make_vi();
    vi.mode = ViMode::CommandLine;
    assert_eq!(vi.cmdline_prompt(), ":");
}

#[test]
fn cmdline_prompt_forward_slash() {
    let mut vi = make_vi();
    vi.mode = ViMode::SearchForward;
    assert_eq!(vi.cmdline_prompt(), "/");
}

#[test]
fn cmdline_prompt_backward_slash() {
    let mut vi = make_vi();
    vi.mode = ViMode::SearchBackward;
    assert_eq!(vi.cmdline_prompt(), "?");
}

// ---------------------------------------------------------------------------
// ViState::push_undo
// ---------------------------------------------------------------------------

#[test]
fn push_undo_basic() {
    let mut vi = make_vi();
    vi.push_undo("hello", "world", 0, 5);
    assert_eq!(vi.undo_stack.len(), 1);
    assert_eq!(vi.redo_stack.len(), 0);
    let entry = &vi.undo_stack[0];
    assert_eq!(entry.old_buffer.as_str(), "hello");
    assert_eq!(entry.new_buffer.as_str(), "world");
    assert_eq!(entry.old_cursor, 0);
    assert_eq!(entry.new_cursor, 5);
}

#[test]
fn push_undo_clears_redo() {
    let mut vi = make_vi();
    vi.push_undo("a", "b", 0, 1);
    // Simulate redo push
    vi.redo_stack.push(UndoEntry {
        old_buffer: CompactString::new("b"),
        new_buffer: CompactString::new("a"),
        old_cursor: 1,
        new_cursor: 0,
    });
    vi.push_undo("b", "c", 1, 2);
    assert_eq!(vi.redo_stack.len(), 0);
}

#[test]
fn push_undo_max_capacity() {
    let mut vi = make_vi();
    for i in 0..200 {
        vi.push_undo(&i.to_string(), &(i + 1).to_string(), i, i + 1);
    }
    assert_eq!(vi.undo_stack.len(), 100);
    // Oldest entry (0->1) should have been dropped
    assert_eq!(vi.undo_stack[0].old_buffer.as_str(), "100");
}

// ---------------------------------------------------------------------------
// ViState::match_bracket
// ---------------------------------------------------------------------------

#[test]
fn match_bracket_parens_forward() {
    let buf = "func(a, b)";
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 4),
        Some(9)
    );
}

#[test]
fn match_bracket_parens_backward() {
    let buf = "func(a, b)";
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 9),
        Some(4)
    );
}

#[test]
fn match_bracket_curly_forward() {
    let buf = "{ hello }";
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 0),
        Some(8)
    );
}

#[test]
fn match_bracket_curly_backward() {
    let buf = "{ hello }";
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 8),
        Some(0)
    );
}

#[test]
fn match_bracket_square_backward() {
    let buf = "[0]";
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 2),
        Some(0)
    );
}

#[test]
fn match_bracket_nested() {
    let buf = "a(b(c)d)";
    // outer '(' at 1 matches outer ')' at 7
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 1),
        Some(7)
    );
    // outer ')' at 7 matches outer '(' at 1
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 7),
        Some(1)
    );
    // inner '(' at 3 matches inner ')' at 5
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 3),
        Some(5)
    );
    // inner ')' at 5 matches inner '(' at 3
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 5),
        Some(3)
    );
}

#[test]
fn match_bracket_no_match() {
    let buf = "abc";
    assert_eq!(crate::ui::input::vi::ViState::match_bracket(buf, 0), None);
    assert_eq!(crate::ui::input::vi::ViState::match_bracket(buf, 1), None);
}

#[test]
fn match_bracket_unmatched_open() {
    let buf = "(abc";
    assert_eq!(crate::ui::input::vi::ViState::match_bracket(buf, 0), None);
}

#[test]
fn match_bracket_unmatched_close() {
    let buf = "abc)";
    assert_eq!(crate::ui::input::vi::ViState::match_bracket(buf, 3), None);
}

#[test]
fn match_bracket_cursor_at_zero_backward() {
    // Cursor at 0 with ')' — no characters before 0, cannot match backward
    let buf = ")";
    assert_eq!(crate::ui::input::vi::ViState::match_bracket(buf, 0), None);
}

#[test]
fn match_bracket_non_bracket_char() {
    let buf = "hello";
    assert_eq!(crate::ui::input::vi::ViState::match_bracket(buf, 2), None);
}

#[test]
fn match_bracket_multiline() {
    let buf = "if (a\n     && b)";
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 3),
        Some(15)
    );
}

// ---------------------------------------------------------------------------
// ViState::next_word_start / prev_word_start / next_word_end
// ---------------------------------------------------------------------------

#[test]
fn next_word_start_basic() {
    let buf = "hello world foo";
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_start(buf, 0, false),
        6
    );
}

#[test]
fn next_word_start_twice() {
    let buf = "hello world foo";
    let w1 = crate::ui::input::vi::ViState::next_word_start(buf, 0, false);
    let w2 = crate::ui::input::vi::ViState::next_word_start(buf, w1, false);
    assert_eq!(w1, 6);
    assert_eq!(w2, 12);
}

#[test]
fn next_word_start_already_at_end() {
    let buf = "hi";
    let end = buf.len();
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_start(buf, end, false),
        end
    );
}

#[test]
fn next_word_start_empty() {
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_start("", 0, false),
        0
    );
}

#[test]
fn prev_word_start_basic() {
    let buf = "hello world";
    assert_eq!(
        crate::ui::input::vi::ViState::prev_word_start(buf, 10, false),
        6
    );
}

#[test]
fn prev_word_start_at_start() {
    let buf = "hello";
    assert_eq!(
        crate::ui::input::vi::ViState::prev_word_start(buf, 0, false),
        0
    );
}

#[test]
fn prev_word_start_empty() {
    assert_eq!(
        crate::ui::input::vi::ViState::prev_word_start("", 0, false),
        0
    );
}

#[test]
fn prev_word_start_in_middle_of_word() {
    let buf = "hello world";
    assert_eq!(
        crate::ui::input::vi::ViState::prev_word_start(buf, 7, false),
        6
    );
}

#[test]
fn next_word_end_basic() {
    let buf = "hello world";
    // Returns char index after the last char of the word (index of ' ')
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_end(buf, 0, false),
        5
    );
}

#[test]
fn next_word_end_twice() {
    let buf = "hello world foo";
    let e1 = crate::ui::input::vi::ViState::next_word_end(buf, 0, false);
    let e2 = crate::ui::input::vi::ViState::next_word_end(buf, e1 + 1, false);
    // e1 = 5 (end of "hello"), e2 = 11 (end of "world")
    assert_eq!(e1, 5);
    assert_eq!(e2, 11);
}

#[test]
fn next_word_end_big_word() {
    let buf = "foo-bar baz";
    // BIGWORD: non-whitespace delimiters are part of the word
    // "foo-bar" = 7 chars, next_word_end returns char index after last
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_end(buf, 0, true),
        7
    );
}

#[test]
fn next_word_end_empty() {
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_end("", 0, false),
        0
    );
}

#[test]
fn next_word_start_big_word() {
    let buf = "foo-bar baz";
    // 'W' treats non-whitespace delimiters as part of the word
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_start(buf, 0, true),
        8
    );
}

#[test]
fn prev_word_start_big_word() {
    let buf = "foo-bar baz";
    assert_eq!(
        crate::ui::input::vi::ViState::prev_word_start(buf, 10, true),
        8
    );
}

#[test]
fn next_word_start_with_underscore() {
    let buf = "my_var next";
    assert_eq!(
        crate::ui::input::vi::ViState::next_word_start(buf, 0, false),
        7
    );
}

// ---------------------------------------------------------------------------
// ViState::next_paragraph / prev_paragraph
// ---------------------------------------------------------------------------

#[test]
fn next_paragraph_basic() {
    let buf = "line1\n\nline2";
    assert_eq!(crate::ui::input::vi::ViState::next_paragraph(buf, 0), 7);
}

#[test]
fn next_paragraph_no_boundary() {
    let buf = "line1\nline2";
    assert_eq!(
        crate::ui::input::vi::ViState::next_paragraph(buf, 0),
        buf.len()
    );
}

#[test]
fn next_paragraph_already_at_end() {
    let buf = "hi";
    let end = buf.len();
    assert_eq!(crate::ui::input::vi::ViState::next_paragraph(buf, end), end);
}

#[test]
fn prev_paragraph_basic() {
    let buf = "line1\n\nline2";
    assert_eq!(crate::ui::input::vi::ViState::prev_paragraph(buf, 10), 5);
}

#[test]
fn prev_paragraph_at_start() {
    let buf = "hello";
    assert_eq!(crate::ui::input::vi::ViState::prev_paragraph(buf, 0), 0);
}

#[test]
fn prev_paragraph_no_boundary() {
    let buf = "hello world";
    assert_eq!(crate::ui::input::vi::ViState::prev_paragraph(buf, 5), 0);
}

#[test]
fn prev_paragraph_empty() {
    assert_eq!(crate::ui::input::vi::ViState::prev_paragraph("", 0), 0);
}

#[test]
fn next_paragraph_empty() {
    assert_eq!(crate::ui::input::vi::ViState::next_paragraph("", 0), 0);
}

// ---------------------------------------------------------------------------
// ViState::first_non_whitespace
// ---------------------------------------------------------------------------

#[test]
fn first_non_whitespace_basic() {
    let buf = "   hello";
    assert_eq!(
        crate::ui::input::vi::ViState::first_non_whitespace(buf, 0),
        3
    );
}

#[test]
fn first_non_whitespace_already_at_first_char() {
    let buf = "hello";
    assert_eq!(
        crate::ui::input::vi::ViState::first_non_whitespace(buf, 0),
        0
    );
}

#[test]
fn first_non_whitespace_cursor_on_same_line() {
    let buf = "   hello\n  world";
    // cursor on line 2
    assert_eq!(
        crate::ui::input::vi::ViState::first_non_whitespace(buf, 10),
        11
    );
}

#[test]
fn first_non_whitespace_only_spaces() {
    let buf = "     ";
    assert_eq!(
        crate::ui::input::vi::ViState::first_non_whitespace(buf, 0),
        buf.len()
    );
}

#[test]
fn first_non_whitespace_empty() {
    assert_eq!(
        crate::ui::input::vi::ViState::first_non_whitespace("", 0),
        0
    );
}

// ---------------------------------------------------------------------------
// ViState::resolve_text_object
// ---------------------------------------------------------------------------

#[test]
fn resolve_iw_in_word() {
    let buf = "some text";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(buf, 1, TextObject::Word, true);
    assert_eq!(&buf[s..e], "some");
}

#[test]
fn resolve_aw_in_word() {
    let buf = "some text";
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 1, TextObject::Word, false);
    assert_eq!(&buf[s..e], "some "); // includes trailing space
}

#[test]
fn resolve_iW() {
    let buf = "foo-bar baz";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(buf, 1, TextObject::WORD, true);
    assert_eq!(&buf[s..e], "foo-bar");
}

#[test]
fn resolve_aW() {
    let buf = "foo-bar baz";
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 1, TextObject::WORD, false);
    assert_eq!(&buf[s..e], "foo-bar "); // includes trailing space
}

#[test]
fn resolve_ip() {
    let buf = "para1\n\npara2\nstill para2\n\npara3";
    // cursor inside "para2"
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 10, TextObject::Paragraph, true);
    assert_eq!(&buf[s..e], "para2\nstill para2");
}

#[test]
fn resolve_ap() {
    let buf = "para1\n\npara2\n\npara3";
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 10, TextObject::Paragraph, false);
    assert_eq!(&buf[s..e], "para2\n\n"); // includes paragraph boundaries
}

#[test]
fn resolve_sentence() {
    let buf = "First. Second! Third?";
    // cursor in "Second"
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 10, TextObject::Sentence, true);
    assert_eq!(&buf[s..e], "Second!");
}

#[test]
fn resolve_quotes_inner() {
    let buf = r#"say "hello" world"#;
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 6, TextObject::Quotes('"'), true);
    assert_eq!(&buf[s..e], "hello");
}

#[test]
fn resolve_quotes_outer() {
    let buf = r#"say "hello" world"#;
    let (s, e) =
        crate::ui::input::vi::ViState::resolve_text_object(buf, 6, TextObject::Quotes('"'), false);
    assert_eq!(&buf[s..e], "\"hello\"");
}

#[test]
fn resolve_brackets_inner() {
    let buf = "func(a, b)";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(
        buf,
        6,
        TextObject::Brackets('(', ')'),
        true,
    );
    assert_eq!(&buf[s..e], "a, b");
}

#[test]
fn resolve_brackets_outer() {
    let buf = "func(a, b)";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(
        buf,
        6,
        TextObject::Brackets('(', ')'),
        false,
    );
    assert_eq!(&buf[s..e], "(a, b)");
}

#[test]
fn resolve_brackets_cursor_after_open() {
    let buf = "if (x > 0)";
    // Cursor is on the opening paren itself — should still find the pair
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(
        buf,
        3,
        TextObject::Brackets('(', ')'),
        true,
    );
    assert_eq!(&buf[s..e], "x > 0");
}

#[test]
fn resolve_brackets_cursor_before_close_nested() {
    let buf = "a(b(c)d)";
    // cursor inside inner parens
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(
        buf,
        4,
        TextObject::Brackets('(', ')'),
        true,
    );
    assert_eq!(&buf[s..e], "c");
}

#[test]
fn resolve_brackets_cursor_before_open() {
    let buf = "a(b)";
    // cursor before opening bracket should search forward
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(
        buf,
        0,
        TextObject::Brackets('(', ')'),
        true,
    );
    assert_eq!(&buf[s..e], "b");
}

#[test]
fn resolve_tag_inner() {
    let buf = "<div>hello</div>";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(buf, 7, TextObject::Tag, true);
    assert_eq!(&buf[s..e], "hello");
}

#[test]
fn resolve_tag_outer() {
    let buf = "<div>hello</div>";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(buf, 7, TextObject::Tag, false);
    assert_eq!(&buf[s..e], "<div>hello</div>");
}

#[test]
fn resolve_tag_no_match() {
    let buf = "hello";
    let (s, e) = crate::ui::input::vi::ViState::resolve_text_object(buf, 3, TextObject::Tag, true);
    assert_eq!(&buf[s..e], "");
    assert_eq!(s, 3);
    assert_eq!(e, 3);
}

// ---------------------------------------------------------------------------
// apply_motion
// ---------------------------------------------------------------------------

#[test]
fn motion_h() {
    let vi = make_vi();
    assert_eq!(apply_motion("hi", 1, 1, "h", &vi), Some(0));
}

#[test]
fn motion_h_at_start() {
    let vi = make_vi();
    assert_eq!(apply_motion("hi", 0, 1, "h", &vi), Some(0));
}

#[test]
fn motion_j() {
    let vi = make_vi();
    assert_eq!(apply_motion("a\nb\nc", 0, 1, "j", &vi), Some(2));
}

#[test]
fn motion_j_beyond_end() {
    let vi = make_vi();
    assert_eq!(apply_motion("a\nb", 0, 10, "j", &vi), Some(2));
}

#[test]
fn motion_k() {
    let vi = make_vi();
    assert_eq!(apply_motion("a\nb\nc", 4, 1, "k", &vi), Some(2));
}

#[test]
fn motion_k_at_start() {
    let vi = make_vi();
    assert_eq!(apply_motion("a\nb", 0, 1, "k", &vi), Some(0));
}

#[test]
fn motion_l() {
    let vi = make_vi();
    assert_eq!(apply_motion("hi", 0, 1, "l", &vi), Some(1));
}

#[test]
fn motion_l_at_end() {
    let vi = make_vi();
    assert_eq!(apply_motion("hi", 2, 1, "l", &vi), Some(2));
}

#[test]
fn motion_w() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello world", 0, 1, "w", &vi), Some(6));
}

#[test]
fn motion_w_count() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello world foo", 0, 2, "w", &vi), Some(12));
}

#[test]
fn motion_b() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello world", 7, 1, "b", &vi), Some(6));
}

#[test]
fn motion_e() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello world", 0, 1, "e", &vi), Some(4));
}

#[test]
fn motion_0() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello", 4, 1, "0", &vi), Some(0));
}

#[test]
fn motion_dollar() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello", 0, 1, "$", &vi), Some(5));
}

#[test]
fn motion_caret() {
    let vi = make_vi();
    assert_eq!(apply_motion("   hi", 0, 1, "^", &vi), Some(3));
}

#[test]
fn motion_gg() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello\nworld", 6, 1, "gg", &vi), Some(0));
}

#[test]
fn motion_G() {
    let vi = make_vi();
    assert_eq!(apply_motion("hello\nworld", 0, 1, "G", &vi), Some(11));
}

#[test]
fn motion_curly_open() {
    let vi = make_vi();
    let buf = "a\n\nb";
    assert_eq!(apply_motion(buf, 3, 1, "{", &vi), Some(1));
}

#[test]
fn motion_curly_close() {
    let vi = make_vi();
    let buf = "a\n\nb";
    assert_eq!(apply_motion(buf, 0, 1, "}", &vi), Some(3));
}

#[test]
fn motion_percent() {
    let vi = make_vi();
    assert_eq!(apply_motion("(hello)", 0, 1, "%", &vi), Some(6));
}

#[test]
fn motion_percent_no_match() {
    let vi = make_vi();
    assert_eq!(apply_motion("abc", 0, 1, "%", &vi), None);
}

#[test]
fn motion_unknown() {
    let vi = make_vi();
    assert_eq!(apply_motion("hi", 0, 1, "z", &vi), None);
}

#[test]
fn motion_empty_buffer() {
    let vi = make_vi();
    assert_eq!(apply_motion("", 0, 1, "j", &vi), Some(0));
    assert_eq!(apply_motion("", 0, 1, "k", &vi), Some(0));
    assert_eq!(apply_motion("", 0, 1, "h", &vi), Some(0));
    assert_eq!(apply_motion("", 0, 1, "l", &vi), Some(0));
}

// f/F/t/T motions require ViState with last_fchar set
#[test]
fn motion_f() {
    let mut vi = make_vi();
    vi.last_fchar = Some('o');
    assert_eq!(apply_motion("hello world", 3, 1, "f", &vi), Some(4));
}

#[test]
fn motion_F() {
    let mut vi = make_vi();
    vi.last_fchar = Some('e');
    assert_eq!(apply_motion("hello world", 6, 1, "F", &vi), Some(1));
}

#[test]
fn motion_t() {
    let mut vi = make_vi();
    vi.last_fchar = Some('o');
    assert_eq!(apply_motion("hello world", 3, 1, "t", &vi), Some(3));
}

#[test]
fn motion_T() {
    let mut vi = make_vi();
    vi.last_fchar = Some('e');
    assert_eq!(apply_motion("hello world", 6, 1, "T", &vi), Some(2));
}

#[test]
fn motion_semicolon_repeats_f() {
    let mut vi = make_vi();
    vi.last_fchar = Some('l');
    vi.last_fchar_dir = Some(Direction::Forward);
    vi.last_fchar_is_t = false;
    assert_eq!(apply_motion("hello world", 0, 1, ";", &vi), Some(2));
}

#[test]
fn motion_comma_reverses_f() {
    let mut vi = make_vi();
    vi.last_fchar = Some('l');
    vi.last_fchar_dir = Some(Direction::Forward);
    vi.last_fchar_is_t = false;
    // Comma reverses direction: becomes 'F' backward from cursor 3
    // First 'l' backward from position 3 is at position 2
    assert_eq!(apply_motion("hello world", 3, 1, ",", &vi), Some(2));
}

#[test]
fn motion_f_count_finds_nth() {
    let mut vi = make_vi();
    vi.last_fchar = Some('l');
    assert_eq!(apply_motion("hello world", 0, 3, "f", &vi), Some(9));
}

// ---------------------------------------------------------------------------
// motion_to_range
// ---------------------------------------------------------------------------

#[test]
fn motion_range_forward() {
    assert_eq!(motion_to_range("test", 0, 4, true), (0, 4));
}

#[test]
fn motion_range_forward_inclusive() {
    assert_eq!(motion_to_range("test", 0, 4, false), (0, 5));
}

#[test]
fn motion_range_backward() {
    assert_eq!(motion_to_range("test", 4, 0, true), (0, 4));
}

#[test]
fn motion_range_same() {
    assert_eq!(motion_to_range("test", 3, 3, true), (3, 3));
}

// ---------------------------------------------------------------------------
// delete_range
// ---------------------------------------------------------------------------

#[test]
fn delete_range_middle() {
    let (new_buf, deleted) = delete_range("hello world", 5, 6);
    assert_eq!(new_buf.as_str(), "helloworld");
    assert_eq!(deleted.as_str(), " ");
}

#[test]
fn delete_range_start() {
    let (new_buf, deleted) = delete_range("hello", 0, 2);
    assert_eq!(new_buf.as_str(), "llo");
    assert_eq!(deleted.as_str(), "he");
}

#[test]
fn delete_range_end() {
    let (new_buf, deleted) = delete_range("hello", 3, 5);
    assert_eq!(new_buf.as_str(), "hel");
    assert_eq!(deleted.as_str(), "lo");
}

#[test]
fn delete_range_entire() {
    let (new_buf, deleted) = delete_range("hello", 0, 5);
    assert_eq!(new_buf.as_str(), "");
    assert_eq!(deleted.as_str(), "hello");
}

#[test]
fn delete_range_empty_range() {
    let (new_buf, deleted) = delete_range("hello", 2, 2);
    assert_eq!(new_buf.as_str(), "hello");
    assert_eq!(deleted.as_str(), "");
}

// ---------------------------------------------------------------------------
// yank_range
// ---------------------------------------------------------------------------

#[test]
fn yank_range_basic() {
    assert_eq!(yank_range("hello", 1, 4).as_str(), "ell");
}

#[test]
fn yank_range_entire() {
    assert_eq!(yank_range("hello", 0, 5).as_str(), "hello");
}

#[test]
fn yank_range_empty() {
    assert_eq!(yank_range("hello", 2, 2).as_str(), "");
}

// ---------------------------------------------------------------------------
// apply_operator
// ---------------------------------------------------------------------------

#[test]
fn op_d_motion_w() {
    let mut vi = make_vi();
    let (buf, cur, deleted) =
        apply_operator("hello world", 0, ViOperator::Delete, "w", 1, &mut vi).unwrap();
    // 'w' deletes to start of next word, including whitespace
    assert_eq!(buf.as_str(), "world");
    assert_eq!(cur, 0);
    assert_eq!(deleted.as_str(), "hello ");
}

#[test]
fn op_d_motion_l() {
    let mut vi = make_vi();
    let (buf, cur, deleted) =
        apply_operator("abc", 0, ViOperator::Delete, "l", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "bc");
    assert_eq!(cur, 0);
    assert_eq!(deleted.as_str(), "a");
}

#[test]
fn op_dd() {
    let mut vi = make_vi();
    let (buf, cur, deleted) =
        apply_operator("hello\nworld\nfoo", 0, ViOperator::Delete, "dd", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "world\nfoo");
    assert_eq!(cur, 0);
    assert_eq!(deleted.as_str(), "hello\n");
}

#[test]
fn op_dd_last_line() {
    let mut vi = make_vi();
    let (buf, _, deleted) =
        apply_operator("hello\nworld", 6, ViOperator::Delete, "dd", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "hello\n");
    assert_eq!(deleted.as_str(), "world");
}

#[test]
fn op_yy() {
    let mut vi = make_vi();
    let (buf, cur, yanked) =
        apply_operator("hello\nworld", 0, ViOperator::Yank, "yy", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "hello\nworld"); // unchanged
    assert_eq!(cur, 0);
    assert_eq!(yanked.as_str(), "hello\n");
}

#[test]
fn op_cc() {
    let mut vi = make_vi();
    let (buf, cur, saved) =
        apply_operator("hello\nworld", 0, ViOperator::Change, "cc", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "\nworld");
    assert_eq!(cur, 0);
    assert_eq!(saved.as_str(), "hello\n");
}

#[test]
fn op_c_motion_w() {
    let mut vi = make_vi();
    let (buf, cur, saved) =
        apply_operator("hello world", 0, ViOperator::Change, "w", 1, &mut vi).unwrap();
    // 'w' deletes to start of next word, including whitespace
    assert_eq!(buf.as_str(), "world");
    assert_eq!(cur, 0);
    assert_eq!(saved.as_str(), "hello ");
}

#[test]
fn op_y_motion_w() {
    let mut vi = make_vi();
    let (buf, cur, yanked) =
        apply_operator("hello world", 0, ViOperator::Yank, "w", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "hello world"); // unchanged
    assert_eq!(cur, 0);
    assert_eq!(yanked.as_str(), "hello ");
}

#[test]
fn op_cc_three_lines() {
    let mut vi = make_vi();
    let (buf, cur, saved) =
        apply_operator("hello\nworld\nfoo", 0, ViOperator::Change, "cc", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "\nworld\nfoo");
    assert_eq!(cur, 0);
    assert_eq!(saved.as_str(), "hello\n");
}

#[test]
fn op_C() {
    let mut vi = make_vi();
    let (buf, cur, saved) =
        apply_operator("hello world", 3, ViOperator::Change, "C", 1, &mut vi).unwrap();
    assert_eq!(buf.as_str(), "hel");
    assert_eq!(cur, 3);
    assert_eq!(saved.as_str(), "lo world");
}

#[test]
fn op_unknown_motion_returns_none() {
    let mut vi = make_vi();
    let result = apply_operator("hi", 0, ViOperator::Delete, "nonexistent", 1, &mut vi);
    assert!(result.is_none());
}

#[test]
fn op_indent_right_returns_none() {
    let mut vi = make_vi();
    let result = apply_operator("hi", 0, ViOperator::IndentRight, "dd", 1, &mut vi);
    assert!(result.is_none());
}

#[test]
fn op_indent_left_returns_none() {
    let mut vi = make_vi();
    let result = apply_operator("hi", 0, ViOperator::IndentLeft, "dd", 1, &mut vi);
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Edge cases: multi-byte UTF-8
// ---------------------------------------------------------------------------

#[test]
fn motion_l_multibyte() {
    let vi = make_vi();
    // 'l' advances by one character: from 'h' (byte 0) to 'é' (byte 1)
    assert_eq!(apply_motion("héllo", 0, 1, "l", &vi), Some(1));
}

#[test]
fn delete_range_multibyte() {
    // "héllo" in bytes: h(0) é(1-2) l(3) l(4) o(5)
    let (new_buf, deleted) = delete_range("héllo", 0, 3);
    assert_eq!(new_buf.as_str(), "llo");
    assert_eq!(deleted.as_str(), "hé");
}

#[test]
fn match_bracket_multibyte_surrounding() {
    let buf = "héllo(wörld)bye";
    // '(' is at char index 5
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 5),
        Some(11)
    );
    // ')' is at char index 11
    assert_eq!(
        crate::ui::input::vi::ViState::match_bracket(buf, 11),
        Some(5)
    );
}

#[test]
fn motion_w_twice() {
    let vi = make_vi();
    let buf = "hello world foo";
    let c1 = apply_motion(buf, 0, 1, "w", &vi).unwrap();
    assert_eq!(c1, 6, "first w should go to start of 'world'");
    let c2 = apply_motion(buf, c1, 1, "w", &vi).unwrap();
    assert_eq!(c2, 12, "second w should go to start of 'foo'");
}

#[test]
fn motion_b_from_first_char_of_previous_word() {
    let vi = make_vi();
    let buf = "hello world";
    // From first char of 'world', b should go to start of 'hello'
    let c = apply_motion(buf, 6, 1, "b", &vi).unwrap();
    assert_eq!(c, 0, "b from start of 'world' should go to start of 'hello'");
}

#[test]
fn motion_b_twice() {
    let vi = make_vi();
    let buf = "hello world foo";
    // From last char of buffer, b twice
    let c1 = apply_motion(buf, 14, 1, "b", &vi).unwrap();
    assert_eq!(c1, 12, "first b from end should go to start of 'foo'");
    let c2 = apply_motion(buf, c1, 1, "b", &vi).unwrap();
    assert_eq!(c2, 6, "second b should go to start of 'world'");
}

#[test]
fn motion_e_twice() {
    let vi = make_vi();
    let buf = "hello world foo";
    let c1 = apply_motion(buf, 0, 1, "e", &vi).unwrap();
    assert_eq!(c1, 4, "first e from start should go to end of 'hello'");
    let c2 = apply_motion(buf, c1, 1, "e", &vi).unwrap();
    assert_eq!(c2, 10, "second e should go to end of 'world'");
}

#[test]
fn motion_W_twice() {
    let vi = make_vi();
    let buf = "hello world foo";
    let c1 = apply_motion(buf, 0, 1, "W", &vi).unwrap();
    assert_eq!(c1, 6, "first W from start should go to start of 'world'");
    let c2 = apply_motion(buf, c1, 1, "W", &vi).unwrap();
    assert_eq!(c2, 12, "second W should go to start of 'foo'");
}

#[test]
fn motion_B_from_first_char_of_previous_word() {
    let vi = make_vi();
    let buf = "hello world";
    let c = apply_motion(buf, 6, 1, "B", &vi).unwrap();
    assert_eq!(c, 0, "B from start of 'world' should go to start of 'hello'");
}

#[test]
fn motion_B_twice() {
    let vi = make_vi();
    let buf = "hello world foo";
    let c1 = apply_motion(buf, 14, 1, "B", &vi).unwrap();
    assert_eq!(c1, 12, "first B from end should go to start of 'foo'");
    let c2 = apply_motion(buf, c1, 1, "B", &vi).unwrap();
    assert_eq!(c2, 6, "second B should go to start of 'world'");
}

#[test]
fn motion_E_twice() {
    let vi = make_vi();
    let buf = "hello world foo";
    let c1 = apply_motion(buf, 0, 1, "E", &vi).unwrap();
    assert_eq!(c1, 4, "first E from start should go to end of 'hello'");
    let c2 = apply_motion(buf, c1, 1, "E", &vi).unwrap();
    assert_eq!(c2, 10, "second E should go to end of 'world'");
}
