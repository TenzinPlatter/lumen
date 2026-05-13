use std::io::{IsTerminal, Write, stdout};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use can_dbc::Dbc;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement};
use lumen_common::{DEFAULT_SOCKET_PATH, HudSnapshot};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod encode;
mod state;
mod transport;

use state::SimState;
use transport::Publisher;

const TICK: Duration = Duration::from_millis(20);
const RENDER_EVERY: u32 = 5;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let dbc_src = include_str!("../../dbc/hyundai_kia_generic.dbc");
    let dbc = Dbc::try_from(dbc_src)
        .map_err(|e| anyhow::anyhow!("{e:?}"))
        .context("parsing bundled opendbc Hyundai/Kia file")?;

    info!(
        messages = dbc.messages.len(),
        nodes = dbc.nodes.len(),
        "loaded dbc"
    );

    let publisher = Publisher::start(Path::new(DEFAULT_SOCKET_PATH))?;

    let interactive = std::io::stdin().is_terminal();
    let pedal_mode = interactive && supports_keyboard_enhancement().unwrap_or(false);

    println!(
        "lumen-sim — hold ↑/↓ for speed, →/← for rpm, space=coast, q=quit {}",
        if pedal_mode { "(pedal mode)" } else { "(step mode — terminal lacks kitty kbd protocol)" }
    );

    if interactive {
        enable_raw_mode().context("entering raw terminal mode")?;
        if pedal_mode {
            execute!(
                stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
            )
            .context("enabling kitty keyboard protocol")?;
        }
    } else {
        info!("stdin is not a TTY — running headlessly, no keyboard input");
    }

    let result = run_loop(&dbc, &publisher, interactive, pedal_mode);

    if interactive {
        if pedal_mode {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        }
        let _ = disable_raw_mode();
        println!();
    }
    result
}

#[derive(Default)]
struct Held {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl Held {
    fn speed_dir(&self) -> f32 {
        (self.up as i32 - self.down as i32) as f32
    }
    fn rpm_dir(&self) -> f32 {
        (self.right as i32 - self.left as i32) as f32
    }
}

fn run_loop(dbc: &Dbc, publisher: &Publisher, interactive: bool, pedal_mode: bool) -> Result<()> {
    let mut state = SimState::new();
    let mut held = Held::default();
    let mut next_tick = Instant::now() + TICK;
    let mut last_tick = Instant::now();
    let mut ticks_since_render: u32 = 0;

    loop {
        let now = Instant::now();
        let wait = next_tick.saturating_duration_since(now);
        if interactive
            && event::poll(wait)?
            && let Event::Key(key) = event::read()?
        {
            // Quit handling — works in any mode.
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(())
                }
                KeyCode::Char(' ') if key.kind == KeyEventKind::Press => {
                    state.coast();
                    held = Held::default();
                }
                _ => {}
            }

            if pedal_mode {
                let pressed = key.kind == KeyEventKind::Press;
                let released = key.kind == KeyEventKind::Release;
                if pressed || released {
                    match key.code {
                        KeyCode::Up => held.up = pressed,
                        KeyCode::Down => held.down = pressed,
                        KeyCode::Right => held.right = pressed,
                        KeyCode::Left => held.left = pressed,
                        _ => {}
                    }
                    state.set_speed_pedal(held.speed_dir());
                    state.set_rpm_pedal(held.rpm_dir());
                }
            } else if key.kind == KeyEventKind::Press {
                // Step mode: nudge on each press; the OS auto-repeat does the
                // "hold" work, at the cost of a leading delay before repeat.
                match key.code {
                    KeyCode::Up => state.nudge_speed(SimState::SPEED_NUDGE),
                    KeyCode::Down => state.nudge_speed(-SimState::SPEED_NUDGE),
                    KeyCode::Right => state.nudge_rpm(SimState::RPM_NUDGE),
                    KeyCode::Left => state.nudge_rpm(-SimState::RPM_NUDGE),
                    _ => {}
                }
            }
        } else if !interactive {
            std::thread::sleep(wait);
        }

        if Instant::now() >= next_tick {
            let dt = last_tick.elapsed().as_secs_f32();
            last_tick = Instant::now();
            state.tick(dt);

            // Exercise the dbc encoder every tick — bytes are not transmitted
            // yet but this validates the encoding under live values.
            let _engine = encode::encode_engine(dbc, state.engine())?;
            let _wheels = encode::encode_wheel_speeds(dbc, state.wheels())?;

            publisher.publish(HudSnapshot {
                rpm: state.rpm,
                speed_kmh: state.speed_kmh,
            });

            ticks_since_render += 1;
            if ticks_since_render >= RENDER_EVERY {
                render_status(&state, &held);
                ticks_since_render = 0;
            }

            next_tick += TICK;
            if next_tick < Instant::now() {
                next_tick = Instant::now() + TICK;
            }
        }
    }
}

fn render_status(state: &SimState, held: &Held) {
    let rpm_arrow = arrow(held.rpm_dir());
    let speed_arrow = arrow(held.speed_dir());
    print!(
        "\r  rpm {:>5.0} {}    speed {:>5.1} km/h {}        ",
        state.rpm, rpm_arrow, state.speed_kmh, speed_arrow
    );
    let _ = stdout().flush();
}

fn arrow(direction: f32) -> &'static str {
    if direction > 0.0 {
        "↑"
    } else if direction < 0.0 {
        "↓"
    } else {
        " "
    }
}
