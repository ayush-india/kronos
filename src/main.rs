mod app;
mod config;

use std::{
    env::{self, args},
    error::Error,
    io,
    path::{PathBuf, Path},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, BorderType, Borders, Cell, Gauge, List, ListItem, Row, Table, Tabs},
    Frame, Terminal,
};

use app::{App, AppTab, InputMode};
use config::Config;
use kronos::gen_funcs;

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, DisableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_secs(1);
    let app = App::new();
    let cfg = Config::new();

    let res = run_app(&mut terminal, app, cfg, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    cfg: Config,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    let input: Vec<String> = env::args().collect();
    let current_dir = env::current_dir().unwrap();
    if current_dir.read_dir()?.next().is_none() {
        let join = Path::new(&current_dir).join(&input[1]);
        app.queue_items.add(join);
    }
    loop {
        terminal.draw(|f| ui(f, &mut app, &cfg))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            // different keys depending on which browser tab
            if let Event::Key(key) = event::read()? {
                match app.input_mode() {
                    InputMode::Queue => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('p') => app.music_handle.play_pause(),
                        KeyCode::Char('g') => app.music_handle.skip(),
                        KeyCode::Enter => {
                            if let Some(i) = app.queue_items.item() {
                                app.music_handle.play(i.clone());
                            };
                        }
                        _ => {}
                    },
                    InputMode::Browser => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        _ => {},
                    },
                    InputMode::Controls => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        // KeyCode::Char('q') => return Ok(()),
                        // KeyCode::Char('p') => app.music_handle.play_pause(),
                        // KeyCode::Char('g') => app.music_handle.skip(),
                        // KeyCode::Down | KeyCode::Char('j') => app.control_table.next(),
                        // KeyCode::Up | KeyCode::Char('k') => app.control_table.previous(),
                        // KeyCode::Tab => {
                        //     app.next();
                        //     match app.input_mode() {
                        //         InputMode::Controls => app.set_input_mode(InputMode::Browser),
                        //         _ => app.set_input_mode(InputMode::Controls),
                        //     };
                        // }
                        _ => {}
                    },
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App, cfg: &Config) {
    // Total Size
    let size = f.size();
    //
    // // chunking from top to bottom, 3 gets tabs displayed, the rest goes to item layouts
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(size);

    // Main Background block, covers entire screen
    let _block = Block::default().style(Style::default().bg(cfg.background()));

    // Tab Title items collected
    let titles = app
        .titles
        .iter()
        .map(|_t| {
            Spans::from(vec![
                Span::styled("Music", Style::default().fg(cfg.highlight_background())), // CHANGE FOR CUSTOMIZATION
            ])
        })
        .collect();

    // Box Around Tab Items
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.active_tab as usize)
        .style(Style::default().fg(cfg.foreground()))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(cfg.background()),
        );

    match app.active_tab {
        AppTab::Music => music_tab(f, app, chunks[1], cfg),
        AppTab::Controls => unimplemented!(),
    };
}

fn music_tab<B: Backend>(f: &mut Frame<B>, app: &mut App, chunks: Rect, cfg: &Config) {
    // split into left / right
    let browser_queue = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(chunks);
    // f.size()

    // queue and playing sections (sltdkh)
    let queue_playing = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(100 - cfg.progress_bar()),
                Constraint::Percentage(cfg.progress_bar()),
            ]
            .as_ref(),
        )
        .split(browser_queue[1]);

    // convert app items to text
    let items: Vec<ListItem> = app
        .browser_items
        .items()
        .iter()
        .map(|i| ListItem::new(Text::from(i.to_owned())))
        .collect();

    // Create a List from all list items and highlight the currently selected one // RENDER 1
    let items = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Browser")
                .title_alignment(Alignment::Left)
                .border_type(BorderType::Rounded),
        )
        .style(Style::default().fg(cfg.foreground()))
        .highlight_style(
            Style::default()
                .bg(cfg.highlight_background())
                .fg(cfg.highlight_foreground())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let queue_items: Vec<ListItem> = app
        .queue_items
        .items()
        .iter()
        .map(|i| ListItem::new(Text::from(gen_funcs::audio_display(i))))
        .collect();

    let queue_title = format!(
        "| Queue: {queue_items} Songs |{total_time}",
        queue_items = app.queue_items.length(),
        total_time = app.queue_items.total_time(),
    );

    let queue_items = List::new(queue_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(queue_title)
                .title_alignment(Alignment::Left)
                .border_type(BorderType::Rounded),
        )
        .style(Style::default().fg(cfg.foreground()))
        .highlight_style(
            Style::default()
                .bg(cfg.highlight_background())
                .fg(cfg.highlight_foreground())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let playing_title = Text::from(format!("| {current_song} |", current_song = app.current_song()));
    let pp = Rect::new(0,0,5,5);


}
