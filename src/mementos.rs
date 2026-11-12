//! Extra Mementos versions: Persona 1, Persona 2 IS, Persona 2 EP and SMT.
//!
//! Songs get one of the special categories `jack_mementoes_p1`, `jack_mementoes_p2is`,
//! `jack_mementoes_p2ep` or `jack_mementoes_smt` (Smash Music Master+). The game only knows
//! P3/P4/P5, so each extra version borrows one of them (P1 -> P5, P2 -> P4, SMT -> P3) and
//! its stage files are served from `sd:/ultimate/persona_joker_colors/mementos/<dir>/`
//! instead of the borrowed version's. With any other song the vanilla files load.

use crate::config::Color;
use core::sync::atomic::{AtomicU8, Ordering};
use std::collections::HashMap;
use std::sync::OnceLock;

pub const MEMENTOS_ROOT: &str = "sd:/ultimate/persona_joker_colors/mementos";
const BGM_DB: &str = "ui/param/database/ui_bgm_db.prc";
const BGM_DB_BUFFER: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Extra {
    P1,
    P2Is,
    P2Ep,
    Smt,
}

pub const ALL: [Extra; 4] = [Extra::P1, Extra::P2Is, Extra::P2Ep, Extra::Smt];

impl Extra {
    /// Special category written by Smash Music Master+.
    pub fn category(self) -> &'static str {
        match self {
            Extra::P1 => "jack_mementoes_p1",
            Extra::P2Is => "jack_mementoes_p2is",
            Extra::P2Ep => "jack_mementoes_p2ep",
            Extra::Smt => "jack_mementoes_smt",
        }
    }

    /// Name in config.txt (`mementos p1, color: ..., theme: ...`).
    pub fn config_name(self) -> &'static str {
        match self {
            Extra::P1 => "p1",
            Extra::P2Is => "p2is",
            Extra::P2Ep => "p2ep",
            Extra::Smt => "smt",
        }
    }

    pub fn from_config_name(name: &str) -> Option<Extra> {
        ALL.iter().copied().find(|e| e.config_name() == name)
    }

    /// Vanilla category the game is told the song has, so every song-based choice
    /// (stage version, result screen, colors) uses the borrowed version.
    pub fn base_category(self) -> &'static str {
        match self {
            Extra::P1 => "jack_mementoes_p5",
            Extra::P2Is | Extra::P2Ep => "jack_mementoes_p4",
            Extra::Smt => "jack_mementoes_p3",
        }
    }

    /// Vanilla version whose slot is used: 0 = P3, 1 = P4, 2 = P5.
    pub fn base_variant(self) -> u32 {
        match self {
            Extra::P1 => 2,
            Extra::P2Is | Extra::P2Ep => 1,
            Extra::Smt => 0,
        }
    }

    /// Folder with the stage files; both Persona 2 versions share one stage.
    pub fn dir(self) -> &'static str {
        match self {
            Extra::P1 => "p1",
            Extra::P2Is | Extra::P2Ep => "p2",
            Extra::Smt => "smt",
        }
    }

    /// Defaults used when config.txt has no `mementos` line for the version.
    pub fn default_color(self) -> &'static str {
        match self {
            Extra::P1 => "purple",
            Extra::P2Is => "orange",
            Extra::P2Ep => "white",
            Extra::Smt => "#0a6b2a",
        }
    }

    pub fn default_theme(self) -> &'static str {
        match self {
            Extra::P1 => "bgm_z97d_f_jack_p1",
            Extra::P2Is => "bgm_z97e_f_jack_p2tsumi",
            Extra::P2Ep => "bgm_z97f_f_jack_p2batsu",
            Extra::Smt => "bgm_z97h_f_jack_smtiv",
        }
    }

    fn index(self) -> u8 {
        ALL.iter().position(|e| *e == self).unwrap() as u8 + 1
    }
}

#[derive(Clone, Debug)]
pub struct ExtraConfig {
    pub color: Color,
    pub theme: String,
}

// ---------------------------------------------------------------- current version

/// Version picked for the last Mementos load (0 = vanilla).
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Option<Extra> {
    match CURRENT.load(Ordering::Relaxed) {
        0 => None,
        i => ALL.get(i as usize - 1).copied(),
    }
}

const HASH_MASK: u64 = 0xff_ffff_ffff;

/// Global holding the song of the match being set up (ui_bgm_id with its type byte). It is
/// already set when the stage starts loading, 10-20 s before the stage looks the song up.
const NEXT_SONG_OFFSET: usize = 0x530a7d0;

/// Remembers the extra version of a song (`bgm` is the tagged ui_bgm_id), if any.
pub fn note_song(bgm: u64) {
    // Lookups without a song (hash 0) happen right after the real one; keep the song.
    if bgm & HASH_MASK == 0 {
        return;
    }
    let extra = categories().get(&(bgm & HASH_MASK)).copied();
    let index = extra.map_or(0, Extra::index);
    if CURRENT.swap(index, Ordering::Relaxed) != index {
        println!("[persona_joker_colors] song {:#x} -> {:?}", bgm, extra);
    }
}

/// Reads the song of the match being set up, so the stage files load for its version.
fn note_next_song() {
    let song = unsafe { *((crate::text_base() + NEXT_SONG_OFFSET) as *const u64) };
    note_song(song);
}

/// Called from the game's own song -> Mementos version function; returns the version slot
/// to use instead of the vanilla one, if the song has an extra category.
pub fn pick(bgm: u64) -> Option<u32> {
    note_song(bgm);
    current().map(Extra::base_variant)
}

/// The game's special_category hash -> category kind conversion: extra categories are
/// converted as their borrowed vanilla category.
pub fn vanilla_category(hash: u64) -> u64 {
    match ALL.iter().find(|e| arcropolis_api::hash40(e.category()).as_u64() == hash & HASH_MASK) {
        Some(extra) => (hash & !HASH_MASK) | arcropolis_api::hash40(extra.base_category()).as_u64(),
        None => hash,
    }
}

// ---------------------------------------------------------------- song categories

/// ui_bgm_id and stream_set_id of every song with an extra category, from the game's
/// ui_bgm_db.prc (classic Music Master build) and from the song_data.json of every mod
/// (Modular CSK Pack build, added to the game by The CSK Collection).
fn categories() -> &'static HashMap<u64, Extra> {
    static MAP: OnceLock<HashMap<u64, Extra>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut buffer = vec![0u8; BGM_DB_BUFFER];
        let (mut map, songs) = match arcropolis_api::load_original_file(arcropolis_api::hash40(BGM_DB), &mut buffer[..]) {
            Some(size) => read_categories(&buffer[..size]).unwrap_or_default(),
            None => Default::default(),
        };
        let _ = PERSONA_SONGS.set(songs);
        drop(buffer);
        map.extend(read_csk_categories());
        println!("[persona_joker_colors] {} songs use an extra Mementos version", map.len() / 2);
        map
    })
}

const MODS_ROOT: &str = "sd:/ultimate/mods";

fn read_csk_categories() -> HashMap<u64, Extra> {
    let h = |s: &str| arcropolis_api::hash40(s).as_u64();
    let mut map = HashMap::new();
    let Ok(mods) = std::fs::read_dir(MODS_ROOT) else { return map };
    for entry in mods.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(format!("{}/{}/database/song_data.json", MODS_ROOT, name)) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else {
            println!("[persona_joker_colors] could not read {}/database/song_data.json", name);
            continue;
        };
        let str_of = |v: &serde_json::Value, key: &str| v.get(key).and_then(|s| s.as_str()).map(str::to_owned);
        let mut by_stream_set = HashMap::new();
        for set in json["stream_set_entries"].as_array().into_iter().flatten() {
            let (Some(id), Some(category)) = (str_of(set, "stream_set_id"), str_of(set, "special_category")) else { continue };
            if let Some(extra) = ALL.iter().find(|e| e.category() == category) {
                by_stream_set.insert(id, *extra);
            }
        }
        for bgm in json["bgm_database_entries"].as_array().into_iter().flatten() {
            let (Some(id), Some(set)) = (str_of(bgm, "ui_bgm_id"), str_of(bgm, "stream_set_id")) else { continue };
            if let Some(extra) = by_stream_set.get(&set) {
                map.insert(h(&id), *extra);
                map.insert(h(&set), *extra);
            }
        }
    }
    map
}

/// One ui_bgm_id (without its type byte) per vanilla Mementos category, in the order
/// P5, P4, P3; the victory scene uses the version of the song it is given.
static PERSONA_SONGS: OnceLock<[Option<u64>; 3]> = OnceLock::new();

const VANILLA_CATEGORIES: [&str; 3] = ["jack_mementoes_p5", "jack_mementoes_p4", "jack_mementoes_p3"];

/// A song of the vanilla Mementos version `index` (0 = P5, 1 = P4, 2 = P3).
pub fn persona_song(index: usize) -> Option<u64> {
    categories();
    *PERSONA_SONGS.get()?.get(index)?
}

fn read_categories(data: &[u8]) -> Option<(HashMap<u64, Extra>, [Option<u64>; 3])> {
    let prc = prc::Prc::parse(data)?;
    let h = |s: &str| arcropolis_api::hash40(s).as_u64();
    let by_category: HashMap<u64, Extra> = ALL.iter().map(|e| (h(e.category()), *e)).collect();
    let root = prc.root();
    let mut by_stream_set = HashMap::new();
    let mut vanilla_sets = HashMap::new();
    for set in prc.list(prc.field(root, h("stream_set"))?)? {
        let category = prc.hash(prc.field(set, h("special_category"))?)?;
        if let Some(extra) = by_category.get(&category) {
            by_stream_set.insert(prc.hash(prc.field(set, h("stream_set_id"))?)?, *extra);
        } else if let Some(index) = VANILLA_CATEGORIES.iter().position(|c| h(c) == category) {
            vanilla_sets.insert(prc.hash(prc.field(set, h("stream_set_id"))?)?, index);
        }
    }
    let mut songs = [None; 3];
    let mut map = HashMap::new();
    for entry in prc.list(prc.field(root, h("db_root"))?)? {
        let (Some(bgm), Some(set)) = (
            prc.field(entry, h("ui_bgm_id")).and_then(|p| prc.hash(p)),
            prc.field(entry, h("stream_set_id")).and_then(|p| prc.hash(p)),
        ) else {
            continue;
        };
        if let Some(extra) = by_stream_set.get(&set) {
            map.insert(bgm, *extra);
            map.insert(set, *extra);
        }
        if let Some(&index) = vanilla_sets.get(&set) {
            songs[index].get_or_insert(bgm);
        }
    }
    Some((map, songs))
}

/// Minimal reader for the param (paracobn) format: structs, lists and hash40 values.
mod prc {
    pub struct Prc<'a> {
        data: &'a [u8],
        hashes: usize,
        refs: usize,
        root: usize,
    }

    impl<'a> Prc<'a> {
        pub fn parse(data: &'a [u8]) -> Option<Self> {
            if data.get(..8)? != b"paracobn" {
                return None;
            }
            let mut prc = Prc { data, hashes: 0x10, refs: 0, root: 0 };
            let hash_size = prc.u32(8)? as usize;
            let ref_size = prc.u32(12)? as usize;
            prc.refs = 0x10 + hash_size;
            prc.root = prc.refs + ref_size;
            Some(prc)
        }

        fn u32(&self, at: usize) -> Option<u32> {
            Some(u32::from_le_bytes(self.data.get(at..at + 4)?.try_into().ok()?))
        }

        fn hash_at(&self, index: u32) -> Option<u64> {
            let at = self.hashes + index as usize * 8;
            Some(u64::from_le_bytes(self.data.get(at..at + 8)?.try_into().ok()?))
        }

        pub fn root(&self) -> usize {
            self.root
        }

        /// Struct field by name hash.
        pub fn field(&self, param: usize, name: u64) -> Option<usize> {
            if *self.data.get(param)? != 12 {
                return None;
            }
            let count = self.u32(param + 1)?;
            let refs = self.refs + self.u32(param + 5)? as usize;
            (0..count as usize).find_map(|i| {
                (self.hash_at(self.u32(refs + i * 8)?)? == name).then(|| Some(param + self.u32(refs + i * 8 + 4)? as usize))?
            })
        }

        pub fn list(&self, param: usize) -> Option<Vec<usize>> {
            if *self.data.get(param)? != 11 {
                return None;
            }
            let count = self.u32(param + 1)? as usize;
            (0..count).map(|i| Some(param + self.u32(param + 5 + i * 4)? as usize)).collect()
        }

        pub fn hash(&self, param: usize) -> Option<u64> {
            if *self.data.get(param)? != 9 {
                return None;
            }
            self.hash_at(self.u32(param + 1)?)
        }
    }
}

// ---------------------------------------------------------------- stage files

/// Stage files per path hash: (version folder, SD path) for each folder that has the file.
fn files() -> &'static HashMap<u64, Vec<(&'static str, String)>> {
    static FILES: OnceLock<HashMap<u64, Vec<(&'static str, String)>>> = OnceLock::new();
    FILES.get_or_init(|| {
        let mut files: HashMap<u64, Vec<(&'static str, String)>> = HashMap::new();
        for dir in ["p1", "p2", "smt"] {
            let root = format!("{}/{}", MEMENTOS_ROOT, dir);
            let mut stack = vec![root.clone()];
            while let Some(folder) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&folder) else { continue };
                for entry in entries.flatten() {
                    let path = format!("{}/{}", folder, entry.file_name().to_string_lossy());
                    match entry.file_type() {
                        Ok(t) if t.is_dir() => stack.push(path),
                        Ok(_) => {
                            let game_path = path[root.len() + 1..].to_string();
                            files.entry(arcropolis_api::hash40(&game_path).as_u64()).or_default().push((dir, path));
                        }
                        Err(_) => {}
                    }
                }
            }
        }
        files
    })
}

extern "C" fn stage_file_callback(hash: u64, out: *mut u8, length: usize, out_size: &mut usize) -> bool {
    note_next_song();
    let Some(extra) = current() else {
        return false;
    };
    let Some((_, path)) = files().get(&hash).and_then(|v| v.iter().find(|(dir, _)| *dir == extra.dir())) else {
        return false;
    };
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    if data.len() > length {
        println!("[persona_joker_colors] {} is larger than registered", path);
        return false;
    }
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), out, data.len()) };
    *out_size = data.len();
    if crate::DEBUG_LOG {
        println!("[persona_joker_colors] served {}", path);
    }
    true
}

/// Vanilla sizes (`<game path> <size>` per line), so the buffer also fits the vanilla file
/// that loads when the callback declines.
fn vanilla_sizes() -> HashMap<u64, usize> {
    let text = std::fs::read_to_string(format!("{}/vanilla_sizes.txt", MEMENTOS_ROOT)).unwrap_or_default();
    text.lines()
        .filter_map(|l| l.rsplit_once(' '))
        .filter_map(|(path, size)| Some((arcropolis_api::hash40(path.trim()).as_u64(), size.trim().parse().ok()?)))
        .collect()
}

/// Registers every file under `mementos/<dir>/` with room for its largest version.
pub fn register_stage_files() {
    let vanilla = vanilla_sizes();
    let mut count = 0;
    for (hash, versions) in files() {
        let size = versions
            .iter()
            .filter_map(|(_, p)| std::fs::metadata(p).ok())
            .map(|m| m.len() as usize)
            .chain(vanilla.get(hash).copied())
            .max()
            .unwrap_or(0);
        if size > 0 {
            arcropolis_api::register_callback(*hash, size, stage_file_callback);
            count += 1;
        }
    }
    println!("[persona_joker_colors] {} extra Mementos stage files", count);
}
