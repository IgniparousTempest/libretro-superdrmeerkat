use crate::config::MAX_PLAYERS;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const PLACES: usize = 10;
const STAFF: [(&str, u32); PLACES] = [
    ("Chief Lion", 2000),
    ("Consultant Springbok", 1600),
    ("Fellow Buffalo", 1300),
    ("Attending Zebra", 1050),
    ("Surgeon Snoek", 800),
    ("Resident Warthog", 600),
    ("Senior Honey Badger", 450),
    ("Junior Impala", 300),
    ("Intern Mongoose", 200),
    ("Student Hippo", 100),
];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Entry {
    pub name: String,
    pub score: u32,
}

#[derive(Serialize, Deserialize)]
struct Table {
    entries: Vec<Entry>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            entries: STAFF
                .iter()
                .map(|&(name, score)| Entry {
                    name: name.into(),
                    score,
                })
                .collect(),
        }
    }
}

fn player_name(name: &str) -> bool {
    (1..=NAME_LENGTH).contains(&name.len()) && name.bytes().all(|c| c.is_ascii_uppercase())
}

impl Table {
    fn load(path: &Path) -> io::Result<Self> {
        let text = match fs::read_to_string(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            result => result?,
        };
        let mut table: Self = toml::from_str(&text).map_err(io::Error::other)?;
        if table.entries.iter().any(|entry| {
            !player_name(&entry.name) && !STAFF.iter().any(|&(name, _)| name == entry.name)
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid high-score name",
            ));
        }
        table.rank();
        Ok(table)
    }

    fn rank(&mut self) {
        // Stable sorting gives the incumbent the place on a tied cutoff.
        self.entries.sort_by_key(|e| Reverse(e.score));
        self.entries.truncate(PLACES);
    }
}

pub struct HighScores {
    path: PathBuf,
    table: Table,
    pending: Vec<Entry>,
    pub storage_error: bool,
}

impl HighScores {
    pub fn cabinet() -> Self {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        Self::open(data.join("super-dr-meerkat/high-scores.toml"))
    }

    pub fn open(path: PathBuf) -> Self {
        let result = Table::load(&path);
        let storage_error = result.is_err();
        let table = result.unwrap_or_else(|e| {
            eprintln!("Reading {}: {e}", path.display());
            Table::default()
        });
        Self {
            path,
            table,
            pending: Vec::new(),
            storage_error,
        }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.table.entries
    }

    pub fn refresh(&mut self) {
        if !self.pending.is_empty() {
            self.flush();
            return;
        }
        match Table::load(&self.path) {
            Ok(table) => {
                self.table = table;
                self.storage_error = false;
            }
            Err(e) => {
                eprintln!("Reading {}: {e}", self.path.display());
                self.storage_error = true;
            }
        }
    }

    pub fn qualifiers(
        &self,
        players: impl IntoIterator<Item = (usize, u32)>,
    ) -> [bool; MAX_PLAYERS] {
        let mut ranking: Vec<_> = self.entries().iter().map(|e| (None, e.score)).collect();
        // Players arrive in seat order. Rank the entire race before asking for names.
        ranking.extend(players.into_iter().map(|(seat, score)| (Some(seat), score)));
        ranking.sort_by_key(|&(_, score)| Reverse(score));
        let mut qualified = [false; MAX_PLAYERS];
        for (seat, _) in ranking.into_iter().take(PLACES) {
            if let Some(seat) = seat {
                qualified[seat] = true;
            }
        }
        qualified
    }

    pub fn record(&mut self, name: &str, score: u32) {
        if player_name(name) {
            let entry = Entry {
                name: name.into(),
                score,
            };
            self.table.entries.push(entry.clone());
            self.table.rank();
            self.pending.push(entry);
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        match self.write_pending() {
            Ok(table) => {
                self.table = table;
                self.pending.clear();
                self.storage_error = false;
            }
            Err(e) => {
                eprintln!("Saving {}: {e}", self.path.display());
                self.storage_error = true;
            }
        }
    }

    fn write_pending(&self) -> io::Result<Table> {
        let dir = self.path.parent().unwrap();
        fs::create_dir_all(dir)?;
        // Lock a stable sibling. The score file itself is replaced by rename.
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_extension("lock"))?;
        lock.lock_exclusive()?;
        let mut table = Table::load(&self.path)?;
        table.entries.extend_from_slice(&self.pending);
        table.rank();
        let text = toml::to_string(&table).map_err(io::Error::other)?;
        let tmp = self.path.with_extension("tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(tmp, &self.path)?;
        let _ = File::open(dir).and_then(|d| d.sync_all());
        Ok(table)
    }
}

pub const NAME_LENGTH: usize = 8;
pub const KEY_COLUMNS: usize = 7;
pub const DELETE: usize = 26;
pub const DONE: usize = 27;
pub const ENTRY_TIME: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKey {
    Up,
    Down,
    Left,
    Right,
    Select,
    Delete,
    Finish,
    Pick(usize),
}

pub struct NamePad {
    pub text: String,
    pub cursor: usize,
    pub done: bool,
    pub time_left: Duration,
}

impl Default for NamePad {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            done: false,
            time_left: ENTRY_TIME,
        }
    }
}

impl NamePad {
    pub fn press(&mut self, key: NameKey) {
        if self.done {
            return;
        }
        self.time_left = ENTRY_TIME;
        let cells = DONE + 1;
        match key {
            NameKey::Left => self.cursor = (self.cursor + cells - 1) % cells,
            NameKey::Right => self.cursor = (self.cursor + 1) % cells,
            NameKey::Up => self.cursor = (self.cursor + cells - KEY_COLUMNS) % cells,
            NameKey::Down => self.cursor = (self.cursor + KEY_COLUMNS) % cells,
            NameKey::Delete => {
                self.text.pop();
            }
            NameKey::Finish => self.done = true,
            NameKey::Select => self.pick(self.cursor),
            NameKey::Pick(i) if i < cells => {
                self.cursor = i;
                self.pick(i);
            }
            NameKey::Pick(_) => {}
        }
    }

    fn pick(&mut self, i: usize) {
        match i {
            DONE => self.done = true,
            DELETE => {
                self.text.pop();
            }
            _ if self.text.len() < NAME_LENGTH => self.text.push((b'A' + i as u8) as char),
            _ => {}
        }
    }
}
