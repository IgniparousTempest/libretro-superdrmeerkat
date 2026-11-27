use sdl2::{
    controller::{Axis, Button, GameController},
    keyboard::{KeyboardState, Scancode as Key},
    GameControllerSubsystem,
};
use super_dr_meerkat::app::{App, Command, Nav, Screen};
use super_dr_meerkat::config::MAX_PLAYERS;
use super_dr_meerkat::game::Action;
use super_dr_meerkat::scores::NameKey;

// Left, right, soft drop, counterclockwise, clockwise, hard drop.
const KEYS: [[Key; 6]; MAX_PLAYERS] = [
    [Key::Left, Key::Right, Key::Down, Key::Z, Key::X, Key::Space],
    [Key::A, Key::D, Key::S, Key::Q, Key::E, Key::F],
    [Key::J, Key::L, Key::K, Key::U, Key::O, Key::N],
    [Key::Kp4, Key::Kp6, Key::Kp2, Key::Kp7, Key::Kp9, Key::Kp0],
];
const ACTIONS: [Action; 6] = [
    Action::Left,
    Action::Right,
    Action::Down,
    Action::Counterclockwise,
    Action::Clockwise,
    Action::Drop,
];
const UP: [Key; MAX_PLAYERS] = [Key::Up, Key::W, Key::I, Key::Kp8];
// A separate green button lets a keyboard encoder wire all four face buttons.
const SOFT_DROP: [Key; MAX_PLAYERS] = [Key::C, Key::R, Key::Y, Key::Kp5];
const COIN: [Key; MAX_PLAYERS] = [Key::Num5, Key::Num6, Key::Num7, Key::Num8];
const START: [Key; MAX_PLAYERS] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4];

pub fn key_press(app: &mut App, key: Key) {
    let coin = COIN.iter().position(|&k| key == k);
    if app.press(coin) {
        return;
    }
    if let Some(seat) = START.iter().position(|&k| key == k) {
        app.command(Command::Start(seat));
        return;
    }
    if matches!(app.screen, Screen::Pause | Screen::Result(_)) {
        for &s in &app.seats {
            let nav = if key == UP[s] {
                Nav::Up
            } else if key == KEYS[s][2] {
                Nav::Down
            } else if key == KEYS[s][4] {
                Nav::Confirm
            } else {
                continue;
            };
            app.navigate(nav);
            return;
        }
    }
    if app.screen == Screen::Names {
        let actions = [
            NameKey::Left,
            NameKey::Right,
            NameKey::Down,
            NameKey::Delete,
            NameKey::Select,
            NameKey::Select,
        ];
        for (i, keys) in KEYS.iter().enumerate() {
            let action = if key == UP[i] {
                Some(NameKey::Up)
            } else {
                keys.iter().position(|&k| k == key).map(|k| actions[k])
            };
            if let Some(action) = action {
                app.command(Command::Name(i, action));
                return;
            }
        }
        match key {
            Key::Return => app.command(Command::Name(0, NameKey::Select)),
            Key::Backspace => app.command(Command::Name(0, NameKey::Delete)),
            Key::M => app.navigate(Nav::Mute),
            _ => {}
        }
        return;
    }
    match key {
        Key::M => app.navigate(Nav::Mute),
        Key::Escape => app.navigate(Nav::Back),
        Key::P => app.navigate(Nav::Pause),
        Key::Return if matches!(app.screen, Screen::Title | Screen::Ready) => {
            app.command(Command::Start(0))
        }
        Key::Tab if app.screen == Screen::Title => app.command(Command::Scores),
        Key::Return | Key::KpEnter if app.screen == Screen::Play => app.navigate(Nav::Pause),
        Key::Return | Key::KpEnter => app.navigate(Nav::Confirm),
        _ if app.screen == Screen::Play => {
            for (i, keys) in KEYS.iter().enumerate() {
                if let Some(k) = keys.iter().position(|&k| k == key) {
                    app.play(i, ACTIONS[k]);
                    return;
                }
            }
            if let Some(i) = UP.iter().position(|&k| k == key) {
                app.play(i, Action::Clockwise);
            } else if let Some(i) = SOFT_DROP.iter().position(|&k| k == key) {
                app.play(i, Action::Down);
            }
        }
        _ => {
            let nav = match key {
                Key::Up => Nav::Up,
                Key::Down => Nav::Down,
                Key::H | Key::F1 => Nav::Help,
                Key::Space => Nav::Confirm,
                _ => return,
            };
            app.navigate(nav);
        }
    }
}

pub(super) fn pad_press(app: &mut App, player: usize, button: Button) {
    use super_dr_meerkat::pad;
    let button = match button {
        Button::Back => pad::Button::Coin,
        Button::Start => pad::Button::Start,
        Button::RightShoulder => pad::Button::Mute,
        Button::DPadLeft => pad::Button::Left,
        Button::DPadRight => pad::Button::Right,
        Button::DPadUp => pad::Button::Up,
        Button::DPadDown => pad::Button::Down,
        Button::A => pad::Button::Red,
        Button::B => pad::Button::Yellow,
        Button::X => pad::Button::Green,
        Button::Y => pad::Button::Blue,
        _ => pad::Button::Other,
    };
    pad::press(app, player, button);
}

struct Pad {
    controller: GameController,
    stick: [i8; 2],
}

pub struct Pads {
    system: GameControllerSubsystem,
    slots: Vec<Option<Pad>>,
}

const DEAD_ZONE: i16 = 16_000;

impl Pads {
    pub fn new(system: GameControllerSubsystem, seats: usize) -> Self {
        let mut pads = Self {
            system,
            slots: (0..seats).map(|_| None).collect(),
        };
        for i in 0..pads.system.num_joysticks().unwrap_or(0) {
            pads.add(i);
        }
        pads
    }
    pub fn add(&mut self, index: u32) {
        if !self.system.is_game_controller(index) || self.slots.iter().all(Option::is_some) {
            return;
        }
        if let Ok(controller) = self.system.open(index) {
            if self.seat(controller.instance_id()).is_some() {
                return;
            }
            let slot = self.slots.iter().position(Option::is_none).unwrap();
            eprintln!("Player {} controller: {}", slot + 1, controller.name());
            self.slots[slot] = Some(Pad {
                controller,
                stick: [0; 2],
            });
        }
    }
    fn seat(&self, id: u32) -> Option<usize> {
        self.slots
            .iter()
            .position(|p| p.as_ref().is_some_and(|p| p.controller.instance_id() == id))
    }
    pub fn remove(&mut self, id: u32, app: &mut App) {
        if let Some(i) = self.seat(id) {
            self.slots[i] = None;
            if app.seats.contains(&i) {
                app.command(Command::Pause);
            }
            for idx in 0..self.system.num_joysticks().unwrap_or(0) {
                self.add(idx);
            }
        }
    }
    pub fn press(&self, app: &mut App, id: u32, button: Button) {
        if let Some(i) = self.seat(id) {
            pad_press(app, i, button);
        }
    }
    pub fn axis(&mut self, app: &mut App, id: u32, axis: Axis, value: i16) {
        let Some(i) = self.seat(id) else { return };
        let which = match axis {
            Axis::LeftX => 0,
            Axis::LeftY => 1,
            _ => return,
        };
        let sign = if value < -DEAD_ZONE {
            -1
        } else if value > DEAD_ZONE {
            1
        } else {
            0
        };
        let old = &mut self.slots[i].as_mut().unwrap().stick[which];
        if *old == sign {
            return;
        }
        *old = sign;
        let button = match (which, sign) {
            (0, -1) => Button::DPadLeft,
            (0, 1) => Button::DPadRight,
            (1, -1) => Button::DPadUp,
            (1, 1) => Button::DPadDown,
            _ => return,
        };
        pad_press(app, i, button);
    }
    pub fn held(&self, keys: KeyboardState) -> [[bool; 3]; MAX_PLAYERS] {
        std::array::from_fn(|i| {
            std::array::from_fn(|axis| {
                keys.is_scancode_pressed(KEYS[i][axis])
                    || (axis == 2 && keys.is_scancode_pressed(SOFT_DROP[i]))
                    || self
                        .slots
                        .get(i)
                        .and_then(Option::as_ref)
                        .is_some_and(|pad| {
                            let button =
                                [Button::DPadLeft, Button::DPadRight, Button::DPadDown][axis];
                            let stick = [pad.stick[0] < 0, pad.stick[0] > 0, pad.stick[1] > 0];
                            pad.controller.button(button)
                                || stick[axis]
                                || (axis == 2 && pad.controller.button(Button::X))
                        })
            })
        })
    }
}
