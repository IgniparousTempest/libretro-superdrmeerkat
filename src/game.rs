//! Bottle rules. Time advances in 60 Hz ticks, independently of the renderer.
pub const COLS: usize = 8;
pub const ROWS: usize = 16;
const CELLS: usize = COLS * ROWS;
const FLASH_TICKS: u8 = 18;
const SETTLE_TICKS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Red,
    Blue,
    Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    East,
    South,
    West,
    North,
}

impl Direction {
    pub fn offset(self) -> (i32, i32) {
        match self {
            Self::East => (1, 0),
            Self::South => (0, 1),
            Self::West => (-1, 0),
            Self::North => (0, -1),
        }
    }

    fn turn(self, clockwise: bool) -> Self {
        use Direction::*;
        let dirs = [East, South, West, North];
        dirs[(self as usize + if clockwise { 1 } else { 3 }) % 4]
    }

    pub fn opposite(self) -> Self {
        self.turn(true).turn(true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Germ,
    // A link points to the other half. A cut capsule falls as a single cell.
    Pill(Option<Direction>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub color: Color,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capsule {
    pub x: i32,
    pub y: i32,
    pub direction: Direction,
    pub colors: [Color; 2],
}

impl Capsule {
    pub fn halves(self) -> [(i32, i32, Block); 2] {
        let (dx, dy) = self.direction.offset();
        [
            (
                self.x,
                self.y,
                Block {
                    color: self.colors[0],
                    kind: Kind::Pill(Some(self.direction)),
                },
            ),
            (
                self.x + dx,
                self.y + dy,
                Block {
                    color: self.colors[1],
                    kind: Kind::Pill(Some(self.direction.opposite())),
                },
            ),
        ]
    }

    fn shifted(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Falling(Capsule),
    Flash(u8),
    Settling(u8),
    Cleared,
    Out,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Left,
    Right,
    Down,
    Clockwise,
    Counterclockwise,
    Drop,
}

#[derive(Clone, Copy, Debug)]
pub enum Sound {
    Move,
    Turn,
    Land,
    Match(u8),
    Select,
    Start,
    Win,
    Lose,
}

// Seeded separately for pieces and the starting layout. Players consume their
// own copy of each stream, so drop speed cannot affect anyone else's queue.
#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn pick(&mut self, limit: usize) -> usize {
        // SplitMix64 also accepts seed zero.
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut n = self.0;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((n ^ (n >> 31)) % limit as u64) as usize
    }

    fn pair(&mut self) -> [Color; 2] {
        let colors = [Color::Red, Color::Blue, Color::Gold];
        [colors[self.pick(3)], colors[self.pick(3)]]
    }
}

fn cell(x: i32, y: i32) -> Option<usize> {
    if (0..COLS as i32).contains(&x) && (0..ROWS as i32).contains(&y) {
        Some(y as usize * COLS + x as usize)
    } else {
        None
    }
}

fn neighbour(i: usize, dir: Direction) -> Option<usize> {
    let (dx, dy) = dir.offset();
    cell((i % COLS) as i32 + dx, (i / COLS) as i32 + dy)
}

#[derive(Clone)]
pub struct Board {
    pub cells: [Option<Block>; CELLS],
    pub marked: [bool; CELLS],
    pub phase: Phase,
    pub next: [Color; 2],
    pub score: u32,
    pub level: u32,
    pub chain: u8,
    rng: Rng,
    fall_ticks: u32,
}

impl Board {
    pub fn new(seed: u64, level: u32, score: u32) -> Self {
        let mut rng = Rng(seed ^ 0xd0c7_0a11);
        let mut board = Self {
            cells: [None; CELLS],
            marked: [false; CELLS],
            phase: Phase::Settling(0),
            next: rng.pair(),
            score,
            level,
            chain: 0,
            rng,
            fall_ticks: 0,
        };
        let mut layout = Rng(seed);
        let first_row = 7usize.saturating_sub((level / 3).min(3) as usize);
        let mut places: Vec<_> = (first_row * COLS..CELLS).collect();
        let mut remaining = (8 + level.min(20) * 2) as usize;
        let palette = [Color::Red, Color::Blue, Color::Gold];
        // Cycle colors for a balanced bottle, shuffle positions, and reject any
        // placement that would start the round with a completed run.
        let mut color_index = layout.pick(3);
        while remaining > 0 && !places.is_empty() {
            let choice = layout.pick(places.len());
            let i = places.swap_remove(choice);
            board.cells[i] = Some(Block {
                color: palette[color_index],
                kind: Kind::Germ,
            });
            if board.find_matches()[i] {
                board.cells[i] = None;
            } else {
                remaining -= 1;
                color_index = (color_index + 1) % 3;
            }
        }
        board.spawn();
        board
    }

    pub fn germs(&self) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|b| b.kind == Kind::Germ)
            .count()
    }

    fn fits(&self, piece: Capsule) -> bool {
        piece
            .halves()
            .iter()
            .all(|&(x, y, _)| cell(x, y).is_some_and(|i| self.cells[i].is_none()))
    }

    fn spawn(&mut self) {
        let piece = Capsule {
            x: 3,
            y: 0,
            direction: Direction::East,
            colors: self.next,
        };
        self.next = self.rng.pair();
        self.fall_ticks = 0;
        self.chain = 0;
        self.phase = if self.fits(piece) {
            Phase::Falling(piece)
        } else {
            Phase::Out
        };
    }

    pub fn landing(&self, mut piece: Capsule) -> Capsule {
        while self.fits(piece.shifted(0, 1)) {
            piece.y += 1;
        }
        piece
    }

    pub fn action(&mut self, action: Action) -> Option<Sound> {
        let Phase::Falling(piece) = self.phase else {
            return None;
        };
        let (candidate, sound) = match action {
            Action::Left => (piece.shifted(-1, 0), Sound::Move),
            Action::Right => (piece.shifted(1, 0), Sound::Move),
            Action::Clockwise | Action::Counterclockwise => {
                let turned = Capsule {
                    direction: piece.direction.turn(action == Action::Clockwise),
                    ..piece
                };
                // Allow turning beside a wall, at the mouth and against the floor.
                let rotated = [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .map(|&(dx, dy)| turned.shifted(dx, dy))
                    .find(|&p| self.fits(p))?;
                (rotated, Sound::Turn)
            }
            Action::Down => return self.step_down(true),
            Action::Drop => {
                let landed = self.landing(piece);
                self.score = self.score.saturating_add((landed.y - piece.y) as u32 * 2);
                return Some(self.lock(landed));
            }
        };
        if self.fits(candidate) {
            self.phase = Phase::Falling(candidate);
            Some(sound)
        } else {
            None
        }
    }

    fn step_down(&mut self, soft: bool) -> Option<Sound> {
        let Phase::Falling(piece) = self.phase else {
            return None;
        };
        self.fall_ticks = 0;
        let next = piece.shifted(0, 1);
        if self.fits(next) {
            self.phase = Phase::Falling(next);
            self.score = self.score.saturating_add(u32::from(soft));
            None
        } else {
            Some(self.lock(piece))
        }
    }

    fn lock(&mut self, piece: Capsule) -> Sound {
        for (x, y, block) in piece.halves() {
            self.cells[cell(x, y).expect("locked capsule fits")] = Some(block);
        }
        self.check_runs().unwrap_or(Sound::Land)
    }

    // Mark every run against the same snapshot before clearing intersections.
    fn find_matches(&self) -> [bool; CELLS] {
        let mut marked = [false; CELLS];
        for (i, block) in self.cells.iter().enumerate() {
            let Some(block) = block else { continue };
            for dir in [Direction::East, Direction::South] {
                let same = |j: usize| self.cells[j].is_some_and(|b| b.color == block.color);
                if neighbour(i, dir.opposite()).is_some_and(same) {
                    continue;
                }
                let mut end = i;
                let mut length = 1;
                while let Some(j) = neighbour(end, dir).filter(|&j| same(j)) {
                    end = j;
                    length += 1;
                }
                if length >= 4 {
                    let step = if dir == Direction::East { 1 } else { COLS };
                    for j in (i..=end).step_by(step) {
                        marked[j] = true;
                    }
                }
            }
        }
        marked
    }

    fn check_runs(&mut self) -> Option<Sound> {
        self.marked = self.find_matches();
        if self.marked.iter().any(|&m| m) {
            self.chain = self.chain.saturating_add(1);
            self.phase = Phase::Flash(FLASH_TICKS);
            Some(Sound::Match(self.chain))
        } else {
            self.spawn();
            None
        }
    }

    fn clear_marked(&mut self) {
        let mut points = 0;
        for i in 0..CELLS {
            if self.marked[i] {
                let block = self.cells[i].take().expect("only occupied cells match");
                points += if block.kind == Kind::Germ { 100 } else { 10 };
                if let Kind::Pill(Some(dir)) = block.kind {
                    if let Some(partner) = neighbour(i, dir).and_then(|j| self.cells[j].as_mut()) {
                        partner.kind = Kind::Pill(None);
                    }
                }
            }
        }
        self.marked.fill(false);
        self.score = self.score.saturating_add(points * u32::from(self.chain));
        self.phase = if self.germs() == 0 {
            Phase::Cleared
        } else {
            Phase::Settling(SETTLE_TICKS)
        };
    }

    fn settle_one_row(&mut self) -> bool {
        let mut moved = false;
        // Visit the bottom half of a vertical pair, or the left half of a
        // horizontal pair. Both destinations must be free to move the pair.
        for i in (0..CELLS - COLS).rev() {
            let Some(Block {
                kind: Kind::Pill(link),
                ..
            }) = self.cells[i]
            else {
                continue;
            };
            if matches!(link, Some(Direction::South | Direction::West)) {
                continue;
            }
            let partner = link.and_then(|dir| neighbour(i, dir));
            let below = i + COLS;
            if self.cells[below].is_some() {
                continue;
            }
            if let Some(j) = partner {
                let other_below = j + COLS;
                if other_below != i && self.cells[other_below].is_some() {
                    continue;
                }
                let a = self.cells[i].take();
                let b = self.cells[j].take();
                self.cells[below] = a;
                self.cells[other_below] = b;
            } else {
                self.cells[below] = self.cells[i].take();
            }
            moved = true;
        }
        moved
    }

    pub fn tick(&mut self) -> Option<Sound> {
        match self.phase {
            Phase::Falling(_) => {
                self.fall_ticks += 1;
                if self.fall_ticks >= 47 - self.level.min(12) * 3 {
                    return self.step_down(false);
                }
            }
            Phase::Flash(1) => self.clear_marked(),
            Phase::Flash(n) => self.phase = Phase::Flash(n - 1),
            Phase::Settling(1) => {
                if self.settle_one_row() {
                    self.phase = Phase::Settling(SETTLE_TICKS);
                } else {
                    return self.check_runs();
                }
            }
            Phase::Settling(n) => self.phase = Phase::Settling(n - 1),
            Phase::Out | Phase::Cleared => {}
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    SoloClear,
    SoloLost,
    Winner(usize),
    Draw,
}

pub struct Round {
    pub boards: Vec<Board>,
}

impl Round {
    pub fn new(players: usize, seed: u64, level: u32, score: u32) -> Self {
        Self {
            boards: vec![Board::new(seed, level, score); players],
        }
    }

    pub fn outcome(&self) -> Option<Outcome> {
        if self.boards.len() == 1 {
            return match self.boards[0].phase {
                Phase::Cleared => Some(Outcome::SoloClear),
                Phase::Out => Some(Outcome::SoloLost),
                _ => None,
            };
        }
        let mut finished = self
            .boards
            .iter()
            .enumerate()
            .filter(|(_, b)| b.phase == Phase::Cleared);
        if let Some((first, _)) = finished.next() {
            return Some(if finished.next().is_some() {
                Outcome::Draw
            } else {
                Outcome::Winner(first)
            });
        }
        let mut alive = self
            .boards
            .iter()
            .enumerate()
            .filter(|(_, b)| b.phase != Phase::Out);
        let first = alive.next();
        match (first, alive.next()) {
            (None, _) => Some(Outcome::Draw),
            (Some((only, _)), None) => Some(Outcome::Winner(only)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Color::*;
    use Direction::*;

    fn empty() -> Board {
        let mut b = Board::new(0, 1, 0);
        b.cells.fill(None);
        b
    }

    fn put(b: &mut Board, x: i32, y: i32, color: Color, kind: Kind) {
        b.cells[cell(x, y).unwrap()] = Some(Block { color, kind });
    }

    #[test]
    fn every_player_gets_the_same_playable_bottle_and_sequence() {
        for level in [1, 8, 20, 100] {
            for seed in 0..24 {
                let mut r = Round::new(4, seed, level, 0);
                let initial = r.boards[0].cells;
                let next = r.boards[0].next;
                for b in &mut r.boards {
                    assert_eq!(initial, b.cells);
                    assert_eq!(next, b.next);
                    assert_eq!(b.germs(), (8 + 2 * level.min(20)) as usize);
                    assert!(!b.find_matches().contains(&true));
                    assert!(matches!(b.phase, Phase::Falling(_)));
                }
                for _ in 0..12 {
                    r.boards[0].spawn();
                    r.boards[3].spawn();
                    assert_eq!(r.boards[0].next, r.boards[3].next);
                }
            }
        }
    }

    #[test]
    fn cross_is_scored_once_and_cut_partner_falls_into_a_chain() {
        let mut b = empty();
        for x in 0..4 {
            put(&mut b, x, 13, Red, Kind::Germ);
        }
        for y in 10..13 {
            put(&mut b, 2, y, Red, Kind::Germ);
        }
        put(&mut b, 3, 13, Red, Kind::Pill(Some(East)));
        put(&mut b, 4, 13, Blue, Kind::Pill(Some(West)));
        for x in 1..4 {
            put(&mut b, x, 15, Blue, Kind::Germ);
        }
        b.check_runs();
        assert_eq!(b.marked.iter().filter(|&&m| m).count(), 7);
        for _ in 0..FLASH_TICKS {
            b.tick();
        }
        assert_eq!(b.score, 610);
        assert_eq!(
            b.cells[cell(4, 13).unwrap()].unwrap().kind,
            Kind::Pill(None)
        );
        for _ in 0..100 {
            b.tick();
            if b.phase == Phase::Cleared {
                break;
            }
        }
        assert_eq!(b.phase, Phase::Cleared);
        assert_eq!(b.chain, 2);
        assert_eq!(b.score, 610 + 310 * 2);
    }

    #[test]
    fn rigid_pairs_remain_supported_at_either_end_and_do_not_overwrite() {
        let mut b = empty();
        put(&mut b, 5, 11, Blue, Kind::Pill(Some(East)));
        put(&mut b, 6, 11, Red, Kind::Pill(Some(West)));
        put(&mut b, 6, 13, Gold, Kind::Germ);
        put(&mut b, 1, 13, Red, Kind::Pill(Some(South)));
        put(&mut b, 1, 14, Blue, Kind::Pill(Some(North)));
        assert!(b.settle_one_row());
        assert!(!b.settle_one_row());
        assert_eq!(b.cells[cell(5, 12).unwrap()].unwrap().color, Blue);
        assert_eq!(b.cells[cell(1, 15).unwrap()].unwrap().color, Blue);
        assert_eq!(b.cells[cell(1, 14).unwrap()].unwrap().color, Red);
        assert_eq!(b.cells.iter().flatten().count(), 5);
    }

    #[test]
    fn rotation_kicks_at_walls_and_a_blocked_mouth_ends_play() {
        let mut b = empty();
        let p = Capsule {
            x: 7,
            y: 3,
            direction: North,
            colors: [Gold, Red],
        };
        b.phase = Phase::Falling(p);
        b.action(Action::Clockwise);
        let Phase::Falling(p) = b.phase else { panic!() };
        assert_eq!((p.x, p.direction), (6, East));
        b.action(Action::Right);
        assert_eq!(b.phase, Phase::Falling(p));
        put(&mut b, 3, 0, Gold, Kind::Pill(None));
        b.action(Action::Drop);
        assert_eq!(b.phase, Phase::Out);
    }

    #[test]
    fn race_handles_clears_last_survivor_and_simultaneous_finishes() {
        let mut r = Round::new(3, 10, 1, 0);
        assert_eq!(r.outcome(), None);
        r.boards[1].phase = Phase::Out;
        assert_eq!(r.outcome(), None);
        r.boards[0].phase = Phase::Out;
        assert_eq!(r.outcome(), Some(Outcome::Winner(2)));
        r.boards[2].phase = Phase::Out;
        assert_eq!(r.outcome(), Some(Outcome::Draw));
        r.boards[0].phase = Phase::Cleared;
        assert_eq!(r.outcome(), Some(Outcome::Winner(0)));
        r.boards[2].phase = Phase::Cleared;
        assert_eq!(r.outcome(), Some(Outcome::Draw));
    }
}
