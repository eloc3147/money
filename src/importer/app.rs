use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use color_eyre::{Result, eyre::Context};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget, Wrap},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AppConfig {
    import_path: PathBuf,
}

impl AppConfig {
    fn load(path: &Path) -> Result<Self> {
        let mut config_text = String::new();

        File::open(path)
            .and_then(|mut f| f.read_to_string(&mut config_text))
            .wrap_err_with(|| format!("Cannot read config file at {:?}", path))?;

        toml::from_str(&config_text).wrap_err("Malformed config file")
    }
}

#[derive(Debug)]
struct AppState {
    config: AppConfig,
}

#[derive(Debug)]
enum StateOption {
    Ok(AppState),
    Err(String),
}

#[derive(Debug)]
pub struct ImporterApp {
    data_dir: PathBuf,
    state: StateOption,

    counter: u8,
    exit: bool,
}

impl ImporterApp {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir()
            .expect("OS User data directory missing")
            .join("money_app");

        let state = match AppConfig::load(&data_dir.join("config.toml")) {
            Ok(config) => StateOption::Ok(AppState { config }),
            Err(err) => StateOption::Err(format!("{:#}", err)),
        };

        Self {
            data_dir,
            state,
            counter: 0,
            exit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left => self.decrement_counter(),
            KeyCode::Right => self.increment_counter(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn increment_counter(&mut self) {
        self.counter += 1;
    }

    fn decrement_counter(&mut self) {
        self.counter -= 1;
    }
}

impl Widget for &ImporterApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Money Importer ".bold());

        let block;
        let contents;
        match &self.state {
            StateOption::Ok(state) => {
                let instructions = Line::from(vec![
                    " Decrement ".into(),
                    "<Left>".blue().bold(),
                    " Increment ".into(),
                    "<Right>".blue().bold(),
                    " Quit ".into(),
                    "<Q> ".blue().bold(),
                ]);

                block = Block::bordered()
                    .title(title.centered())
                    .title_bottom(instructions.centered())
                    .border_set(border::THICK);

                contents = Text::from(format!("{:#?}", state));
            }
            StateOption::Err(err) => {
                let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);

                block = Block::bordered()
                    .title(title.centered())
                    .title_bottom(instructions.centered())
                    .border_set(border::THICK)
                    .border_style(Color::LightRed);

                contents =
                    Text::from(format!("Error. Cannot load:\n\n{}", err)).style(Color::LightYellow);
            }
        }

        Paragraph::new(contents)
            .wrap(Wrap { trim: false })
            .block(block)
            .render(area, buf);
    }
}
