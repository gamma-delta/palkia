use std::collections::HashMap;
use std::io::{stdout, Write};
use std::time::Duration;

use aglet::{Coord, CoordVec, Direction9};
use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::Color;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, event, style, terminal, ExecutableCommand, QueueableCommand};

use palkia::prelude::*;

fn main() -> crossterm::Result<()> {
    let mut world = World::new();

    world.register_component::<Positioned>();
    world.register_component::<AiRandomWanderer>();
    world.register_component::<AiFollower>();
    world.register_component::<Renderable>();

    world.insert_resource_default::<TerminalGfx>();

    for i in 0..10 {
        let target = world
            .spawn()
            .with(AiRandomWanderer)
            .with(Positioned(Coord::new(5 + i * 3, 10)))
            // how ergonomic
            .with(Renderable((b'A' + i as u8) as char, Color::Green))
            .build();
        world
            .spawn()
            .with(AiFollower(target))
            .with(Positioned(Coord::new(2 + i * 6, 30)))
            .with(Renderable((b'a' + i as u8) as char, Color::Magenta))
            .build();
    }

    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    loop {
        *world.get_resource::<TerminalGfx>().unwrap() = TerminalGfx::default();

        world.dispatch_to_all(EvStepAI::new());
        world.dispatch_to_all(EvRender::new());
        world.finalize();

        stdout().queue(Clear(ClearType::All))?;

        let gfx = world.get_resource::<TerminalGfx>().unwrap();
        for (pos, (ch, col)) in gfx.0.iter() {
            stdout()
                .queue(cursor::MoveTo(pos.x as _, pos.y as _))?
                .queue(style::SetForegroundColor(*col))?
                .queue(style::Print(*ch))?;
        }

        stdout().flush()?;

        if event::poll(Duration::from_secs_f32(1.0 / 5.0))? {
            if let event::Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) = event::read()?
            {
                break;
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    Ok(())
}

#[derive(Clone)]
struct Positioned(Coord);

impl Positioned {
    fn on_render(&self, mut event: EvRender, _: Entity, _: &WorldAccess) -> EvRender {
        debug_assert_eq!(event.position, None);
        event.position = Some(self.0);
        event
    }

    fn on_step_ai(&mut self, event: EvStepAI, _: Entity, _: &WorldAccess) -> EvStepAI {
        let target = self.0.to_icoord() + event.move_dir.deltas();
        if let Ok(target) = target.try_into() {
            self.0 = target;
        }
        event
    }
}

impl Component for Positioned {
    fn register_listeners(builder: ListenerBuilder<Self>) -> ListenerBuilder<Self>
    where
        Self: Sized,
    {
        builder
            .listen_write(Self::on_step_ai)
            .listen_read(Self::on_render)
    }
}

#[derive(Clone)]
struct Renderable(char, Color);

impl Renderable {
    fn on_render(&self, event: EvRender, _: Entity, access: &WorldAccess) -> EvRender {
        if let Some(pos) = event.position {
            let mut display = access.write_resource::<TerminalGfx>().unwrap();
            display.0.insert(pos, (self.0, self.1));
        }
        event
    }
}

impl Component for Renderable {
    fn register_listeners(builder: ListenerBuilder<Self>) -> ListenerBuilder<Self>
    where
        Self: Sized,
    {
        builder.listen_read(Self::on_render)
    }
}

// AI Components

struct AiRandomWanderer;

impl Component for AiRandomWanderer {
    fn register_listeners(builder: ListenerBuilder<Self>) -> ListenerBuilder<Self>
    where
        Self: Sized,
    {
        // you can use closures too!
        builder.listen_read(
            |_: &Self, mut event: EvStepAI, _: Entity, _: &WorldAccess| {
                let dir = Direction9::DIRECTIONS[fastrand::usize(0..9)];
                event.move_dir = dir;
                event
            },
        )
    }
}

struct AiFollower(Entity);
impl Component for AiFollower {
    fn register_listeners(builder: ListenerBuilder<Self>) -> ListenerBuilder<Self>
    where
        Self: Sized,
    {
        builder.listen_read(
            |this: &Self, mut event: EvStepAI, e: Entity, access: &WorldAccess| {
                let here = access.query::<&Positioned>(e);
                let target = access.query::<&Positioned>(this.0);
                if let (Some(here_pos), Some(target_pos)) = (here, target) {
                    let here_pos: CoordVec = here_pos.0.into();
                    let target_pos: CoordVec = target_pos.0.into();
                    event.move_dir = (here_pos - target_pos).point9();
                }
                event
            },
        )
    }
}

#[derive(Debug, Clone)]
struct EvStepAI {
    move_dir: Direction9,
}

impl EvStepAI {
    fn new() -> Self {
        Self {
            move_dir: Direction9::Center,
        }
    }
}

impl Event for EvStepAI {}

#[derive(Debug, Clone)]
struct EvRender {
    position: Option<Coord>,
}

impl EvRender {
    fn new() -> Self {
        Self { position: None }
    }
}

impl Event for EvRender {}

// Resources

#[derive(Default)]
struct TerminalGfx(HashMap<Coord, (char, Color)>);

impl Resource for TerminalGfx {}
