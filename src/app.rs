use crate::{
    config::{Config, MAX_PLAYERS},
    demo::Demo,
    game::{Action, Outcome, Phase, Round, RoundEvent, Sound},
    scores::{HighScores, NameKey, NamePad},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ATTRACT_PAGE_SECONDS: u64 = 8;
const READY_FLASH: Duration = Duration::from_millis(320);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Guide,
    Demo,
    Scores,
    Ready,
    Play,
    Pause,
    Result(Outcome),
    Names,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Coin(usize),
    Start(usize),
    Name(usize, NameKey),
    Guide,
    Scores,
    Back,
    Mute,
    Pause,
    Resume,
    Finish,
    Title,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Confirm,
    Back,
    Help,
    Pause,
    Mute,
}

pub struct App {
    pub screen: Screen,
    enabled: [bool; MAX_PLAYERS],
    pub credits: [u32; MAX_PLAYERS],
    /// Physical cabinet seats in bottle order. A solo game can belong to any seat.
    pub seats: Vec<usize>,
    ready_flash_left: [Duration; MAX_PLAYERS],
    pub time_left: Duration,
    pub scores: HighScores,
    pub names: [Option<NamePad>; MAX_PLAYERS],
    pub demo: Option<Demo>,
    pub sound: bool,
    has_audio: bool,
    pub choice: usize,
    pub round: Option<Round>,
    pub quit: bool,
    pub sounds: Vec<Sound>,
    repeat: [[u8; 3]; MAX_PLAYERS],
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

impl App {
    pub fn new(config: &Config, has_audio: bool, scores: HighScores) -> Self {
        let mut app = Self {
            screen: Screen::Title,
            enabled: std::array::from_fn(|seat| seat < config.max_players),
            credits: [0; MAX_PLAYERS],
            seats: Vec::new(),
            ready_flash_left: [Duration::ZERO; MAX_PLAYERS],
            time_left: Duration::ZERO,
            scores,
            names: std::array::from_fn(|_| None),
            demo: None,
            sound: config.sound && has_audio,
            has_audio,
            choice: 0,
            round: None,
            quit: false,
            sounds: Vec::new(),
            repeat: [[0; 3]; MAX_PLAYERS],
        };
        app.switch(Screen::Title);
        app
    }

    pub fn enabled_seats(&self) -> impl Iterator<Item = usize> + '_ {
        (0..MAX_PLAYERS).filter(|&seat| self.enabled[seat])
    }

    pub fn set_enabled_seats(&mut self, enabled: [bool; MAX_PLAYERS]) {
        if self.seats.iter().any(|&s| self.enabled[s] && !enabled[s]) {
            self.command(Command::Pause);
        }
        self.enabled = enabled;
    }

    fn switch(&mut self, screen: Screen) {
        self.screen = screen;
        self.choice = 0;
        self.ready_flash_left.fill(Duration::ZERO);
        self.repeat = [[0; 3]; MAX_PLAYERS];
        self.time_left = Duration::from_secs(match screen {
            Screen::Title | Screen::Guide | Screen::Scores => ATTRACT_PAGE_SECONDS,
            Screen::Demo => 12,
            Screen::Ready => 10,
            Screen::Result(_) => 4,
            _ => 0,
        });
        match screen {
            Screen::Title | Screen::Scores => {
                self.round = None;
                self.demo = None;
                self.seats.clear();
                self.names = std::array::from_fn(|_| None);
                if screen == Screen::Scores {
                    self.scores.refresh();
                }
            }
            Screen::Demo => self.demo = Some(Demo::new(seed())),
            _ => {}
        }
    }

    /// A coin credits its seat even when the same press wakes the cabinet.
    pub fn press(&mut self, coin: Option<usize>) -> bool {
        let wake = matches!(self.screen, Screen::Guide | Screen::Demo | Screen::Scores);
        if wake {
            self.switch(Screen::Title);
        } else if self.screen == Screen::Title {
            self.time_left = Duration::from_secs(ATTRACT_PAGE_SECONDS);
        }
        if let Some(seat) = coin {
            self.command(Command::Coin(seat));
        }
        wake || coin.is_some()
    }

    fn start_button(&mut self, seat: usize) {
        match self.screen {
            Screen::Title | Screen::Ready => {
                if self.seats.contains(&seat) {
                    self.ready_flash_left[seat] = READY_FLASH;
                    self.time_left = self.time_left.saturating_sub(Duration::from_secs(1));
                    if self.time_left.is_zero() {
                        self.start_round();
                    }
                } else if self.credits[seat] > 0 {
                    self.credits[seat] -= 1;
                    self.seats.push(seat);
                    self.seats.sort_unstable();
                    if self.screen == Screen::Title {
                        self.switch(Screen::Ready);
                    }
                    self.ready_flash_left[seat] = READY_FLASH;
                    self.sounds.push(Sound::Select);
                }
            }
            Screen::Names => self.name_key(seat, NameKey::Finish),
            _ if !self.seats.contains(&seat) => {}
            Screen::Play => self.command(Command::Pause),
            Screen::Pause => self.command(Command::Resume),
            Screen::Result(_) => self.command(self.options()[self.choice]),
            _ => {}
        }
    }

    pub fn ready_flash(&self, seat: usize) -> f32 {
        self.ready_flash_left[seat].as_secs_f32() / READY_FLASH.as_secs_f32()
    }

    fn start_round(&mut self) {
        self.round = Some(Round::new(self.seats.len(), seed(), 1, 0));
        self.switch(if self.seats.iter().all(|&s| self.enabled[s]) {
            Screen::Play
        } else {
            Screen::Pause
        });
        self.sounds.push(Sound::Start);
    }

    fn round_event(&mut self, event: RoundEvent) {
        match event {
            RoundEvent::CountdownStarted => self.sounds.push(Sound::Start),
            RoundEvent::NextLevel => {
                self.round.as_mut().unwrap().next_level(seed());
                self.repeat.fill([0; 3]);
                self.sounds.push(Sound::Win);
            }
            RoundEvent::Finished(outcome) => {
                self.switch(Screen::Result(outcome));
                self.sounds.push(if outcome == Outcome::SoloLost {
                    Sound::Lose
                } else {
                    Sound::Win
                });
            }
        }
    }

    fn finish_round(&mut self) {
        self.scores.refresh();
        let round = self.round.as_ref().unwrap();
        let results = self
            .seats
            .iter()
            .copied()
            .zip(round.boards.iter().map(|b| b.score));
        let cut = self.scores.qualifiers(results);
        self.names = cut.map(|yes| yes.then(NamePad::default));
        self.switch(if cut.contains(&true) {
            Screen::Names
        } else {
            Screen::Scores
        });
    }

    fn name_key(&mut self, seat: usize, key: NameKey) {
        let Some(pad) = self.names[seat].as_mut().filter(|p| !p.done) else {
            return;
        };
        pad.press(key);
        if pad.done {
            let i = self.seats.iter().position(|&s| s == seat).unwrap();
            self.scores
                .record(&pad.text, self.round.as_ref().unwrap().boards[i].score);
            if self.names.iter().flatten().all(|p| p.done) {
                self.switch(Screen::Scores);
            }
        }
    }

    pub fn options(&self) -> &'static [Command] {
        match self.screen {
            Screen::Pause => &[Command::Resume, Command::Title],
            Screen::Result(_) => &[Command::Finish],
            _ => &[],
        }
    }

    pub fn command(&mut self, cmd: Command) {
        match cmd {
            Command::Coin(s) if self.enabled.get(s) == Some(&true) => {
                self.credits[s] = self.credits[s].saturating_add(1);
                self.sounds.push(Sound::Select);
            }
            Command::Start(s) if self.enabled.get(s) == Some(&true) => self.start_button(s),
            Command::Name(s, key)
                if self.enabled.get(s) == Some(&true) && self.screen == Screen::Names =>
            {
                self.name_key(s, key)
            }
            Command::Finish if matches!(self.screen, Screen::Result(_)) => self.finish_round(),
            Command::Guide if self.screen == Screen::Title => self.switch(Screen::Guide),
            Command::Scores if self.screen == Screen::Title => self.switch(Screen::Scores),
            Command::Pause if self.screen == Screen::Play => self.switch(Screen::Pause),
            Command::Resume if self.screen == Screen::Pause => self.switch(Screen::Play),
            Command::Mute if self.has_audio => {
                self.sound = !self.sound;
                if self.sound {
                    self.sounds.push(Sound::Select);
                }
            }
            Command::Back => match self.screen {
                Screen::Title => self.command(Command::Quit),
                Screen::Play => self.command(Command::Pause),
                Screen::Pause => self.command(Command::Resume),
                Screen::Result(_) => self.command(Command::Finish),
                Screen::Guide | Screen::Scores | Screen::Demo => self.switch(Screen::Title),
                Screen::Ready | Screen::Names => {}
            },
            Command::Title if self.screen != Screen::Ready && self.screen != Screen::Names => {
                self.switch(Screen::Title)
            }
            Command::Quit => {
                self.scores.flush();
                self.quit = true;
            }
            _ => {}
        }
    }

    pub fn navigate(&mut self, nav: Nav) {
        let command = match (self.screen, nav) {
            (_, Nav::Mute) => Command::Mute,
            (_, Nav::Back) => Command::Back,
            (Screen::Title, Nav::Help) => Command::Guide,
            (Screen::Play, Nav::Pause) => Command::Pause,
            (Screen::Pause, Nav::Pause) => Command::Resume,
            (Screen::Pause | Screen::Result(_), Nav::Up | Nav::Down) => {
                let count = self.options().len();
                let step = if nav == Nav::Up { count - 1 } else { 1 };
                self.choice = (self.choice + step) % count;
                self.sounds.push(Sound::Select);
                return;
            }
            (Screen::Pause | Screen::Result(_), Nav::Confirm) => self.options()[self.choice],
            _ => return,
        };
        self.command(command);
    }

    pub fn play(&mut self, seat: usize, action: Action) {
        if self.screen == Screen::Play {
            if let Some(i) = self.seats.iter().position(|&s| s == seat) {
                if let Some(sound) = self.round.as_mut().unwrap().boards[i].action(action) {
                    self.sounds.push(sound);
                }
            }
        }
    }

    // Cabinet deadlines use real elapsed time even if rendering skips simulation ticks.
    pub fn clock(&mut self, elapsed: Duration) {
        if self.screen == Screen::Play {
            if let Some(event) = self.round.as_mut().unwrap().elapse(elapsed) {
                self.round_event(event);
            }
            return;
        }
        if self.screen == Screen::Ready {
            for flash in &mut self.ready_flash_left {
                *flash = flash.saturating_sub(elapsed);
            }
        }
        if self.screen == Screen::Names {
            for seat in 0..self.names.len() {
                if let Some(p) = self.names[seat].as_mut().filter(|p| !p.done) {
                    p.time_left = p.time_left.saturating_sub(elapsed);
                    if p.time_left.is_zero() {
                        self.name_key(seat, NameKey::Finish);
                    }
                }
            }
            return;
        }
        self.time_left = self.time_left.saturating_sub(elapsed);
        if self.time_left.is_zero() {
            match self.screen {
                Screen::Title => self.switch(Screen::Guide),
                Screen::Guide => self.switch(Screen::Demo),
                Screen::Demo => self.switch(Screen::Scores),
                Screen::Scores => self.switch(Screen::Title),
                Screen::Ready => self.start_round(),
                Screen::Result(_) => self.finish_round(),
                _ => {}
            }
        }
    }

    pub fn tick(&mut self, held: &[[bool; 3]; MAX_PLAYERS]) {
        if let Some(demo) = &mut self.demo {
            demo.tick(&mut self.sounds);
            if matches!(demo.board.phase, Phase::Out | Phase::Cleared) {
                self.sounds.push(if demo.board.phase == Phase::Cleared {
                    Sound::Win
                } else {
                    Sound::Lose
                });
                *demo = Demo::new(seed());
            }
            return;
        }
        if self.screen != Screen::Play {
            return;
        }
        let round = self.round.as_mut().unwrap();
        for (&seat, b) in self.seats.iter().zip(&mut round.boards) {
            for (k, action) in [Action::Left, Action::Right, Action::Down]
                .into_iter()
                .enumerate()
            {
                let timer = &mut self.repeat[seat][k];
                if !held[seat][k] || (k < 2 && held[seat][1 - k]) {
                    *timer = 0;
                    continue;
                }
                *timer += 1;
                // The initial movement is delivered by the physical press.
                let delay = if k == 2 { 4 } else { 13 };
                if *timer >= delay {
                    *timer = delay - 4;
                    if let Some(sound) = b.action(action) {
                        self.sounds.push(sound);
                    }
                }
            }
            if let Some(sound) = b.tick() {
                self.sounds.push(sound);
            }
        }
        if let Some(event) = round.progress() {
            self.round_event(event);
        }
    }
}
