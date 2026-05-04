use crate::modes::morse::{MorseItem, MorseMode, MorseSession};
use crate::morse::{self, dit_ms};
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
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Terminal,
};
use rodio::source::SineWave;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Screen {
    Practice,
    Score,
}

/// Which direction this particular item is being practiced.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ItemMode {
    Receive,
    Transmit,
}

/// State of the animated receive playback.
#[derive(Debug, Clone, Copy)]
enum PlaybackState {
    /// Showing element `idx` of the code string; started at `since`.
    Playing { element_idx: usize, since: Instant },
    /// All elements shown; waiting for user input.
    Waiting,
}

struct App {
    items: Vec<MorseItem>,
    current: usize,
    score: u32,
    /// Current text typed by the user (held in a buffer).
    input: String,
    /// Some(correct) after the user submits; None while waiting.
    result: Option<bool>,
    /// Time the current item started (for transmit WPM).
    item_started: Instant,
    /// Accumulated response times in ms (transmit only).
    response_times_ms: Vec<u64>,
    playback: PlaybackState,
    item_mode: ItemMode,
    session_mode: MorseMode,
    wpm: u32,
    screen: Screen,
    should_quit: bool,
    // Audio — _audio_stream must stay alive for the duration of playback.
    _audio_stream: Option<OutputStream>,
    audio_handle: Option<OutputStreamHandle>,
    audio_sink: Option<Sink>,
}

impl App {
    fn new(session: MorseSession) -> Self {
        let item_mode = effective_mode(session.config.mode, 0);
        let playback = initial_playback(item_mode);
        let (_audio_stream, audio_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(_) => (None, None),
        };
        let mut app = Self {
            items: session.items,
            current: 0,
            score: 0,
            input: String::new(),
            result: None,
            item_started: Instant::now(),
            response_times_ms: Vec::new(),
            playback,
            item_mode,
            session_mode: session.config.mode,
            wpm: session.config.wpm,
            screen: Screen::Practice,
            should_quit: false,
            _audio_stream,
            audio_handle,
            audio_sink: None,
        };
        if app.item_mode == ItemMode::Receive {
            app.play_audio();
        }
        app
    }

    fn play_audio(&mut self) {
        self.audio_sink = None; // drop previous sink, stopping any active playback
        let Some(handle) = &self.audio_handle else {
            return;
        };
        let Ok(sink) = Sink::try_new(handle) else {
            return;
        };
        let code = self.current_item().code;
        let dit = dit_ms(self.wpm);
        let bytes = code.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            let tone_ms = if b == b'-' { dit * 3 } else { dit };
            let gap_ms = if i + 1 < bytes.len() { dit } else { 0 };
            sink.append(
                SineWave::new(700.0)
                    .take_duration(Duration::from_millis(tone_ms))
                    .amplify(0.20),
            );
            if gap_ms > 0 {
                sink.append(
                    SineWave::new(700.0)
                        .take_duration(Duration::from_millis(gap_ms))
                        .amplify(0.0),
                );
            }
        }
        self.audio_sink = Some(sink);
    }

    fn current_item(&self) -> &MorseItem {
        &self.items[self.current]
    }

    fn total(&self) -> usize {
        self.items.len()
    }

    fn advance(&mut self) {
        if self.current + 1 >= self.total() {
            self.audio_sink = None;
            self.screen = Screen::Score;
        } else {
            self.current += 1;
            self.input.clear();
            self.result = None;
            self.item_mode = effective_mode(self.session_mode, self.current);
            self.playback = initial_playback(self.item_mode);
            self.item_started = Instant::now();
            if self.item_mode == ItemMode::Receive {
                self.play_audio();
            }
        }
    }

    fn submit(&mut self) {
        let raw = morse::normalise(&self.input);
        let item = self.current_item();

        let correct = match self.item_mode {
            ItemMode::Receive => raw
                .to_uppercase()
                .chars()
                .next()
                .map(|c| c == item.character)
                .unwrap_or(false),
            // raw is already normalised; item.code is the canonical table string
            ItemMode::Transmit => raw == item.code,
        };

        if correct {
            self.score += 1;
        }

        if self.item_mode == ItemMode::Transmit {
            let ms = self.item_started.elapsed().as_millis() as u64;
            self.response_times_ms.push(ms);
        }

        self.result = Some(correct);
    }

    /// Advance the receive animation by one tick.
    fn tick_playback(&mut self) {
        if self.item_mode != ItemMode::Receive || self.result.is_some() {
            return;
        }
        let code = self.current_item().code;
        let bytes = code.as_bytes();

        // Loop so multiple elements can advance in one tick at high WPM
        while let PlaybackState::Playing { element_idx, since } = self.playback {
            let element_ms = if bytes[element_idx] == b'-' {
                dit_ms(self.wpm) * 3
            } else {
                dit_ms(self.wpm)
            };
            let next = element_idx + 1;
            // Include a 1-dit inter-element gap so timing matches stated WPM
            let total_ms = element_ms
                + if next < bytes.len() {
                    dit_ms(self.wpm)
                } else {
                    0
                };
            if since.elapsed().as_millis() as u64 >= total_ms {
                if next >= bytes.len() {
                    self.playback = PlaybackState::Waiting;
                    self.item_started = Instant::now();
                    break;
                } else {
                    self.playback = PlaybackState::Playing {
                        element_idx: next,
                        since: Instant::now(),
                    };
                }
            } else {
                break;
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode) {
        match self.screen {
            Screen::Score => {
                if matches!(
                    code,
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Enter | KeyCode::Esc
                ) {
                    self.should_quit = true;
                }
            }
            Screen::Practice => self.handle_practice_key(code),
        }
    }

    fn handle_practice_key(&mut self, code: KeyCode) {
        // After submitting — Enter/Space advances, q quits
        if self.result.is_some() {
            match code {
                KeyCode::Enter | KeyCode::Char(' ') => self.advance(),
                KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
                _ => {}
            }
            return;
        }

        // Receive: block input and handle replay/quit while animation plays
        if self.item_mode == ItemMode::Receive
            && matches!(self.playback, PlaybackState::Playing { .. })
        {
            match code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.playback = PlaybackState::Playing {
                        element_idx: 0,
                        since: Instant::now(),
                    };
                    self.play_audio();
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
                _ => {}
            }
            return;
        }

        // Receive: allow replay even after animation finishes (Waiting state)
        if self.item_mode == ItemMode::Receive {
            if let KeyCode::Char('r') | KeyCode::Char('R') = code {
                self.playback = PlaybackState::Playing {
                    element_idx: 0,
                    since: Instant::now(),
                };
                self.play_audio();
                return;
            }
        }

        match code {
            KeyCode::Enter if !self.input.is_empty() => {
                self.submit();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char('q') | KeyCode::Char('Q') if self.input.is_empty() => {
                self.should_quit = true;
            }
            KeyCode::Char(c) => {
                // Transmit: accept dots, dashes, spaces
                // Receive: accept letters and digits
                match self.item_mode {
                    ItemMode::Transmit => {
                        if matches!(c, '.' | '-' | ' ') {
                            self.input.push(c);
                        }
                    }
                    ItemMode::Receive => {
                        if c.is_ascii_alphanumeric() {
                            self.input.push(c.to_ascii_uppercase());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// WPM computed from transmit response times (chars/min ÷ 5, PARIS standard).
    fn effective_transmit_wpm(&self) -> Option<u32> {
        if self.response_times_ms.is_empty() {
            return None;
        }
        let avg_ms =
            self.response_times_ms.iter().sum::<u64>() / self.response_times_ms.len() as u64;
        // avg_ms per character; 1 word = 5 chars; WPM = 60000 / (avg_ms * 5)
        let wpm = 60_000u64 / (avg_ms.max(1) * 5);
        Some(wpm.min(999) as u32)
    }
}

fn effective_mode(session_mode: MorseMode, index: usize) -> ItemMode {
    match session_mode {
        MorseMode::Receive => ItemMode::Receive,
        MorseMode::Transmit => ItemMode::Transmit,
        MorseMode::Both => {
            if index.is_multiple_of(2) {
                ItemMode::Receive
            } else {
                ItemMode::Transmit
            }
        }
    }
}

fn initial_playback(mode: ItemMode) -> PlaybackState {
    match mode {
        ItemMode::Receive => PlaybackState::Playing {
            element_idx: 0,
            since: Instant::now(),
        },
        ItemMode::Transmit => PlaybackState::Waiting,
    }
}

pub fn run(session: MorseSession) -> Result<()> {
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
        app.tick_playback();
        terminal.draw(|frame| render(frame, &app))?;

        if app.should_quit {
            break;
        }

        // Use a tight poll only while the receive animation is active
        let poll_ms = if app.screen == Screen::Practice
            && app.item_mode == ItemMode::Receive
            && matches!(app.playback, PlaybackState::Playing { .. })
        {
            15
        } else {
            100
        };
        if event::poll(Duration::from_millis(poll_ms))? {
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
    let title = match app.item_mode {
        ItemMode::Receive => " hamrs-ca — Morse Receive ",
        ItemMode::Transmit => " hamrs-ca — Morse Transmit ",
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(Style::default().add_modifier(Modifier::BOLD));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    match app.screen {
        Screen::Practice => render_practice(frame, inner, app),
        Screen::Score => render_score(frame, inner, app),
    }
}

fn render_practice(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Length(1), // spacer
            Constraint::Length(3), // prompt / morse display
            Constraint::Length(1), // spacer
            Constraint::Length(3), // input box
            Constraint::Min(1),    // spacer
            Constraint::Length(2), // footer
        ])
        .split(area);

    // Header
    let wpm_str = format!("{} WPM", app.wpm);
    let progress = format!("  Item {}/{}  │  {}", app.current + 1, app.total(), wpm_str);
    frame.render_widget(
        Paragraph::new(progress).style(Style::default().add_modifier(Modifier::DIM)),
        chunks[0],
    );

    // Central prompt
    match app.item_mode {
        ItemMode::Receive => render_receive_prompt(frame, chunks[2], app),
        ItemMode::Transmit => render_transmit_prompt(frame, chunks[2], app),
    }

    // Input area
    render_input(frame, chunks[4], app);

    // Footer
    render_footer(frame, chunks[6], app);
}

fn render_receive_prompt(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let code = app.current_item().code;

    let current_element_idx = match &app.playback {
        PlaybackState::Playing { element_idx, .. } => Some(*element_idx),
        PlaybackState::Waiting => None,
    };

    let spans: Vec<Span> = code
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let symbol = if ch == '.' { "·" } else { "—" };
            let style = match current_element_idx {
                Some(idx) if i == idx => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                Some(idx) if i < idx => Style::default().fg(Color::DarkGray),
                None => Style::default().fg(Color::White),
                _ => Style::default().fg(Color::DarkGray),
            };
            Span::styled(format!(" {symbol}"), style)
        })
        .collect();

    let label = Span::styled("  Decode: ", Style::default().add_modifier(Modifier::DIM));
    let mut line_spans = vec![label];
    line_spans.extend(spans);

    frame.render_widget(
        Paragraph::new(Line::from(line_spans)).style(Style::default()),
        area,
    );
}

fn render_transmit_prompt(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let ch = app.current_item().character;
    let prompt = Paragraph::new(Line::from(vec![
        Span::styled("  Encode: ", Style::default().add_modifier(Modifier::DIM)),
        Span::styled(
            format!(" {ch} "),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(prompt, area);
}

fn render_input(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let (display, style) = match app.result {
        None => {
            let waiting = app.item_mode == ItemMode::Receive
                && matches!(app.playback, PlaybackState::Playing { .. });
            if waiting {
                (
                    "  Listening…".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                )
            } else {
                (
                    format!("  > {}_", app.input),
                    Style::default().fg(Color::White),
                )
            }
        }
        Some(true) => {
            let answer = match app.item_mode {
                ItemMode::Receive => app.current_item().character.to_string(),
                ItemMode::Transmit => app.current_item().code.to_string(),
            };
            (
                format!("  ✓  {}  =  {}", app.input, answer),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        }
        Some(false) => {
            let correct = match app.item_mode {
                ItemMode::Receive => app.current_item().character.to_string(),
                ItemMode::Transmit => app.current_item().code.to_string(),
            };
            (
                format!("  ✗  {}  →  correct: {}", app.input, correct),
                Style::default().fg(Color::Red),
            )
        }
    };

    frame.render_widget(Paragraph::new(display).style(style), area);
}

fn render_footer(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let text = if app.result.is_some() {
        "  Enter / Space  Next   q  Quit"
    } else {
        match app.item_mode {
            ItemMode::Receive => match &app.playback {
                PlaybackState::Playing { .. } => "  r  Replay   q  Quit",
                PlaybackState::Waiting => "  Type character + Enter   r  Replay   q  Quit",
            },
            ItemMode::Transmit => "  Type Morse (. - Space) + Enter   q  Quit",
        }
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().add_modifier(Modifier::DIM)),
        area,
    );
}

fn render_score(frame: &mut ratatui::Frame, area: ratatui::layout::Rect, app: &App) {
    let total = app.total() as u32;
    let pct = (app.score * 100) / total.max(1);

    let (label, color) = if pct >= 90 {
        ("★ Excellent (90%+)", Color::Green)
    } else if pct >= 70 {
        ("✓ Good (70%+)", Color::Yellow)
    } else {
        ("Keep practising", Color::Red)
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(format!("  Score: {}/{} ({}%)", app.score, total, pct))
            .style(Style::default().add_modifier(Modifier::BOLD)),
        chunks[1],
    );

    // WPM line — show actual transmit WPM if available
    let wpm_line = if let Some(actual_wpm) = app.effective_transmit_wpm() {
        format!(
            "  Transmit speed: {} WPM  (target: {} WPM)",
            actual_wpm, app.wpm
        )
    } else {
        format!("  Target WPM: {}", app.wpm)
    };
    frame.render_widget(
        Paragraph::new(wpm_line).style(Style::default().fg(color)),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(format!("  {label}")).style(Style::default().fg(color)),
        chunks[3],
    );

    let bar_pct = pct.min(100) as u16;
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color))
        .percent(bar_pct)
        .label(format!("{pct}%"));
    frame.render_widget(
        gauge,
        chunks[4].inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    frame.render_widget(
        Paragraph::new("  q / Enter  Quit").style(Style::default().add_modifier(Modifier::DIM)),
        chunks[5],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::morse::{Charset, MorseConfig, MorseMode, MorseSession};

    fn app_with_times(times: Vec<u64>) -> App {
        let session = MorseSession::build(MorseConfig {
            mode: MorseMode::Transmit,
            wpm: 13,
            charset: Charset::Letters,
            count: 1,
        });
        let mut app = App::new(session);
        app.response_times_ms = times;
        app
    }

    #[test]
    fn wpm_empty_returns_none() {
        let app = app_with_times(vec![]);
        assert_eq!(app.effective_transmit_wpm(), None);
    }

    #[test]
    fn wpm_typical_speed() {
        // 1000ms per char → 60000 / (1000 * 5) = 12 WPM
        let app = app_with_times(vec![1000]);
        assert_eq!(app.effective_transmit_wpm(), Some(12));
    }

    #[test]
    fn wpm_slow_speed() {
        // 6000ms per char → 60000 / (6000 * 5) = 2 WPM
        let app = app_with_times(vec![6000]);
        assert_eq!(app.effective_transmit_wpm(), Some(2));
    }

    #[test]
    fn wpm_fast_speed() {
        // 200ms per char → 60000 / (200 * 5) = 60 WPM
        let app = app_with_times(vec![200]);
        assert_eq!(app.effective_transmit_wpm(), Some(60));
    }

    #[test]
    fn wpm_caps_at_999() {
        // 1ms per char → would be 12000 WPM, capped to 999
        let app = app_with_times(vec![1]);
        assert_eq!(app.effective_transmit_wpm(), Some(999));
    }

    #[test]
    fn wpm_averages_multiple_times() {
        // avg of 1000 and 3000 = 2000ms → 60000 / (2000 * 5) = 6 WPM
        let app = app_with_times(vec![1000, 3000]);
        assert_eq!(app.effective_transmit_wpm(), Some(6));
    }
}
