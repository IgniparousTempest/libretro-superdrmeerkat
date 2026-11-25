use crate::ffi::{
    ControllerDescription, ControllerInfo, Environment, Input, InputDescriptor,
    SET_CONTROLLER_INFO, SET_INPUT_DESCRIPTORS,
};
use std::ffi::CStr;
use super_dr_meerkat::{
    app::App,
    config::MAX_PLAYERS,
    pad::{self, Button},
};

pub const JOYPAD: u32 = 1;
pub const ANALOG: u32 = 5;
const BUTTONS: [(Button, &CStr); 16] = [
    (Button::Red, c"Red - Turn right / Confirm"),
    (Button::Green, c"Green - Soft drop / How to play"),
    (Button::Coin, c"Insert credit"),
    (Button::Start, c"Start / Pause / Finish name"),
    (Button::Up, c"Up / Turn right"),
    (Button::Down, c"Down / Soft drop"),
    (Button::Left, c"Left"),
    (Button::Right, c"Right"),
    (Button::Yellow, c"Yellow - Turn left / Back"),
    (Button::Blue, c"Blue - Hard drop / High scores"),
    (Button::Other, c""),
    (Button::Mute, c"Toggle sound"),
    (Button::Other, c""),
    (Button::Other, c""),
    (Button::Other, c""),
    (Button::Other, c""),
];

pub fn describe(env: Environment, ports: usize) {
    let pad = ControllerDescription {
        description: c"RetroPad".as_ptr(),
        id: JOYPAD,
    };
    let mut controllers = vec![
        ControllerInfo {
            types: &pad,
            num_types: 1
        };
        ports
    ];
    controllers.push(ControllerInfo {
        types: std::ptr::null(),
        num_types: 0,
    });
    unsafe { env(SET_CONTROLLER_INFO, controllers.as_mut_ptr().cast()) };
    let mut entries = Vec::new();
    for port in 0..ports {
        for (id, &(_, text)) in BUTTONS.iter().enumerate().filter(|(_, b)| !b.1.is_empty()) {
            entries.push(InputDescriptor {
                port: port as u32,
                device: JOYPAD,
                index: 0,
                id: id as u32,
                description: text.as_ptr(),
            });
        }
    }
    entries.push(InputDescriptor {
        port: 0,
        device: 0,
        index: 0,
        id: 0,
        description: std::ptr::null(),
    });
    unsafe { env(SET_INPUT_DESCRIPTORS, entries.as_mut_ptr().cast()) };
}

pub fn read(input: Input, port: u32) -> u16 {
    let mut bits = 0;
    for id in 0..16 {
        if unsafe { input(port, JOYPAD, 0, id) } != 0 {
            bits |= 1 << id;
        }
    }
    let x = unsafe { input(port, ANALOG, 0, 0) };
    let y = unsafe { input(port, ANALOG, 0, 1) };
    for (pressed, id) in [
        (x < -16000, 6),
        (x > 16000, 7),
        (y < -16000, 4),
        (y > 16000, 5),
    ] {
        if pressed {
            bits |= 1 << id;
        }
    }
    bits
}

pub fn apply(
    app: &mut App,
    previous: &[u16; MAX_PLAYERS],
    current: &[u16; MAX_PLAYERS],
) -> [[bool; 3]; MAX_PLAYERS] {
    std::array::from_fn(|seat| {
        let bits = current[seat];
        let pressed = bits & !previous[seat];
        for (id, &(button, _)) in BUTTONS.iter().enumerate() {
            if pressed & (1 << id) != 0 {
                pad::press(app, seat, button);
            }
        }
        [
            bits & (1 << 6) != 0,
            bits & (1 << 7) != 0,
            bits & ((1 << 5) | (1 << 1)) != 0,
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super_dr_meerkat::{app::Screen, config::Config, scores::HighScores};

    #[test]
    fn held_select_and_start_generate_one_edge_for_their_seat() {
        let mut app = App::new(
            &Config::default(),
            true,
            HighScores::open(std::env::temp_dir().join("meerkat-retropad-test/scores.toml")),
        );
        let coin = [0, 0, 0, 1 << 2];
        let start = [0, 0, 0, 1 << 3];
        apply(&mut app, &[0; 4], &coin);
        apply(&mut app, &coin, &coin);
        assert_eq!(app.credits, [0, 0, 0, 1]);
        apply(&mut app, &coin, &start);
        apply(&mut app, &start, &start);
        assert_eq!(app.seats, [3]);
        assert_eq!(app.screen, Screen::Ready);
        assert_eq!(app.time_left.as_secs(), 10);
        apply(&mut app, &[0; 4], &start);
        assert_eq!(app.time_left.as_secs(), 9);
    }
}
