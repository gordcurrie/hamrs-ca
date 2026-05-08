use crate::modes::drill::{DrillSession, Flashcard};
use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Screen {
    Prompting,
    Revealed,
    Summary,
}

struct App {
    label: &'static str,
    cards: Vec<Flashcard>,
    current: usize,
    screen: Screen,
    correct: u32,
    missed_indices: Vec<usize>,
    should_quit: bool,
}

impl App {
    fn new(session: DrillSession) -> Self {
        Self {
            label: session.label,
            cards: session.cards,
            current: 0,
            screen: Screen::Prompting,
            correct: 0,
            missed_indices: Vec::new(),
            should_quit: false,
        }
    }

    fn total(&self) -> usize {
        self.cards.len()
    }

    fn current_card(&self) -> &Flashcard {
        &self.cards[self.current]
    }

    fn handle_key(&mut self, code: KeyCode) {
        match self.screen {
            Screen::Prompting => match code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.screen = Screen::Revealed;
                }
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    self.screen = Screen::Summary;
                }
                _ => {}
            },
            Screen::Revealed => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.correct += 1;
                    self.advance();
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.missed_indices.push(self.current);
                    self.advance();
                }
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    self.screen = Screen::Summary;
                }
                _ => {}
            },
            Screen::Summary => match code {
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Enter | KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            },
        }
    }

    fn advance(&mut self) {
        if self.current + 1 >= self.total() {
            self.screen = Screen::Summary;
        } else {
            self.current += 1;
            self.screen = Screen::Prompting;
        }
    }
}

pub fn run(session: DrillSession) -> Result<()> {
    let mut app = App::new(session);

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let _guard = scopeguard::guard((), |_| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
    });

    loop {
        terminal.draw(|frame| render(frame, &app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
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
        .title(format!(" hamrs-ca — Drill: {} ", app.label))
        .title_style(Style::default().add_modifier(Modifier::BOLD));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    match app.screen {
        Screen::Prompting | Screen::Revealed => render_card(frame, inner, app),
        Screen::Summary => render_summary(frame, inner, app),
    }
}

fn render_card(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let card = app.current_card();
    let revealed = app.screen == Screen::Revealed;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),      // header: progress + running score
            Constraint::Length(1),      // spacer
            Constraint::Percentage(40), // prompt
            Constraint::Length(1),      // separator (blank when not revealed)
            Constraint::Percentage(40), // answer (blank when not revealed)
            Constraint::Min(0),         // flexible buffer
            Constraint::Length(1),      // footer / key hints
        ])
        .split(area);

    let header = Paragraph::new(format!(
        "Card {} / {}    \u{2713} {}   \u{2717} {}",
        app.current + 1,
        app.total(),
        app.correct,
        app.missed_indices.len(),
    ))
    .style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(header, chunks[0]);

    let prompt = Paragraph::new(card.prompt.as_str())
        .wrap(Wrap { trim: false })
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(prompt, chunks[2]);

    if revealed {
        let sep = Paragraph::new("─".repeat(area.width.saturating_sub(2) as usize))
            .style(Style::default().add_modifier(Modifier::DIM));
        frame.render_widget(sep, chunks[3]);

        let answer = Paragraph::new(card.answer.as_str())
            .wrap(Wrap { trim: false })
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(answer, chunks[4]);
    }

    let footer_text = if revealed {
        "  [y] Got it   [n] Missed   [q] Quit"
    } else {
        "  [Space / Enter] Reveal answer   [q] Quit"
    };
    let footer = Paragraph::new(footer_text).style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(footer, chunks[6]);
}

fn render_summary(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let attempted = app.correct as usize + app.missed_indices.len();
    let pct = (app.correct as usize * 100)
        .checked_div(attempted)
        .unwrap_or(0);

    let (pct_color, pct_badge) = if pct >= 80 {
        (Color::Green, "\u{2605}")
    } else if pct >= 60 {
        (Color::Yellow, "~")
    } else {
        (Color::Red, "\u{2717}")
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(1), // spacer
            Constraint::Length(1), // correct line
            Constraint::Length(1), // missed count
            Constraint::Length(1), // spacer
            Constraint::Min(4),    // missed list
            Constraint::Length(1), // spacer
            Constraint::Length(1), // footer
        ])
        .split(area);

    let title =
        Paragraph::new("Session Complete").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let correct_line = Paragraph::new(Line::from(vec![
        Span::raw(format!("Correct:  {} / {}   ", app.correct, attempted)),
        Span::styled(
            format!("{}%  {}", pct, pct_badge),
            Style::default().fg(pct_color).add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(correct_line, chunks[2]);

    let missed_count = Paragraph::new(format!("Missed:   {}", app.missed_indices.len())).style(
        Style::default().fg(if app.missed_indices.is_empty() {
            Color::Green
        } else {
            Color::Red
        }),
    );
    frame.render_widget(missed_count, chunks[3]);

    if !app.missed_indices.is_empty() {
        let items: Vec<ListItem> = app
            .missed_indices
            .iter()
            .map(|&i| {
                let card = &app.cards[i];
                ListItem::new(vec![
                    Line::from(Span::styled(
                        format!("  Q: {}", card.prompt),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(Span::styled(
                        format!("  A: {}", card.answer),
                        Style::default().fg(Color::Red),
                    )),
                ])
            })
            .collect();
        let missed_list = List::new(items);
        frame.render_widget(missed_list, chunks[5]);
    }

    let footer =
        Paragraph::new("  [q / Enter]  Quit").style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(footer, chunks[7]);
}
