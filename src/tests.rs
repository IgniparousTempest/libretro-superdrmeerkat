use crate::input::{key_press, pad_press};
use sdl2::{controller::Button, keyboard::Scancode as Key};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Barrier,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use super_dr_meerkat::{
    app::{App, Command, Screen},
    config::Config,
    demo::Demo,
    game::{Action, Kind, Outcome, Phase, Sound},
    scores::{HighScores, NameKey, NamePad, DELETE, DONE},
};

const IDLE: [[bool; 3]; 4] = [[false; 3]; 4];

struct Cabinet(PathBuf);
impl Cabinet {
    fn new() -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "meerkat-{}-{now}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn scores(&self) -> HighScores {
        HighScores::open(self.0.join("high-scores.toml"))
    }
    fn app(&self, cap: usize) -> App {
        App::new(
            &Config {
                max_players: cap,
                ..Config::default()
            },
            false,
            self.scores(),
        )
    }
}
impl Drop for Cabinet {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn join(app: &mut App, seats: &[usize]) {
    for &s in seats {
        app.command(Command::Coin(s));
        app.command(Command::Start(s));
    }
    app.clock(Duration::from_secs(10));
    assert_eq!(app.screen, Screen::Play);
}

#[test]
fn cabinet_buttons_and_menu_joystick_follow_each_seat() {
    let dir = Cabinet::new();
    let bindings = [
        (Key::C, Key::Down, Key::Up, Key::X),
        (Key::R, Key::S, Key::W, Key::E),
        (Key::Y, Key::K, Key::I, Key::O),
        (Key::Kp5, Key::Kp2, Key::Kp8, Key::Kp9),
    ];
    for (seat, (green, down, up, red)) in bindings.into_iter().enumerate() {
        let mut app = dir.app(4);
        join(&mut app, &[seat]);
        key_press(&mut app, green);
        assert_eq!(app.round.as_ref().unwrap().boards[0].score, 1);
        pad_press(&mut app, seat, Button::X);
        assert_eq!(app.round.as_ref().unwrap().boards[0].score, 2);
        pad_press(&mut app, seat, Button::Start);
        key_press(&mut app, down);
        assert_eq!(app.choice, 1);
        key_press(&mut app, up);
        assert_eq!(app.choice, 0);
        key_press(&mut app, red);
        assert_eq!(app.screen, Screen::Play);
    }
}

#[test]
fn sparse_seats_pay_once_and_share_one_wall_clock_countdown() {
    let dir = Cabinet::new();
    let mut app = dir.app(4);
    key_press(&mut app, Key::Num1);
    assert_eq!(app.screen, Screen::Title);
    for key in [Key::Num8, Key::Num8, Key::Num4] {
        key_press(&mut app, key);
    }
    assert_eq!(app.screen, Screen::Ready);
    assert_eq!(app.time_left, Duration::from_secs(10));
    app.clock(Duration::from_millis(2350));
    for key in [Key::Num6, Key::Num2, Key::Num1] {
        key_press(&mut app, key);
    }
    assert_eq!(app.seats, [1, 3]);
    assert_eq!(app.time_left, Duration::from_millis(7650));
    key_press(&mut app, Key::Num4);
    assert_eq!(app.credits, [0, 0, 0, 1]);
    app.clock(Duration::from_millis(6649));
    assert_eq!(app.screen, Screen::Ready);
    app.clock(Duration::from_millis(1));
    assert_eq!(app.screen, Screen::Play);
    let original = app.round.as_ref().unwrap().boards[0].phase;
    app.play(0, Action::Left);
    app.play(3, Action::Right);
    let boards = &app.round.as_ref().unwrap().boards;
    assert_eq!(boards[0].phase, original);
    assert_ne!(boards[1].phase, original);
    assert_eq!(boards[0].cells, boards[1].cells);
}

#[test]
fn every_enabled_seat_can_join_and_abandoning_requires_a_new_credit() {
    let dir = Cabinet::new();
    for cap in 1..=4 {
        let mut app = dir.app(cap);
        for s in 0..4 {
            app.command(Command::Coin(s));
            app.command(Command::Start(s));
        }
        assert_eq!(app.seats.len(), cap);
        assert_eq!(app.credits, [0; 4]);
        assert_eq!(app.screen, Screen::Ready);
        for _ in 0..10 {
            app.command(Command::Start(cap - 1));
        }
        assert_eq!(app.round.as_ref().unwrap().boards.len(), cap);
        app.command(Command::Pause);
        let phase = app.round.as_ref().unwrap().boards[0].phase;
        for _ in 0..120 {
            app.tick(&IDLE);
        }
        assert_eq!(app.round.as_ref().unwrap().boards[0].phase, phase);
        app.command(Command::Title);
        app.command(Command::Start(0));
        assert_eq!(app.screen, Screen::Title);
        assert!(app.round.is_none());
    }
}

#[test]
fn ready_bars_flash_independently_on_joins_and_repeated_start() {
    let dir = Cabinet::new();
    let mut app = dir.app(3);
    app.command(Command::Coin(1));
    app.command(Command::Start(1));
    assert_eq!(app.ready_flash(1), 1.0);
    app.clock(Duration::from_millis(100));
    let first = app.ready_flash(1);
    assert!(first > 0.0 && first < 1.0);
    app.command(Command::Start(0));
    app.command(Command::Coin(0));
    assert_eq!(app.ready_flash(0), 0.0);
    app.command(Command::Start(0));
    assert_eq!(app.ready_flash(0), 1.0);
    assert_eq!(app.ready_flash(1), first);
    app.clock(Duration::from_millis(100));
    let other = app.ready_flash(0);
    let remaining = app.time_left;
    app.command(Command::Start(1));
    assert_eq!(app.ready_flash(1), 1.0);
    assert_eq!(app.ready_flash(0), other);
    assert_eq!(app.time_left, remaining - Duration::from_secs(1));
    app.clock(Duration::from_millis(350));
    assert_eq!(app.ready_flash(0), 0.0);
    assert_eq!(app.ready_flash(1), 0.0);
}

#[test]
fn solo_carries_score_and_seat_until_the_run_finishes() {
    let dir = Cabinet::new();
    let mut app = dir.app(4);
    join(&mut app, &[3]);
    let b = &mut app.round.as_mut().unwrap().boards[0];
    b.score = 870;
    b.phase = Phase::Cleared;
    app.tick(&IDLE);
    assert_eq!(app.screen, Screen::Result(Outcome::SoloClear));
    app.command(Command::Next);
    let b = &app.round.as_ref().unwrap().boards[0];
    assert_eq!((b.level, b.score), (2, 870));
    assert_eq!(app.seats, [3]);
    assert!(!dir.0.exists());
    app.round.as_mut().unwrap().boards[0].phase = Phase::Out;
    app.tick(&IDLE);
    app.clock(Duration::from_secs(4));
    assert_eq!(app.screen, Screen::Names);
    app.command(Command::Start(0));
    assert!(!app.names[3].as_ref().unwrap().done);
    key_press(&mut app, Key::Kp9);
    key_press(&mut app, Key::Num4);
    assert_eq!(app.screen, Screen::Scores);
    assert!(dir
        .scores()
        .entries()
        .iter()
        .any(|e| e.name == "A" && e.score == 870));
}

#[test]
fn attract_pages_cycle_in_order_and_wake_without_spending() {
    let dir = Cabinet::new();
    let mut app = dir.app(4);
    key_press(&mut app, Key::Num6);
    for (seconds, expected) in [
        (8, Screen::Guide),
        (8, Screen::Demo),
        (12, Screen::Scores),
        (8, Screen::Title),
    ] {
        app.clock(Duration::from_secs(seconds));
        assert_eq!(app.screen, expected);
        for _ in 0..30 {
            app.tick(&IDLE);
        }
    }
    assert_eq!(app.credits[1], 1);
    assert!(app.round.is_none() && !dir.0.exists());
    app.clock(Duration::from_secs(8));
    key_press(&mut app, Key::Num2);
    assert_eq!(app.screen, Screen::Title);
    assert_eq!(app.credits[1], 1);
    app.clock(Duration::from_secs(8));
    app.clock(Duration::from_secs(8));
    key_press(&mut app, Key::Num7);
    assert_eq!(app.screen, Screen::Title);
    assert_eq!(app.credits[2], 1);
    app.command(Command::Scores);
    key_press(&mut app, Key::F10);
    assert_eq!(app.screen, Screen::Title);
}

#[test]
fn demo_starts_in_varied_played_positions_and_keeps_solving() {
    let mut demo = Demo::new(37);
    let start = demo.board.cells;
    let score = demo.board.score;
    let germs = demo.board.germs();
    assert!(start
        .iter()
        .flatten()
        .any(|b| matches!(b.kind, Kind::Pill(_))));
    assert_ne!(Demo::new(97).board.cells, start);
    let mut sounds = Vec::new();
    for _ in 0..720 {
        demo.tick(&mut sounds);
    }
    assert!(sounds.iter().any(|s| matches!(s, Sound::Land)));
    assert!(sounds.iter().any(|s| matches!(s, Sound::Match(_))));
    assert_ne!(demo.board.cells, start);
    assert!(demo.board.score > score && demo.board.germs() < germs);
}

#[test]
fn qualifiers_compete_for_the_same_ten_places_and_keep_incumbent_ties() {
    let dir = Cabinet::new();
    let scores = dir.scores();
    let entries = scores.entries();
    assert_eq!(entries.len(), 10);
    assert_eq!(entries[0].name, "Chief Lion");
    assert_eq!(entries[9].name, "Student Hippo");
    assert_eq!(
        scores.qualifiers([(0, 130), (1, 165), (2, 165), (3, 100)]),
        [false, true, false, false]
    );
    assert_eq!(scores.qualifiers([(0, 100)]), [false; 4]);
    assert_eq!(
        scores.qualifiers([(0, 9000), (1, 9000), (2, 9000), (3, 9000)]),
        [true; 4]
    );
}

#[test]
fn letter_grid_accepts_eight_letters_and_supports_correction() {
    let mut pad = NamePad::default();
    pad.press(NameKey::Right);
    for _ in 0..12 {
        pad.press(NameKey::Select);
    }
    assert_eq!(pad.text, "BBBBBBBB");
    pad.press(NameKey::Pick(DELETE));
    pad.press(NameKey::Pick(25));
    assert_eq!(pad.text, "BBBBBBBZ");
    pad.press(NameKey::Pick(DONE));
    pad.press(NameKey::Delete);
    assert!(pad.done);
    assert_eq!(pad.text, "BBBBBBBZ");
}

#[test]
fn racers_can_enter_and_save_names_independently_over_their_seats() {
    let dir = Cabinet::new();
    let mut app = dir.app(4);
    join(&mut app, &[3, 1, 0]);
    let boards = &mut app.round.as_mut().unwrap().boards;
    boards[0].score = 40;
    boards[1].score = 6000;
    boards[2].score = 4000;
    boards[2].phase = Phase::Cleared;
    app.tick(&IDLE);
    assert_eq!(app.screen, Screen::Result(Outcome::Winner(2)));
    app.clock(Duration::from_secs(4));
    assert!(app.names[0].is_none() && app.names[2].is_none());
    // P2 enters B, P4 enters H. Movement and deletion on one grid never edit another.
    for key in [Key::D, Key::Kp2, Key::E, Key::Kp9, Key::Q, Key::E] {
        key_press(&mut app, key);
    }
    assert_eq!(app.names[1].as_ref().unwrap().text, "B");
    assert_eq!(app.names[3].as_ref().unwrap().text, "H");
    key_press(&mut app, Key::Num2);
    assert_eq!(app.screen, Screen::Names);
    assert_eq!(dir.scores().entries()[0].name, "B");
    app.clock(Duration::from_secs(60));
    assert_eq!(app.screen, Screen::Scores);
    let saved = dir.scores();
    assert_eq!(saved.entries().len(), 10);
    assert_eq!(saved.entries()[1].name, "H");
    assert_eq!(saved.entries()[1].score, 4000);
    app.clock(Duration::from_secs(8));
    key_press(&mut app, Key::Num4);
    assert_eq!(app.screen, Screen::Title);
}

#[test]
fn an_empty_name_and_a_score_below_the_cut_leave_the_table_alone() {
    let dir = Cabinet::new();
    let mut app = dir.app(1);
    for score in [10, 100, 101] {
        join(&mut app, &[0]);
        let b = &mut app.round.as_mut().unwrap().boards[0];
        b.score = score;
        b.phase = Phase::Out;
        app.tick(&IDLE);
        app.command(Command::Finish);
        if score > 100 {
            assert_eq!(app.screen, Screen::Names);
            app.command(Command::Name(0, NameKey::Finish));
        }
        assert_eq!(app.screen, Screen::Scores);
        app.clock(Duration::from_secs(8));
    }
    assert!(!dir.0.exists());
}

#[test]
fn concurrent_saves_merge_and_a_damaged_file_can_be_repaired_and_retried() {
    let dir = Cabinet::new();
    let first = dir.scores();
    let second = dir.scores();
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        for (mut scores, name) in [(first, "ONE"), (second, "TWO")] {
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                scores.record(name, 6000);
                assert!(!scores.storage_error);
            });
        }
    });
    let path = dir.0.join("high-scores.toml");
    let saved = fs::read(&path).unwrap();
    let mut scores = dir.scores();
    assert_eq!(
        scores.entries().iter().filter(|e| e.score == 6000).count(),
        2
    );
    fs::write(&path, "broken = [").unwrap();
    scores.record("THREE", 7000);
    assert!(scores.storage_error);
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken = [");
    fs::write(&path, saved).unwrap();
    scores.flush();
    assert!(!scores.storage_error);
    assert_eq!(dir.scores().entries()[0].name, "THREE");
}
