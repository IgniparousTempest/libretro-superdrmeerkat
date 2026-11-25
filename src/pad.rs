use crate::{
    app::{App, Command, Nav, Screen},
    game::Action,
    scores::NameKey,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Coin,
    Start,
    Mute,
    Left,
    Right,
    Up,
    Down,
    Red,
    Yellow,
    Green,
    Blue,
    Other,
}

pub fn press(app: &mut App, player: usize, button: Button) {
    if app.press((button == Button::Coin).then_some(player)) {
        return;
    }
    if button == Button::Start {
        app.command(Command::Start(player));
        return;
    }
    if button == Button::Mute {
        app.navigate(Nav::Mute);
        return;
    }
    if app.screen == Screen::Names {
        let key = match button {
            Button::Left => NameKey::Left,
            Button::Right => NameKey::Right,
            Button::Up => NameKey::Up,
            Button::Down => NameKey::Down,
            Button::Red | Button::Blue => NameKey::Select,
            Button::Yellow => NameKey::Delete,
            _ => return,
        };
        app.command(Command::Name(player, key));
    } else if app.screen == Screen::Play {
        let action = match button {
            Button::Left => Action::Left,
            Button::Right => Action::Right,
            Button::Down | Button::Green => Action::Down,
            Button::Red | Button::Up => Action::Clockwise,
            Button::Yellow => Action::Counterclockwise,
            Button::Blue => Action::Drop,
            _ => return,
        };
        app.play(player, action);
    } else {
        if matches!(app.screen, Screen::Pause | Screen::Result(_)) && !app.seats.contains(&player) {
            return;
        }
        let nav = match button {
            Button::Up => Nav::Up,
            Button::Down => Nav::Down,
            Button::Red => Nav::Confirm,
            Button::Yellow => Nav::Back,
            Button::Green => Nav::Help,
            Button::Blue if app.screen == Screen::Title => {
                app.command(Command::Scores);
                return;
            }
            _ => return,
        };
        app.navigate(nav);
    }
}
