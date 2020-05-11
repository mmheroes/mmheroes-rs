use mmheroes_core::{
    logic::{Game, GameMode},
    ui::{self, renderer::RendererRequest, recording, *},
};
use pancurses::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

mod screen {
    use super::{endwin, initscr, Window};

    /// A RAII object responsible for initializing and cleaning up the curses
    /// window.
    pub(crate) struct ScreenRAII {
        window: Window,
    }

    impl ScreenRAII {
        pub(crate) fn new() -> ScreenRAII {
            ScreenRAII { window: initscr() }
        }
    }

    impl Drop for ScreenRAII {
        fn drop(&mut self) {
            endwin();
        }
    }

    impl std::ops::Deref for ScreenRAII {
        type Target = Window;

        fn deref(&self) -> &Self::Target {
            &self.window
        }
    }
}

use screen::ScreenRAII;

type Log = String;
type Logger = Mutex<RefCell<recording::InputRecorder<'static, Log>>>;

fn getch<'a>(window: &ScreenRAII, logger: &Logger) -> ui::Input {
    loop {
        let ui_input = match window.getch() {
            None | Some(pancurses::Input::KeyResize) => continue,
            Some(pancurses::Input::KeyUp) => ui::Input::KeyUp,
            Some(pancurses::Input::KeyDown) => ui::Input::KeyDown,
            Some(pancurses::Input::Character('\n')) => ui::Input::Enter,
            Some(_) => ui::Input::Other,
        };
        {
            let logger = logger.lock().unwrap();
            logger.borrow_mut().record_input(ui_input).unwrap();
        }
        break ui_input;
    }
}

fn main() {
    use std::io::Write;

    let window = ScreenRAII::new();
    start_color();
    set_blink(true);
    curs_set(1);

    cbreak();
    noecho();

    window.keypad(true);
    window.nodelay(false);

    // Resize the terminal. We want 24 lines and 80 columns.
    print!("\x1B[8;24;80t");
    std::io::stdout().flush().unwrap();
    resize_term(24, 80);

    window.clear();
    window.refresh();

    let color_pairs = [
        (Color::White, Color::Black),
        (Color::Gray, Color::Black),
        (Color::Red, Color::Black),
        (Color::RedBright, Color::Black),
        (Color::Green, Color::Black),
        (Color::YellowBright, Color::Black),
        (Color::Cyan, Color::Black),
        (Color::CyanBright, Color::Black),
        (Color::WhiteBright, Color::Black),
        (Color::Black, Color::White),
        (Color::Black, Color::Yellow),
        (Color::Black, Color::Gray),
        (Color::Magenta, Color::Black),
        (Color::MagentaBright, Color::Black),
        (Color::BlueBright, Color::Black),
        (Color::Blue, Color::Black),
    ];

    let mut color_pairs_map = HashMap::<(Color, Color), i16>::new();

    for (i, &(foreground, background)) in color_pairs.iter().enumerate() {
        init_pair(i as i16, foreground as i16, background as i16);
        color_pairs_map.insert((foreground, background), i as i16);
    }

    window.bkgd(COLOR_PAIR(
        *color_pairs_map.get(&(Color::White, Color::Black)).unwrap() as chtype,
    ));

    // We save each pressed key to this log, so that if a panic occurs,
    // we could print it and the player could send a useful bug report.
    let logger = {
        let log = &mut *Box::leak(Box::new(Log::new()));
        let logger = Box::new(Mutex::new(RefCell::new(recording::InputRecorder::new(log))));

        // Leak the log and the logger object so that we could obtain a reference with
        // static lifetime. This is needed for accessing it in the panic handler.
        &*Box::leak(logger)
    };

    let mode = match std::env::args().nth(1).as_deref() {
        Some("-3dec-happy-birthday-Diamond") => GameMode::God,
        Some(_) => GameMode::SelectInitialParameters,
        None => GameMode::Normal,
    };

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let mut game = Game::new(mode, seed);
    let mut game_ui = GameUI::new(&mut game);

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        endwin(); // Switch back to normal terminal I/O.
        default_hook(panic_info); // Print panic message and optionally a backtrace.
        eprintln!("Зерно игры: {}", seed);
        let logger = logger.lock().unwrap();
        logger.borrow_mut().flush().unwrap();
        eprintln!("Шаги для воспроизведения бага: {:?}", logger.borrow_mut().output());
        eprintln!("Пожалуйста, отправь зерно игры и шаги для воспроизведения бага разработчику.")
    }));

    let mut input = ui::Input::Enter;
    while game_ui.continue_game(input) {
        for request in game_ui.requests() {
            match request {
                RendererRequest::ClearScreen => window.clear(),
                RendererRequest::Flush => window.refresh(),
                RendererRequest::WriteStr(s) => window.addnstr(s, s.len()),
                RendererRequest::MoveCursor { line, column } => {
                    window.mv(line as i32, column as i32)
                }
                RendererRequest::SetColor {
                    foreground,
                    background,
                } => window.color_set(
                    *color_pairs_map
                        .get(&(foreground, background))
                        .unwrap_or_else(|| {
                            panic!("Unknown color pair: ({:?}, {:?})", foreground, background)
                        }),
                ),
                RendererRequest::Sleep(ms) => napms(ms.0),
            };
        }

        input = getch(&window, logger);
    }
}
