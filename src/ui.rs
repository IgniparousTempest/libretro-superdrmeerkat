use crate::{
    app::{App, Command, Screen},
    game::{Block, Board, Color, Kind, Outcome, Phase, CLEAR_BONUS, COLS, ROWS},
    render::{rect, rgb, Canvas, Ink, Rect, BODY, DISPLAY, WHITE},
    scores::{NameKey, NamePad, DELETE, DONE, KEY_COLUMNS, NAME_LENGTH},
};

const NIGHT: Ink = rgb(19, 49, 54);
const DEEP: Ink = rgb(12, 36, 42);
const WELL: Ink = rgb(16, 39, 47);
const LINE: Ink = rgb(54, 86, 85);
const SOFT: Ink = rgb(168, 195, 179);
const MINT: Ink = rgb(165, 221, 188);
const PAPER: Ink = rgb(255, 242, 212);
const RED: Ink = rgb(248, 133, 105);
const GOLD: Ink = rgb(255, 204, 111);
const BLUE: Ink = rgb(132, 200, 221);
const SEATS: [Ink; 4] = [MINT, RED, BLUE, GOLD];
const CARD_WIDTH: f32 = 146.0;
const CARD_GAP: f32 = 10.0;
const FOOTER_HEIGHT: f32 = 51.0;
const PLAY_CONTROLS: [(&str, &str); 5] = [
    ("joystick", "Move"),
    ("button-a", "Turn right"),
    ("button-b", "Turn left"),
    ("button-c", "Soft drop"),
    ("button-d", "Hard drop"),
];

pub struct Ui {
    hits: Vec<(Rect, Command)>,
    pointer: Option<(f32, f32)>,
    screen: Screen,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            hits: Vec::new(),
            pointer: None,
            screen: Screen::Title,
        }
    }
}

impl Ui {
    pub fn click(&self, screen: Screen, point: (f32, f32)) -> Option<Command> {
        if self.screen != screen {
            return None;
        }
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(point))
            .map(|(_, cmd)| *cmd)
    }
    fn hit(&mut self, c: &Canvas, r: Rect, cmd: Command) -> bool {
        let world = c.world(r);
        self.hits.push((world, cmd));
        self.pointer.is_some_and(|p| world.contains(p))
    }
    pub fn draw(&mut self, c: &mut Canvas, app: &App, pointer: Option<(f32, f32)>, time: f32) {
        self.hits.clear();
        self.pointer = pointer;
        self.screen = app.screen;
        background(c);
        let margin = ((c.w - 1490.0) * 0.5).max(if c.w < 850.0 { 24.0 } else { 44.0 });
        let footer = rect(
            margin,
            c.h - FOOTER_HEIGHT,
            c.w - 2.0 * margin,
            FOOTER_HEIGHT,
        );
        let body = rect(margin, 83.0, footer.w, footer.y - 90.0);
        match app.screen {
            Screen::Title | Screen::Ready => self.title(c, body, app, time),
            Screen::Guide => c.within(body.inset(8.0), 724.0, 520.0, guide),
            Screen::Scores => score_screen(c, body, app),
            _ => self.play_screen(c, body, app, time),
        }
        if matches!(
            app.screen,
            Screen::Ready | Screen::Pause | Screen::Result(_)
        ) {
            self.hits.clear();
            c.fill(rect(0.0, 0.0, c.w, c.h), DEEP.alpha(214));
            let room = body.inset(10.0);
            let (w, h) = match app.screen {
                Screen::Ready => (
                    (seat_row_width(app.enabled_seats().count()) + 64.0).max(440.0),
                    224.0,
                ),
                Screen::Result(Outcome::Draw) => (540.0, 190.0),
                _ if app.options().len() > 1 => (540.0, 320.0),
                _ => (540.0, 222.0),
            };
            let scale = (room.w / w).min(room.h / h).min(1.12);
            let panel = rect(
                room.x + (room.w - w * scale) * 0.5,
                room.y + (room.h - h * scale) * 0.5,
                w * scale,
                h * scale,
            );
            c.within(panel, w, h, |c| {
                paper_panel(c, w, h);
                if app.screen == Screen::Ready {
                    self.ready(c, app, w);
                } else {
                    self.round_dialog(c, app);
                }
            });
        }
        header(c, margin);
        self.footer(c, app, footer);
    }

    fn title(&mut self, c: &mut Canvas, area: Rect, app: &App, t: f32) {
        if area.w / area.h > 1.26 {
            c.within(area, 1192.0, 552.0, |c| {
                c.art("wordmark", rect(-3.0, 36.0, 643.0, 221.0));
                self.title_prompt(c, rect(54.0, 279.0, 526.0, 182.0), app, t);
                scene(c, rect(631.0, 9.0, 558.0, 533.0), t, true);
            });
        } else {
            c.within(area, 610.0, 846.0, |c| {
                c.art("wordmark", rect(13.0, 6.0, 585.0, 190.0));
                scene(c, rect(65.0, 186.0, 480.0, 412.0), t, true);
                self.title_prompt(c, rect(42.0, 617.0, 526.0, 182.0), app, t);
            });
        }
    }

    fn title_prompt(&mut self, c: &mut Canvas, area: Rect, app: &App, t: f32) {
        c.within(area, 500.0, 172.0, |c| {
            for (i, sprite) in ["germ-red", "germ-blue", "germ-gold"].iter().enumerate() {
                let bob = (t * 2.0 + i as f32 * 2.0).sin() * 4.0;
                c.art(
                    sprite,
                    rect(53.0 + i as f32 * 160.0, 10.0 + bob, 74.0, 74.0),
                );
            }
            for (x, left, right) in [
                (144.0, "end-gold", "end-blue"),
                (304.0, "end-red", "end-gold"),
            ] {
                c.tile(left, rect(x, 34.0, 26.0, 26.0), 0, WHITE);
                c.tile(right, rect(x + 26.0, 34.0, 26.0, 26.0), 2, WHITE);
            }
            if app.screen == Screen::Title {
                let (prompt, command) =
                    if let Some(seat) = app.enabled_seats().find(|&s| app.credits[s] > 0) {
                        ("Press start to begin", Some(Command::Start(seat)))
                    } else if let Some(seat) = app.enabled_seats().next() {
                        ("Insert credits", Some(Command::Coin(seat)))
                    } else {
                        ("Enable a controller port", None)
                    };
                let width = c.text_width(DISPLAY, 46.0, prompt) + 24.0;
                let hover = command.is_some_and(|cmd| {
                    self.hit(c, rect((500.0 - width) * 0.5, 101.0, width, 65.0), cmd)
                });
                c.center_text(
                    DISPLAY,
                    46.0,
                    250.0,
                    148.0,
                    prompt,
                    if hover { PAPER } else { GOLD },
                );
            }
        });
    }

    fn ready(&mut self, c: &mut Canvas, app: &App, width: f32) {
        c.text(DISPLAY, 58.0, 31.0, 81.0, "GET READY!", DEEP);
        let timer = rect(width - 131.0, 18.0, 99.0, 78.0);
        let center = timer.x + timer.w * 0.5;
        c.round(timer, 9.0, NIGHT);
        c.center_text(DISPLAY, 14.0, center, timer.y + 20.0, "STARTING IN", MINT);
        c.center_text(
            DISPLAY,
            53.0,
            center,
            timer.y + 66.0,
            &format!("{:02}", seconds_left(app.time_left)),
            PAPER,
        );

        let first = (width - seat_row_width(app.enabled_seats().count())) * 0.5;
        for (i, seat) in app.enabled_seats().enumerate() {
            let accent = SEATS[seat];
            let left = first + i as f32 * (CARD_WIDTH + CARD_GAP);
            let joined = app.seats.contains(&seat);
            let credits = app.credits[seat];
            let frame = rect(left, 115.0, CARD_WIDTH, 82.0);
            c.round(frame, 10.0, if joined { accent } else { LINE });
            c.round(frame.inset(1.5), 9.0, DEEP);
            c.text(
                DISPLAY,
                22.0,
                left + 11.0,
                frame.y + 28.0,
                &format!("P{}", seat + 1),
                accent,
            );
            let credit = if credits == 1 { "CREDIT" } else { "CREDITS" };
            let balance = format!("{credits} {credit}");
            let size = 17.0 * (92.0 / c.text_width(DISPLAY, 17.0, &balance)).min(1.0);
            c.right_text(
                DISPLAY,
                size,
                left + CARD_WIDTH - 10.0,
                frame.y + 27.0,
                &balance,
                PAPER,
            );
            let (label, command) = if joined {
                ("Ready!", Command::Start(seat))
            } else if credits > 0 {
                ("Press start", Command::Start(seat))
            } else {
                ("Insert credits", Command::Coin(seat))
            };
            let bar = rect(left + 5.0, frame.y + 38.0, CARD_WIDTH - 10.0, 39.0);
            let hover = self.hit(c, bar, command);
            c.round(bar, 6.0, accent);
            let light = app.ready_flash(seat).max(if hover { 0.16 } else { 0.0 });
            if light > 0.0 {
                c.round(bar, 6.0, WHITE.alpha((light * 255.0) as u8));
            }
            c.center_text(
                DISPLAY,
                24.0,
                bar.x + bar.w * 0.5,
                bar.y + 27.0,
                label,
                DEEP,
            );
        }
    }

    fn round_dialog(&mut self, c: &mut Canvas, app: &App) {
        let boards = &app.round.as_ref().unwrap().boards;
        let (heading, detail) = match app.screen {
            Screen::Pause => {
                let detail = if boards.len() == 1 {
                    format!(
                        "Level {:02}    /    Germs left {:02}",
                        boards[0].level,
                        boards[0].germs()
                    )
                } else {
                    let alive = boards.iter().filter(|b| b.phase != Phase::Out).count();
                    format!("Still playing    {alive} / {}", boards.len())
                };
                ("PAUSED".into(), Some(detail))
            }
            Screen::Result(Outcome::Winner(i)) => (
                format!("PLAYER {} WINS!", app.seats[i] + 1),
                Some(format!("Final score  {:06}", boards[i].score)),
            ),
            Screen::Result(Outcome::Draw) => ("DRAW GAME".into(), None),
            Screen::Result(Outcome::SoloLost) => {
                let b = &boards[0];
                (
                    "BOTTLE FULL".into(),
                    Some(format!("Level {:02}    /    Score {:06}", b.level, b.score)),
                )
            }
            _ => return,
        };
        c.text(DISPLAY, 57.0, 33.0, 77.0, &heading, DEEP);
        let button_top = if let Some(detail) = detail {
            c.text(BODY, 20.0, 36.0, 110.0, &detail, NIGHT);
            138.0
        } else {
            106.0
        };
        let options = app.options();
        for (i, &command) in options.iter().enumerate() {
            let label = match command {
                Command::Resume => "RESUME GAME",
                Command::Title => "ABANDON GAME",
                Command::Finish => "CONTINUE",
                _ => "",
            };
            let y = button_top + i as f32 * 58.0;
            self.dialog_button(
                c,
                rect(35.0, y, 470.0, 48.0),
                label,
                command,
                app.choice == i,
            );
        }
        if options.len() > 1 {
            control_strip(
                c,
                rect(35.0, 260.0, 470.0, 34.0),
                &[("joystick", "Choose an option"), ("button-a", "Confirm")],
                NIGHT,
            );
        }
    }
    fn dialog_button(
        &mut self,
        c: &mut Canvas,
        r: Rect,
        label: &str,
        cmd: Command,
        selected: bool,
    ) {
        let hover = self.hit(c, r, cmd);
        c.round(
            r,
            7.0,
            if hover {
                GOLD
            } else if selected {
                RED
            } else {
                rgb(221, 225, 201)
            },
        );
        c.text(
            DISPLAY,
            23.0,
            r.x + 18.0,
            r.y + r.h * 0.5 + 8.0,
            label,
            DEEP,
        );
        if selected {
            c.art("button-a", rect(r.x + r.w - 48.0, r.y + 5.0, 38.0, 38.0));
        }
    }
}

fn seconds_left(time: std::time::Duration) -> u64 {
    time.as_secs() + u64::from(time.subsec_nanos() != 0)
}

fn seat_row_width(count: usize) -> f32 {
    count as f32 * CARD_WIDTH + count.saturating_sub(1) as f32 * CARD_GAP
}

fn score_screen(c: &mut Canvas, body: Rect, app: &App) {
    fn chart(c: &mut Canvas, app: &App) {
        paper_panel(c, 608.0, 542.0);
        c.art("clinic-mark", rect(27.0, 22.0, 47.0, 47.0));
        c.text(DISPLAY, 47.0, 88.0, 62.0, "HIGH SCORES", DEEP);
        let muted = rgb(68, 100, 88);
        c.text(DISPLAY, 16.0, 35.0, 103.0, "RANK", muted);
        c.text(DISPLAY, 16.0, 108.0, 103.0, "NAME", muted);
        c.right_text(DISPLAY, 16.0, 567.0, 103.0, "SCORE", muted);
        c.fill(rect(30.0, 113.0, 548.0, 1.5), muted);
        for (i, entry) in app.scores.entries().iter().enumerate() {
            let top = 123.0 + i as f32 * 39.0;
            if i % 2 == 0 {
                c.round(
                    rect(24.0, top, 560.0, 36.0),
                    5.0,
                    if i == 0 { GOLD } else { rgb(234, 231, 206) },
                );
            }
            let baseline = top + 26.0;
            c.text(
                DISPLAY,
                22.0,
                38.0,
                baseline,
                &format!("{:02}", i + 1),
                muted,
            );
            c.text(BODY, 23.0, 108.0, baseline, &entry.name, NIGHT);
            c.right_text(
                DISPLAY,
                25.0,
                566.0,
                baseline,
                &format!("{:06}", entry.score),
                DEEP,
            );
        }
        if app.scores.storage_error {
            c.center_text(
                BODY,
                15.0,
                304.0,
                529.0,
                "Score file unavailable. Showing this session's table.",
                NIGHT,
            );
        }
    }
    if body.w > body.h * 1.55 {
        c.within(body.inset(8.0), 1070.0, 542.0, |c| {
            c.within(rect(24.0, 0.0, 608.0, 542.0), 608.0, 542.0, |c| {
                chart(c, app)
            });
            c.art("savanna", rect(648.0, 82.0, 387.0, 387.0));
            c.ellipse(rect(726.0, 448.0, 244.0, 16.0), DEEP.alpha(90));
            c.art("secretary", rect(694.0, 60.0, 278.0, 407.0));
        });
    } else {
        c.within(body.inset(8.0), 608.0, 542.0, |c| chart(c, app));
    }
}

fn header(c: &mut Canvas, margin: f32) {
    c.fill(rect(0.0, 0.0, c.w, 77.0), NIGHT);
    c.art("wordmark", rect((c.w - 206.0) * 0.5, 3.0, 206.0, 69.0));
    c.fill(rect(margin, 75.0, c.w - 2.0 * margin, 1.0), LINE);
}

fn control_strip(c: &mut Canvas, area: Rect, items: &[(&str, &str)], ink: Ink) {
    const SIZE: f32 = 32.0;
    const FONT: f32 = 18.0;
    let advance = |c: &Canvas, label: &str| SIZE + 5.0 + c.text_width(BODY, FONT, label) + 18.0;
    let width = items
        .iter()
        .map(|(_, label)| advance(c, label))
        .sum::<f32>()
        - 18.0;
    c.within(area, width, SIZE, |c| {
        let mut x = 0.0;
        for &(sprite, label) in items {
            c.art(sprite, rect(x, 0.0, SIZE, SIZE));
            c.text(BODY, FONT, x + SIZE + 5.0, 22.0, label, ink);
            x += advance(c, label);
        }
    });
}

impl Ui {
    fn credits(&mut self, c: &mut Canvas, area: Rect, app: &App) {
        let amounts: Vec<_> = app
            .enabled_seats()
            .map(|seat| (seat, app.credits[seat].to_string()))
            .collect();
        if amounts.is_empty() {
            return;
        }
        let advance = |c: &Canvas, amount: &str| 45.0 + c.text_width(DISPLAY, 22.0, amount);
        let width = 67.0 + amounts.iter().map(|(_, n)| advance(c, n)).sum::<f32>() - 20.0;
        let mut fitted = area.fit(width, 32.0);
        fitted.x = area.x;
        c.within(fitted, width, 32.0, |c| {
            c.text(DISPLAY, 18.0, 0.0, 22.0, "CREDITS", SOFT);
            let mut x = 67.0;
            let interactive = matches!(app.screen, Screen::Title | Screen::Ready);
            for (seat, amount) in amounts {
                let seat_hover = interactive
                    && self.hit(c, rect(x - 6.0, 0.0, 29.0, 32.0), Command::Start(seat));
                let coin_hover = interactive
                    && self.hit(
                        c,
                        rect(
                            x + 23.0,
                            0.0,
                            c.text_width(DISPLAY, 22.0, &amount) + 9.0,
                            32.0,
                        ),
                        Command::Coin(seat),
                    );
                c.text(
                    DISPLAY,
                    17.0,
                    x,
                    22.0,
                    &format!("P{}", seat + 1),
                    if seat_hover { PAPER } else { SEATS[seat] },
                );
                c.text(
                    DISPLAY,
                    22.0,
                    x + 25.0,
                    23.0,
                    &amount,
                    if coin_hover { GOLD } else { PAPER },
                );
                x += advance(c, &amount);
            }
        });
    }

    fn footer(&mut self, c: &mut Canvas, app: &App, area: Rect) {
        c.fill(rect(0.0, area.y, c.w, area.h), DEEP);
        c.fill(rect(area.x, area.y, area.w, 1.0), LINE);
        let byline = "Courtney Pitcher. 2026";
        let row_y = area.y + 9.0;
        c.right_text(BODY, 18.0, area.x + area.w, row_y + 22.0, byline, SOFT);
        let credits_w = area.w - c.text_width(BODY, 18.0, byline) - 16.0;
        self.credits(c, rect(area.x, row_y, credits_w, 32.0), app);
    }
}

fn guide(c: &mut Canvas) {
    paper_panel(c, 724.0, 520.0);
    let rule = rgb(198, 204, 180);
    c.art("clinic-mark", rect(30.0, 23.0, 45.0, 45.0));
    c.text(DISPLAY, 43.0, 86.0, 61.0, "HOW TO PLAY", DEEP);
    c.fill(rect(34.0, 80.0, 656.0, 2.0), rule);

    c.round(rect(34.0, 103.0, 173.0, 65.0), 9.0, WELL);
    for (i, (tile, turn)) in [
        ("germ-red", 0),
        ("end-red", 0),
        ("end-red", 2),
        ("germ-red", 0),
    ]
    .into_iter()
    .enumerate()
    {
        c.tile(
            tile,
            rect(41.0 + 39.5 * i as f32, 115.0, 39.5, 39.5),
            turn,
            WHITE,
        );
    }
    c.text(DISPLAY, 29.0, 228.0, 121.0, "CLEAR EVERY GERM", DEEP);
    c.text(
        BODY,
        19.0,
        228.0,
        148.0,
        "Match four or more of one color across or down.",
        NIGHT,
    );
    c.text(
        BODY,
        18.0,
        228.0,
        172.0,
        "Loose capsule halves can fall into another match.",
        NIGHT,
    );

    c.round(rect(34.0, 198.0, 656.0, 94.0), 9.0, rgb(220, 229, 202));
    c.art("joystick", rect(47.0, 199.0, 92.0, 88.0));
    c.text(DISPLAY, 28.0, 164.0, 238.0, "MOVE THE CAPSULE", DEEP);
    c.text(
        BODY,
        20.0,
        164.0,
        267.0,
        "Push left or right to move. Hold down to fall faster.",
        NIGHT,
    );

    for (i, &(sprite, label)) in PLAY_CONTROLS[1..].iter().enumerate() {
        let x = 116.0 + i as f32 * 164.0;
        c.art(sprite, rect(x - 43.0, 306.0, 86.0, 86.0));
        c.center_text(DISPLAY, 25.0, x, 416.0, label, DEEP);
    }
    c.fill(rect(34.0, 436.0, 656.0, 1.5), rule);
    control_strip(
        c,
        rect(34.0, 451.0, 656.0, 44.0),
        &[
            ("coin", "Insert a credit"),
            ("button-start", "Join the round"),
        ],
        NIGHT,
    );
}

fn paper_panel(c: &mut Canvas, w: f32, h: f32) {
    c.round(rect(0.0, 5.0, w, h - 5.0), 17.0, DEEP);
    c.round(rect(0.0, 0.0, w, h - 5.0), 17.0, PAPER);
}

fn background(c: &mut Canvas) {
    c.fill(rect(0.0, 0.0, c.w, c.h), NIGHT);
    let grid = rgb(25, 57, 60);
    let pitch = 48.0;
    let mut x = (c.w * 0.5) % pitch;
    while x < c.w {
        c.fill(rect(x, 77.0, 0.8, c.h - FOOTER_HEIGHT - 77.0), grid);
        x += pitch;
    }
    let mut y = 97.0;
    while y < c.h - FOOTER_HEIGHT {
        c.fill(rect(0.0, y, c.w, 0.8), grid);
        y += pitch;
    }
    for (x, y) in [
        (20.0, 101.0),
        (c.w - 20.0, 101.0),
        (20.0, c.h - 76.0),
        (c.w - 20.0, c.h - 76.0),
    ] {
        c.fill(rect(x - 4.0, y - 0.7, 8.0, 1.4), LINE);
        c.fill(rect(x - 0.7, y - 4.0, 1.4, 8.0), LINE);
    }
}

fn scene(c: &mut Canvas, r: Rect, t: f32, with_germs: bool) {
    c.within(r, 530.0, 495.0, |c| {
        c.art("savanna", rect(23.0, 8.0, 475.0, 475.0));
        c.ellipse(rect(75.0, 423.0, 302.0, 26.0), DEEP.alpha(60));
        c.ellipse(rect(322.0, 426.0, 127.0, 15.0), DEEP.alpha(67));
        c.art("doctor", rect(27.0, 44.0, 341.0, 409.0));
        c.art("secretary", rect(291.0, 172.0, 182.0, 271.0));
        if with_germs {
            let bob = (t * 2.0).sin() * 3.0;
            c.tile("germ-blue", rect(443.0, 103.0 + bob, 55.0, 55.0), 0, WHITE);
            c.tile("germ-red", rect(21.0, 340.0 - bob, 63.0, 63.0), 0, WHITE);
            c.tile("germ-gold", rect(335.0, 21.0 - bob, 44.0, 44.0), 0, WHITE);
        }
    });
}

fn sprite(block: Block) -> (&'static str, usize) {
    let names = match block.kind {
        Kind::Germ => ["germ-red", "germ-blue", "germ-gold"],
        Kind::Pill(None) => ["single-red", "single-blue", "single-gold"],
        Kind::Pill(Some(_)) => ["end-red", "end-blue", "end-gold"],
    };
    let turn = if let Kind::Pill(Some(d)) = block.kind {
        d as usize
    } else {
        0
    };
    (names[block.color as usize], turn)
}

fn block(c: &mut Canvas, b: Block, dest: Rect, ink: Ink) {
    let (name, turn) = sprite(b);
    c.tile(name, dest, turn, ink);
}

const LANE_W: f32 = 250.0;
const LANE_H: f32 = 536.0;

fn lanes(area: Rect, count: usize) -> Vec<Rect> {
    // Pick the arrangement giving each whole bottle the largest cell size.
    let gap = 14.0;
    let mut best = (0.0, 1);
    for cols in 1..=count {
        let rows = count.div_ceil(cols);
        let scale = ((area.w - gap * (cols - 1) as f32) / (cols as f32 * LANE_W))
            .min((area.h - gap * (rows - 1) as f32) / (rows as f32 * LANE_H));
        if scale > best.0 {
            best = (scale, cols);
        }
    }
    let (s, cols) = best;
    let (w, h) = (LANE_W * s, LANE_H * s);
    let gap_x = ((area.w - w * cols as f32) / cols as f32).clamp(gap, 64.0);
    let rows = count.div_ceil(cols);
    let y = area.y + (area.h - h * rows as f32 - gap * (rows - 1) as f32) * 0.5;
    (0..count)
        .map(|i| {
            let row = i / cols;
            let n = (count - row * cols).min(cols);
            let x = area.x + (area.w - n as f32 * w - (n - 1) as f32 * gap_x) * 0.5;
            rect(
                x + (i % cols) as f32 * (w + gap_x),
                y + row as f32 * (h + gap),
                w,
                h,
            )
        })
        .collect()
}

impl Ui {
    fn play_screen(&mut self, c: &mut Canvas, mut body: Rect, app: &App, t: f32) {
        if app.round.is_some() {
            let controls = rect(body.x, body.y + body.h - 32.0, body.w, 32.0);
            body.h -= controls.h + 6.0;
            if app.screen == Screen::Play {
                control_strip(c, controls, &PLAY_CONTROLS, PAPER);
            }
        }
        let boards = if let Some(demo) = &app.demo {
            std::slice::from_ref(&demo.board)
        } else {
            &app.round.as_ref().unwrap().boards[..]
        };
        let layout = lanes(body.inset(8.0), boards.len());
        let countdown = app.round.as_ref().and_then(|round| round.countdown);
        let mut level_on_bottle = boards.len() > 1;
        if boards.len() == 1 {
            let middle = layout[0];
            let left = rect(
                body.x,
                body.y + 10.0,
                middle.x - body.x - 30.0,
                body.h - 20.0,
            );
            let right = rect(
                middle.x + middle.w + 20.0,
                body.y,
                body.x + body.w - middle.x - middle.w - 20.0,
                body.h,
            );
            if left.w >= 190.0 {
                let stats = left.fit(250.0, 304.0);
                let width = stats.w.min(312.0);
                let height = width * 304.0 / 250.0;
                let panel = rect(
                    stats.x + (stats.w - width) * 0.5,
                    stats.y + (stats.h - height) * 0.5,
                    width,
                    height,
                );
                c.within(panel, 250.0, 304.0, |c| solo_stats(c, &boards[0]));
                scene(c, right, t, false);
            } else {
                level_on_bottle = true;
            }
        }
        for (i, (r, b)) in layout.into_iter().zip(boards).enumerate() {
            let seat = app.seats.get(i).copied();
            c.within(r, LANE_W, LANE_H, |c| {
                lane(c, b, seat, t);
                if level_on_bottle {
                    c.text(
                        DISPLAY,
                        15.0,
                        20.0,
                        56.0,
                        &format!("LEVEL {:02}", b.level),
                        SOFT,
                    );
                }
                if let Some(timer) = countdown.filter(|timer| timer.leader == i) {
                    finish_countdown(c, seconds_left(timer.time_left));
                }
                if let Some(s) = seat {
                    if let Some(pad) = &app.names[s] {
                        self.name_panel(c, s, pad, b.score);
                    }
                }
            });
        }
    }

    fn name_panel(&mut self, c: &mut Canvas, seat: usize, pad: &NamePad, score: u32) {
        let accent = SEATS[seat];
        c.round(rect(5.0, 129.0, 240.0, 287.0), 10.0, DEEP);
        c.round(rect(5.0, 125.0, 240.0, 287.0), 10.0, accent);
        c.round(rect(7.0, 127.0, 236.0, 283.0), 9.0, NIGHT);
        c.center_text(DISPLAY, 26.0, 125.0, 155.0, "NEW HIGH SCORE", accent);
        c.center_text(DISPLAY, 20.0, 125.0, 179.0, &format!("{score:06}"), PAPER);
        if pad.done {
            c.center_text(DISPLAY, 29.0, 125.0, 262.0, &pad.text, PAPER);
            c.center_text(BODY, 16.0, 125.0, 294.0, "Waiting for other players", SOFT);
            return;
        }
        for i in 0..NAME_LENGTH {
            let left = 17.0 + i as f32 * 27.4;
            let color = if i == pad.text.len() { GOLD } else { SOFT };
            c.fill(rect(left + 2.0, 215.0, 20.0, 1.5), color);
            if let Some(&ch) = pad.text.as_bytes().get(i) {
                c.center_text(
                    DISPLAY,
                    25.0,
                    left + 12.0,
                    210.0,
                    &char::from(ch).to_string(),
                    PAPER,
                );
            }
        }
        for i in 0..=DONE {
            let r = rect(
                15.0 + (i % KEY_COLUMNS) as f32 * 31.6,
                228.0 + (i / KEY_COLUMNS) as f32 * 30.0,
                29.0,
                27.0,
            );
            let hover = self.hit(c, r, Command::Name(seat, NameKey::Pick(i)));
            let selected = pad.cursor == i || hover;
            c.round(r, 3.0, if selected { GOLD } else { LINE });
            let label = match i {
                DONE => "OK".into(),
                DELETE => "DEL".into(),
                _ => char::from(b'A' + i as u8).to_string(),
            };
            c.center_text(
                DISPLAY,
                if i >= DELETE { 15.0 } else { 19.0 },
                r.x + r.w * 0.5,
                r.y + 20.0,
                &label,
                if selected { DEEP } else { PAPER },
            );
        }
        for (i, (sprite, caption)) in [
            ("joystick", "Move"),
            ("button-a", "Add"),
            ("button-b", "Erase"),
            ("button-start", "Finish"),
        ]
        .into_iter()
        .enumerate()
        {
            let x = 42.5 + 55.0 * i as f32;
            c.art(sprite, rect(x - 16.0, 351.0, 32.0, 32.0));
            c.center_text(BODY, 16.0, x, 400.0, caption, PAPER);
        }
        let seconds = seconds_left(pad.time_left);
        if seconds <= 10 {
            c.right_text(
                DISPLAY,
                16.0,
                230.0,
                179.0,
                &format!("{seconds}s left"),
                GOLD,
            );
        }
    }
}

fn finish_countdown(c: &mut Canvas, seconds: u64) {
    let center = LANE_W * 0.5;
    c.round(rect(25.0, 66.0, 200.0, 395.0), 12.0, WELL.alpha(235));
    c.center_text(
        DISPLAY,
        100.0,
        center,
        239.0,
        &seconds.to_string(),
        if seconds <= 5 { RED } else { GOLD },
    );
    c.center_text(
        DISPLAY,
        21.5,
        center,
        281.0,
        "All other players hurry!",
        PAPER,
    );
    c.center_text(BODY, 21.0, center, 316.0, "If any one of you", PAPER);
    c.center_text(BODY, 21.0, center, 341.0, "finishes now", PAPER);
    c.center_text(BODY, 21.0, center, 366.0, "you are all safe.", PAPER);
}

fn solo_stats(c: &mut Canvas, b: &Board) {
    c.text(DISPLAY, 20.0, 20.0, 31.0, "LEVEL", MINT);
    c.text(
        DISPLAY,
        104.0,
        16.0,
        122.0,
        &format!("{:02}", b.level),
        PAPER,
    );
    c.fill(rect(20.0, 143.0, 193.0, 1.0), LINE);
    c.text(DISPLAY, 20.0, 20.0, 182.0, "GERMS LEFT", MINT);
    for (i, color) in [Color::Red, Color::Blue, Color::Gold]
        .into_iter()
        .enumerate()
    {
        let count = b
            .cells
            .iter()
            .flatten()
            .filter(|v| v.kind == Kind::Germ && v.color == color)
            .count();
        let x = 18.0 + i as f32 * 70.0;
        block(
            c,
            Block {
                kind: Kind::Germ,
                color,
            },
            rect(x, 195.0, 50.0, 50.0),
            WHITE,
        );
        c.center_text(
            DISPLAY,
            34.0,
            x + 25.0,
            282.0,
            &format!("{count:02}"),
            PAPER,
        );
    }
}

fn lane(c: &mut Canvas, b: &Board, seat: Option<usize>, t: f32) {
    let accent = seat.map_or(GOLD, |s| SEATS[s]);
    c.round(rect(1.0, 4.0, 248.0, 532.0), 13.0, DEEP);
    c.round(rect(1.0, 0.0, 248.0, 532.0), 13.0, LINE);
    c.round(rect(2.5, 1.5, 245.0, 529.0), 12.0, rgb(30, 63, 67));
    c.round(rect(15.0, 12.0, 88.0, 30.0), 6.0, accent);
    c.center_text(
        DISPLAY,
        21.0,
        59.0,
        34.0,
        &seat.map_or_else(|| "CPU".into(), |s| format!("PLAYER {}", s + 1)),
        DEEP,
    );
    c.text(DISPLAY, 16.0, 135.0, 32.0, "NEXT", SOFT);
    for (i, color) in b.next.iter().copied().enumerate() {
        use crate::game::Direction::{East, West};
        block(
            c,
            Block {
                color,
                kind: Kind::Pill(Some(if i == 0 { East } else { West })),
            },
            rect(176.0 + i as f32 * 23.0, 15.0, 23.0, 23.0),
            WHITE,
        );
    }
    // The neck is open above the two middle spawn columns.
    c.round(rect(20.0, 62.0, 210.0, 409.0), 19.0, DEEP);
    c.round(rect(19.0, 60.0, 212.0, 407.0), 18.0, accent);
    c.round(rect(22.0, 63.0, 206.0, 401.0), 15.0, rgb(77, 110, 109));
    c.round(rect(25.0, 66.0, 200.0, 395.0), 12.0, WELL);
    c.fill(rect(97.0, 48.0, 55.0, 27.0), WELL);
    c.fill(rect(94.0, 49.0, 4.0, 14.0), accent);
    c.fill(rect(152.0, 49.0, 4.0, 14.0), accent);
    c.round(rect(91.0, 47.0, 9.0, 6.0), 2.0, accent);
    c.round(rect(150.0, 47.0, 9.0, 6.0), 2.0, accent);
    const STEP: f32 = 24.0;
    let tile = |x: i32, y: i32| rect(29.0 + x as f32 * STEP, 70.0 + y as f32 * STEP, STEP, STEP);
    let grid = rgb(29, 53, 61);
    for x in 1..COLS {
        c.fill(rect(29.0 + x as f32 * STEP, 70.0, 0.7, 384.0), grid);
    }
    for y in 1..ROWS {
        c.fill(rect(29.0, 70.0 + y as f32 * STEP, 192.0, 0.7), grid);
    }
    for (i, item) in b.cells.iter().enumerate() {
        let Some(item) = item else { continue };
        let mut r = tile((i % COLS) as i32, (i / COLS) as i32);
        if b.marked[i] && matches!(b.phase,Phase::Flash(n) if n%8<4) {
            c.round(r.inset(1.0), 5.0, PAPER);
            c.star(r.x + 12.0, r.y + 12.0, 8.0, RED);
        } else {
            if item.kind == Kind::Germ {
                let bounce = (t * 3.2 + (i % 5) as f32).sin() * 0.7;
                r.y += bounce;
                r.h -= bounce;
            }
            block(c, *item, r, WHITE);
        }
    }
    if let Phase::Falling(piece) = b.phase {
        let target = b.landing(piece);
        if target.y != piece.y {
            for (x, y, v) in target.halves() {
                block(c, v, tile(x, y), WHITE.alpha(55));
            }
        }
        for (x, y, v) in piece.halves() {
            block(c, v, tile(x, y), WHITE);
        }
    }
    if b.phase == Phase::Out {
        c.fill(rect(25.0, 202.0, 200.0, 61.0), DEEP.alpha(233));
        c.center_text(DISPLAY, 32.0, 125.0, 242.0, "BOTTLE FULL", RED);
    } else if b.chain > 1 {
        c.round(rect(61.0, 78.0, 128.0, 27.0), 5.0, NIGHT);
        c.center_text(
            DISPLAY,
            20.0,
            125.0,
            98.0,
            &format!("CHAIN x{}", b.chain),
            GOLD,
        );
    }
    c.text(DISPLAY, 15.0, 20.0, 490.0, "GERMS", SOFT);
    c.text(
        DISPLAY,
        32.0,
        19.0,
        519.0,
        &format!("{:02}", b.germs()),
        accent,
    );
    if b.bonus_notice_ticks > 0 {
        c.right_text(
            DISPLAY,
            15.0,
            229.0,
            490.0,
            &format!("CLEAR BONUS +{CLEAR_BONUS}"),
            GOLD,
        );
    } else {
        c.right_text(DISPLAY, 15.0, 229.0, 490.0, "SCORE", SOFT);
    }
    c.right_text(
        DISPLAY,
        30.0,
        229.0,
        519.0,
        &format!("{:06}", b.score),
        PAPER,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottles_fit_without_overlap_and_keep_square_cells() {
        for (w, h) in [
            (1280., 720.),
            (960., 720.),
            (720., 720.),
            (640., 1120.),
            (2000., 720.),
        ] {
            for n in 1..=4 {
                let area = rect(25., 83., w - 50., h - 141.);
                let r = lanes(area, n);
                for (i, a) in r.iter().enumerate() {
                    assert!(a.x >= area.x - 0.01 && a.y >= area.y - 0.01);
                    assert!(
                        a.x + a.w <= area.x + area.w + 0.01 && a.y + a.h <= area.y + area.h + 0.01
                    );
                    assert!((a.w / a.h - LANE_W / LANE_H).abs() < 0.001);
                    for b in &r[..i] {
                        assert!(
                            a.x + a.w <= b.x
                                || b.x + b.w <= a.x
                                || a.y + a.h <= b.y
                                || b.y + b.h <= a.y
                        );
                    }
                }
                if n == 4 && w == 1280. {
                    assert_eq!(r[0].y, r[3].y);
                }
                if n == 4 && w == 640. {
                    assert!(r[2].y > r[0].y);
                }
            }
        }
    }
}
