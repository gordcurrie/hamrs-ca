use crate::db::Db;
use crate::modes::exam::{QuizSession, ShuffledQuestion};
use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Screen {
    Quiz,
    Score,
}

struct App {
    questions: Vec<ShuffledQuestion>,
    current: usize,
    cursor: usize,
    answered: Option<usize>, // index of chosen answer, or None if not yet answered
    score: u32,
    mode_label: &'static str,
    time_limit: Option<Duration>,
    started: Instant,
    screen: Screen,
    should_quit: bool,
}

impl App {
    fn new(session: QuizSession) -> Self {
        Self {
            questions: session.questions,
            current: 0,
            cursor: 0,
            answered: None,
            score: 0,
            mode_label: session.mode_label,
            time_limit: session.time_limit_secs.map(Duration::from_secs),
            started: Instant::now(),
            screen: Screen::Quiz,
            should_quit: false,
        }
    }

    fn current_q(&self) -> &ShuffledQuestion {
        &self.questions[self.current]
    }

    fn total(&self) -> usize {
        self.questions.len()
    }

    fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    fn remaining(&self) -> Option<Duration> {
        self.time_limit
            .map(|limit| limit.saturating_sub(self.elapsed()))
    }

    fn is_time_up(&self) -> bool {
        self.time_limit
            .map(|l| self.elapsed() >= l)
            .unwrap_or(false)
    }

    fn handle_key(&mut self, code: KeyCode, db: &Db, session_id: i64) -> Result<()> {
        match self.screen {
            Screen::Quiz => self.handle_quiz_key(code, db, session_id)?,
            Screen::Score => {
                if matches!(
                    code,
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Enter | KeyCode::Esc
                ) {
                    self.should_quit = true;
                }
            }
        }
        Ok(())
    }

    fn handle_quiz_key(&mut self, code: KeyCode, db: &Db, session_id: i64) -> Result<()> {
        if self.answered.is_none() {
            // Pre-answer: navigate and confirm
            match code {
                KeyCode::Up | KeyCode::Char('k') if self.cursor > 0 => self.cursor -= 1,
                KeyCode::Down | KeyCode::Char('j') if self.cursor < 3 => self.cursor += 1,
                KeyCode::Char('1') => self.cursor = 0,
                KeyCode::Char('2') => self.cursor = 1,
                KeyCode::Char('3') => self.cursor = 2,
                KeyCode::Char('4') => self.cursor = 3,
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let correct = self.cursor == self.current_q().correct_index;
                    if correct {
                        self.score += 1;
                    }
                    self.answered = Some(self.cursor);
                    db.record_attempt(session_id, &self.current_q().question.id, correct)?;
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.should_quit = true;
                }
                _ => {}
            }
        } else {
            // Post-answer: advance
            match code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right => {
                    self.advance();
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.should_quit = true;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn advance(&mut self) {
        if self.current + 1 >= self.total() {
            self.screen = Screen::Score;
        } else {
            self.current += 1;
            self.cursor = 0;
            self.answered = None;
        }
    }

    fn finish_from_timeout(&mut self) {
        self.screen = Screen::Score;
    }
}

pub fn run(session: QuizSession, db: &Db) -> Result<()> {
    let session_id = db.start_session(session.mode_label)?;
    let mut app = App::new(session);

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Restore terminal on both normal return and panic
    let _guard = scopeguard::guard((), |_| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
    });

    let result = run_loop(&mut terminal, &mut app, db, session_id);

    result?;

    if app.screen == Screen::Score {
        let total = app.total() as u32;
        db.finish_session(session_id, app.score, total)?;
    }

    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    db: &Db,
    session_id: i64,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;

        if app.is_time_up() {
            app.finish_from_timeout();
        }

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, db, session_id)?;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn render(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" hamrs-ca — {} ", app.mode_label))
        .title_style(Style::default().add_modifier(Modifier::BOLD));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    match app.screen {
        Screen::Quiz => render_quiz(frame, inner, app),
        Screen::Score => render_score(frame, inner, app),
    }
}

fn render_quiz(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let q = app.current_q();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Min(4),    // question text
            Constraint::Length(1), // separator line
            Constraint::Min(8),    // answers
            Constraint::Length(2), // footer / feedback
        ])
        .split(area);

    // Header: progress + timer + section
    let section = q.question.section_name();
    let progress_text = format!("Q {}/{}  │  {}", app.current + 1, app.total(), section);
    let (timer_text, timer_color) = match app.remaining() {
        Some(rem) => {
            let mins = rem.as_secs() / 60;
            let secs = rem.as_secs() % 60;
            let color = if rem.as_secs() < 300 {
                Color::Red
            } else {
                Color::Green
            };
            (format!("⏱ {:02}:{:02}", mins, secs), color)
        }
        None => (String::new(), Color::Reset),
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(progress_text, Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::styled(timer_text, Style::default().fg(timer_color)),
    ]));
    frame.render_widget(header, chunks[0]);

    // Question text
    let question = Paragraph::new(q.question.text.as_str())
        .wrap(Wrap { trim: false })
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(question, chunks[1]);

    // Separator
    let sep = Paragraph::new("─".repeat(area.width as usize));
    frame.render_widget(sep, chunks[2]);

    // Answers — word-wrap each answer to avoid truncation
    let prefix_width = 6usize; // "  A.  "
    let answer_width = (chunks[3].width as usize).saturating_sub(prefix_width);
    let items: Vec<ListItem> = q
        .answers
        .iter()
        .enumerate()
        .map(|(i, ans)| {
            let label = ['A', 'B', 'C', 'D'][i];
            let style = answer_style(app, i);
            let wrapped = word_wrap(ans, answer_width);
            let mut lines: Vec<Line> = Vec::new();
            for (j, line) in wrapped.iter().enumerate() {
                if j == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {label}.  "), style),
                        Span::styled(line.clone(), style),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw(" ".repeat(prefix_width)),
                        Span::styled(line.clone(), style),
                    ]));
                }
            }
            ListItem::new(Text::from(lines))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    // We manage selection manually via cursor, so build a ListState
    let mut state = ratatui::widgets::ListState::default();
    if app.answered.is_none() {
        state.select(Some(app.cursor));
    }
    frame.render_stateful_widget(list, chunks[3], &mut state);

    // Footer: hint or feedback
    let footer = if let Some(chosen) = app.answered {
        let correct = chosen == q.correct_index;
        if correct {
            Paragraph::new("  ✓  Correct!  —  Space or Enter for next").style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            let correct_label = ['A', 'B', 'C', 'D'][q.correct_index];
            Paragraph::new(format!(
                "  ✗  Incorrect. Correct answer: {}  —  Space or Enter for next",
                correct_label
            ))
            .style(Style::default().fg(Color::Red))
        }
    } else {
        Paragraph::new("  ↑↓  Navigate   1-4  Jump to answer   Enter  Confirm   q  Quit")
            .style(Style::default().add_modifier(Modifier::DIM))
    };
    frame.render_widget(footer, chunks[4]);
}

fn answer_style(app: &App, index: usize) -> Style {
    let q = app.current_q();
    match app.answered {
        None => {
            if index == app.cursor {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }
        }
        Some(chosen) => {
            if index == q.correct_index {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else if index == chosen && chosen != q.correct_index {
                Style::default().fg(Color::Red)
            } else {
                Style::default().add_modifier(Modifier::DIM)
            }
        }
    }
}

fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn render_score(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let total = app.total() as u32;
    let pct = (app.score * 100) / total.max(1);

    let (result_label, result_color) = if pct >= 80 {
        ("★ Honours (80%+) — HF privileges unlocked", Color::Green)
    } else if pct >= 70 {
        ("✓ Pass (70%+)", Color::Yellow)
    } else {
        ("✗ Below passing threshold (need 70%)", Color::Red)
    };

    let elapsed = app.elapsed();
    let elapsed_str = format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(area);

    let score_line = Paragraph::new(format!(
        "  Score: {}/{} ({}%)   Time: {}",
        app.score, total, pct, elapsed_str
    ))
    .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(score_line, chunks[1]);

    let result_line =
        Paragraph::new(format!("  {result_label}")).style(Style::default().fg(result_color));
    frame.render_widget(result_line, chunks[2]);

    let bar_pct = pct.min(100) as u16;
    let gauge_color = result_color;
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(gauge_color))
        .percent(bar_pct)
        .label(format!("{pct}%"));
    frame.render_widget(
        gauge,
        chunks[3].inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    let footer =
        Paragraph::new("  q / Enter  Quit").style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(footer, chunks[4]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_wrap_fits_on_one_line() {
        assert_eq!(word_wrap("hello world", 20), vec!["hello world"]);
    }

    #[test]
    fn word_wrap_breaks_at_boundary() {
        let result = word_wrap("The field strength of your emissions", 20);
        assert!(result.iter().all(|l| l.len() <= 20));
        assert!(result.len() > 1);
    }

    #[test]
    fn word_wrap_zero_width_returns_original() {
        assert_eq!(word_wrap("hello", 0), vec!["hello"]);
    }

    #[test]
    fn word_wrap_long_ised_answer() {
        let text = "The field strength of your emissions, on your neighbour's premises, is below Innovation, Science and Economic Development Canada's specified immunity criteria";
        let result = word_wrap(text, 74);
        assert!(result.iter().all(|l| l.len() <= 74));
        assert_eq!(result.join(" "), text);
    }
}
