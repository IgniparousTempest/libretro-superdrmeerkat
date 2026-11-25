use crate::game::{Action, Board, Kind, Phase, Sound, COLS, ROWS};
use std::collections::VecDeque;

pub struct Demo {
    pub board: Board,
    moves: VecDeque<Action>,
    delay: u8,
}

impl Demo {
    pub fn new(seed: u64) -> Self {
        let level = 1 + (seed % 3) as u32;
        let mut board = Board::new(seed, level, 0);
        // Start mid-game with several actual placements, including any resulting clears.
        for _ in 0..4 + (seed >> 8) % 4 {
            let mut next = board.clone();
            for action in plan(&next) {
                next.action(action);
            }
            settle(&mut next);
            if !matches!(next.phase, Phase::Falling(_)) {
                break;
            }
            board = next;
        }
        Self {
            board,
            moves: VecDeque::new(),
            delay: 12,
        }
    }

    pub fn tick(&mut self, sounds: &mut Vec<Sound>) {
        sounds.extend(self.board.tick());
        let Phase::Falling(piece) = self.board.phase else {
            return;
        };
        if self.delay > 0 {
            self.delay -= 1;
            return;
        }
        if self.moves.is_empty() {
            self.moves = plan(&self.board);
        }
        if self.moves.front() == Some(&Action::Drop) && self.board.landing(piece).y > piece.y + 3 {
            sounds.extend(self.board.action(Action::Down));
            self.delay = 3;
        } else if let Some(action) = self.moves.pop_front() {
            sounds.extend(self.board.action(action));
            self.delay = if action == Action::Drop { 12 } else { 5 };
        }
    }
}

fn settle(board: &mut Board) {
    while matches!(board.phase, Phase::Flash(_) | Phase::Settling(_)) {
        board.tick();
    }
}

fn plan(board: &Board) -> VecDeque<Action> {
    use Action::*;
    let mut best = VecDeque::from([Drop]);
    let mut best_value = i32::MIN;
    // Test only placements reachable by turning at the top and sliding across.
    for turns in [
        &[][..],
        &[Clockwise],
        &[Counterclockwise],
        &[Clockwise, Clockwise],
    ] {
        let mut turned = board.clone();
        if turns.iter().any(|&action| turned.action(action).is_none()) {
            continue;
        }
        // The left pass includes the drop with no horizontal movement.
        for (slide, first_step) in [(Left, 0), (Right, 1)] {
            let mut placement = turned.clone();
            for steps in first_step..COLS {
                if steps > 0 && placement.action(slide).is_none() {
                    break;
                }
                let mut result = placement.clone();
                result.action(Drop);
                settle(&mut result);
                let value = evaluate(&result, board) - (turns.len() + steps) as i32;
                if value > best_value {
                    best_value = value;
                    best = turns
                        .iter()
                        .copied()
                        .chain(std::iter::repeat_n(slide, steps))
                        .chain([Drop])
                        .collect();
                }
            }
        }
    }
    best
}

fn evaluate(after: &Board, before: &Board) -> i32 {
    match after.phase {
        Phase::Cleared => return 1_000_000,
        Phase::Out => return -1_000_000,
        _ => {}
    }
    let mut value = (before.germs() - after.germs()) as i32 * 400
        + after.score.saturating_sub(before.score) as i32;
    for x in 0..COLS {
        let mut covered = false;
        for y in 0..ROWS {
            let i = y * COLS + x;
            if let Some(block) = after.cells[i] {
                if !covered {
                    let height = (ROWS - y) as i32;
                    value -= height * height;
                    covered = true;
                }
                // Favor same-color neighbors, especially a capsule touching a germ.
                for adjacent in [
                    (x + 1 < COLS).then_some(i + 1),
                    (y + 1 < ROWS).then_some(i + COLS),
                ]
                .into_iter()
                .flatten()
                {
                    if let Some(other) = after.cells[adjacent].filter(|b| b.color == block.color) {
                        value += if block.kind == Kind::Germ || other.kind == Kind::Germ {
                            38
                        } else {
                            17
                        };
                    }
                }
            } else if covered {
                value -= 4;
            }
        }
    }
    value
}
