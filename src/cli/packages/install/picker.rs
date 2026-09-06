use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    widgets::{Block, List, ListState, Paragraph},
};

use crate::core::completion::CompletionEngine;

const VISIBLE_MATCH_LIMIT: usize = 200;
const MAX_QUERY_BYTES: usize = 255;

struct Picker {
    candidates: Vec<String>,
    query: String,
    matches: Vec<usize>,
    selected: usize,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Continue,
    Cancelled,
    Selected(String),
}

impl Picker {
    fn new(candidates: Vec<String>) -> Self {
        let matches = CompletionEngine::fuzzy_indices("", &candidates, VISIBLE_MATCH_LIMIT);
        Self {
            candidates,
            query: String::new(),
            matches,
            selected: 0,
        }
    }

    fn update_matches(&mut self) {
        self.matches =
            CompletionEngine::fuzzy_indices(&self.query, &self.candidates, VISIBLE_MATCH_LIMIT);
        self.selected = 0;
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::Continue;
        }
        match key.code {
            KeyCode::Esc => return Action::Cancelled,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::Cancelled;
            }
            KeyCode::Enter => {
                if let Some(package) = self
                    .matches
                    .get(self.selected)
                    .and_then(|index| self.candidates.get(*index))
                {
                    return Action::Selected(package.clone());
                }
            }
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.matches.len().saturating_sub(1));
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.update_matches();
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && !character.is_control()
                    && self.query.len() + character.len_utf8() <= MAX_QUERY_BYTES =>
            {
                self.query.push(character);
                self.update_matches();
            }
            _ => {}
        }
        Action::Continue
    }
}

pub(super) fn choose(candidates: Vec<String>) -> Result<Option<String>> {
    anyhow::ensure!(
        !candidates.is_empty(),
        "No package names are available to search"
    );
    crate::core::security::validate_package_names(&candidates)?;
    let mut picker = Picker::new(candidates);
    let result: Result<Option<String>> = (|| {
        let mut terminal = ratatui::try_init()?;
        let mut list_state = ListState::default();
        loop {
            list_state.select(picker.matches.get(picker.selected).map(|_| picker.selected));
            terminal.draw(|frame| {
                let [search, results, help] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(2),
                ])
                .areas(frame.area());
                frame.render_widget(
                    Paragraph::new(picker.query.as_str())
                        .block(Block::bordered().title("Install a package — type to search")),
                    search,
                );
                let title = if picker.matches.is_empty() {
                    "No matching packages".to_owned()
                } else {
                    format!("{} shown · refine your search", picker.matches.len())
                };
                let items = picker
                    .matches
                    .iter()
                    .filter_map(|index| picker.candidates.get(*index))
                    .map(String::as_str);
                frame.render_stateful_widget(
                    List::new(items)
                        .block(Block::bordered().title(title))
                        .highlight_style(Style::new().reversed()),
                    results,
                    &mut list_state,
                );
                frame.render_widget(
                    Paragraph::new("↑/↓ choose · Enter select · Esc/Ctrl-C cancel"),
                    help,
                );
            })?;
            if let Event::Key(key) = event::read()? {
                match picker.handle_key(key) {
                    Action::Continue => {}
                    Action::Cancelled => return Ok(None),
                    Action::Selected(package) => return Ok(Some(package)),
                }
            }
        }
    })();
    match (result, ratatui::try_restore()) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => {
            Err(error).context("Failed to restore the terminal after package selection")
        }
        (Err(error), Err(restore_error)) => {
            Err(error.context(format!("Terminal restoration also failed: {restore_error}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn fuzzy_query_selects_the_matching_package() {
        let mut picker = Picker::new(vec!["git".into(), "firefox".into()]);
        for character in "frfx".chars() {
            assert_eq!(
                picker.handle_key(key(KeyCode::Char(character))),
                Action::Continue
            );
        }
        assert_eq!(
            picker.handle_key(key(KeyCode::Enter)),
            Action::Selected("firefox".into())
        );
    }

    #[test]
    fn no_matches_cannot_select_a_package() {
        let mut picker = Picker::new(vec!["git".into()]);
        picker.handle_key(key(KeyCode::Char('z')));
        picker.handle_key(key(KeyCode::Down));
        assert_eq!(picker.handle_key(key(KeyCode::Enter)), Action::Continue);
        picker.handle_key(key(KeyCode::Backspace));
        assert_eq!(
            picker.handle_key(key(KeyCode::Enter)),
            Action::Selected("git".into())
        );
    }

    #[test]
    fn cancellation_never_selects_the_highlighted_package() {
        let mut picker = Picker::new(vec!["git".into()]);
        assert_eq!(picker.handle_key(key(KeyCode::Esc)), Action::Cancelled);
        assert_eq!(
            picker.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Cancelled
        );
    }

    #[test]
    fn query_input_is_bounded_without_splitting_unicode() {
        let mut picker = Picker::new(vec!["git".into()]);
        for _ in 0..MAX_QUERY_BYTES {
            picker.handle_key(key(KeyCode::Char('é')));
        }
        assert_eq!(picker.query.len(), MAX_QUERY_BYTES - 1);
    }

    #[test]
    fn narrowing_the_query_resets_selection_and_bounds_results() {
        let mut picker = Picker::new((0..500).map(|index| format!("package-{index}")).collect());
        assert_eq!(picker.matches.len(), VISIBLE_MATCH_LIMIT);
        for _ in 0..600 {
            picker.handle_key(key(KeyCode::Down));
        }
        assert_eq!(picker.selected, VISIBLE_MATCH_LIMIT - 1);
        picker.handle_key(key(KeyCode::Char('9')));
        assert_eq!(picker.selected, 0);
    }
}
