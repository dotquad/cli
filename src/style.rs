//! Terminal styles. anstream drops them when output isn't a terminal or `NO_COLOR` is set.
//! Grade tones follow Dotquad's Signal design: teal for A, grey for B and C, amber for D and F.

use anstyle::{AnsiColor, Style};

const BOLD: Style = Style::new().bold();
const DIM: Style = Style::new().dimmed();
const OK: Style = AnsiColor::Cyan.on_default();
const FLAG: Style = AnsiColor::Yellow.on_default();

fn paint(style: Style, s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    format!("{}{s}{}", style.render(), style.render_reset())
}

pub fn bold(s: &str) -> String {
    paint(BOLD, s)
}

pub fn dim(s: &str) -> String {
    paint(DIM, s)
}

pub fn ok(s: &str) -> String {
    paint(OK, s)
}

pub fn flag(s: &str) -> String {
    paint(FLAG, s)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Ok,
    Neutral,
    Flag,
}

pub fn grade_tone(grade: &str) -> Tone {
    match grade {
        "A+" | "A" => Tone::Ok,
        "B" | "C" => Tone::Neutral,
        _ => Tone::Flag,
    }
}

pub fn tone(t: Tone, s: &str) -> String {
    match t {
        Tone::Ok => ok(s),
        Tone::Neutral => s.to_string(),
        Tone::Flag => flag(s),
    }
}

/// A grade padded to `width` before it's colored, so columns still line up.
pub fn grade(g: &str, width: usize) -> String {
    let padded = format!("{g:<width$}");
    let t = grade_tone(g);
    let trimmed = padded.trim_end();
    let pad = &padded[trimmed.len()..];
    format!("{}{pad}", paint(BOLD, &tone(t, trimmed)))
}
