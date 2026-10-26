//! Per-slot configuration: `sd:/ultimate/persona_joker_colors/config.txt`.
//!
//! One slot per line, optionally prefixed by the fighter (Joker when omitted), e.g.
//!     c08, color: blue, theme: bgm_z97c_f_jack_p3
//!     c21-c23, color: blue, theme: bgm_z97c_f_jack_p3
//!     roy c20, color: red, theme: bgm_z97e_f_jack_p2tsumi
//! `color` is a name from the palette below or `#RRGGBB`; both keys are optional.
//! A trailing `fixed` keeps the color and theme on Mementos too.
//! Lines starting with `#` are comments. Slots without a line keep vanilla behavior.

use std::collections::HashMap;

pub const CONFIG_PATH: &str = "sd:/ultimate/persona_joker_colors/config.txt";

/// Where a color comes from in the game's own Persona color table (index 9/10/11), if it is
/// one of the three Mementos colors; custom colors carry their own values.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    Table(usize),
    Custom([f32; 3]),
}

#[derive(Clone, Debug)]
pub struct Color {
    /// Name used for the sky texture folder (`sky/<name>`).
    pub sky: Option<String>,
    pub effect: Source,
    /// HUD jack_bg color, 0..255.
    pub hud: [f32; 3],
    /// Existing HUD animation frame (0 red, 1 blue, 2 yellow) or None for an added frame.
    pub hud_frame: Option<f32>,
}

fn named(name: &str) -> Option<Color> {
    let custom = |sky: &str, effect: [f32; 3], hud: [f32; 3]| Color {
        sky: Some(sky.to_string()),
        effect: Source::Custom(effect),
        hud,
        hud_frame: None,
    };
    Some(match name {
        "red" => Color { sky: Some("red".into()), effect: Source::Table(11), hud: [255.0, 0.0, 0.0], hud_frame: Some(0.0) },
        "blue" => Color { sky: Some("blue".into()), effect: Source::Table(9), hud: [0.0, 42.6, 255.0], hud_frame: Some(1.0) },
        "yellow" => Color { sky: Some("yellow".into()), effect: Source::Table(10), hud: [223.2, 149.4, 4.3], hud_frame: Some(2.0) },
        "green" => custom("green", [0.05, 0.8, 0.1], [20.0, 200.0, 40.0]),
        "pink" => custom("pink", [1.0, 0.3, 0.6], [255.0, 80.0, 170.0]),
        "purple" => custom("purple", [0.55, 0.15, 1.0], [140.0, 40.0, 255.0]),
        "orange" => custom("orange", [1.0, 0.35, 0.0], [255.0, 100.0, 0.0]),
        "white" => custom("white", [0.95, 0.95, 0.95], [240.0, 240.0, 240.0]),
        "black" => custom("black", [0.03, 0.03, 0.03], [25.0, 25.0, 25.0]),
        _ => return None,
    })
}

fn hex(value: &str) -> Option<Color> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    let rgb = [channel(0)?, channel(2)?, channel(4)?];
    Some(Color {
        sky: Some(format!("hex_{}", digits.to_ascii_lowercase())),
        effect: Source::Custom(rgb.map(|c| c as f32 / 255.0)),
        hud: rgb.map(|c| c as f32),
        hud_frame: None,
    })
}

#[derive(Clone, Debug, Default)]
pub struct SlotConfig {
    pub color: Option<Color>,
    /// Stream name without extension, e.g. `bgm_z97c_f_jack_p3`.
    pub theme: Option<String>,
    /// Keep the color and theme on Mementos too (`fixed` in the line).
    pub fixed: bool,
}

pub const FIGHTER_KIND_JACK: u32 = 0x52;

/// Fighter names (as in fighter/<name>/ folders) and their FIGHTER_KIND values.
const FIGHTER_KINDS: &[(&str, u32)] = &[
    ("bayonetta", 0x40), ("brave", 0x53), ("buddy", 0x54), ("captain", 0xB), ("chrom", 0x1B),
    ("cloud", 0x3E), ("daisy", 0xE), ("dedede", 0x2A), ("demon", 0x5C), ("diddy", 0x27),
    ("dolly", 0x55), ("donkey", 0x1), ("duckhunt", 0x3B), ("edge", 0x59), ("eflame", 0x5A),
    ("element", 0x75), ("elight", 0x5B), ("falco", 0x14), ("flame", 0x73), ("fox", 0x7),
    ("fushigisou", 0x70), ("gamewatch", 0x1C), ("ganon", 0x18), ("gaogaen", 0x47), ("gekkouga", 0x35),
    ("ice_climber", 0x6E), ("ike", 0x23), ("inkling", 0x41), ("jack", 0x52), ("kamui", 0x3F),
    ("ken", 0x3D), ("kirby", 0x6), ("koopa", 0xF), ("koopag", 0x4D), ("koopajr", 0x3A),
    ("krool", 0x45), ("light", 0x74), ("link", 0x2), ("littlemac", 0x34), ("lizardon", 0x71),
    ("lucario", 0x2C), ("lucas", 0x28), ("lucina", 0x16), ("luigi", 0x9), ("mario", 0x0),
    ("mariod", 0x12), ("marth", 0x15), ("master", 0x56), ("metaknight", 0x1D), ("mewtwo", 0x19),
    ("miifighter", 0x48), ("miigunner", 0x4A), ("miiswordsman", 0x49), ("murabito", 0x30), ("nana", 0x4C),
    ("ness", 0xA), ("packun", 0x51), ("pacman", 0x37), ("palutena", 0x36), ("peach", 0xD),
    ("pfushigisou", 0x25), ("pichu", 0x13), ("pickel", 0x58), ("pikachu", 0x8), ("pikmin", 0x2B),
    ("pit", 0x1E), ("pitb", 0x1F), ("plizardon", 0x26), ("popo", 0x4B), ("ptrainer", 0x72),
    ("purin", 0xC), ("pzenigame", 0x24), ("reflet", 0x38), ("richter", 0x44), ("ridley", 0x42),
    ("robot", 0x2D), ("rockman", 0x31), ("rosetta", 0x33), ("roy", 0x1A), ("ryu", 0x3C),
    ("samus", 0x3), ("samusd", 0x4), ("sheik", 0x10), ("shizue", 0x46), ("shulk", 0x39),
    ("simon", 0x43), ("snake", 0x22), ("sonic", 0x29), ("szerosuit", 0x20), ("tantan", 0x57),
    ("toonlink", 0x2E), ("trail", 0x5D), ("wario", 0x21), ("wiifit", 0x32), ("wolf", 0x2F),
    ("yoshi", 0x5), ("younglink", 0x17), ("zelda", 0x11), ("zenigame", 0x6F),
];

fn fighter_kind(name: &str) -> Option<u32> {
    FIGHTER_KINDS.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

#[derive(Default)]
pub struct Config {
    /// Keyed by (FIGHTER_KIND, costume slot).
    pub slots: HashMap<(u32, u8), SlotConfig>,
    /// Colors that need an added HUD frame, in frame order (frame = 3 + index).
    pub extra_hud_colors: Vec<[f32; 3]>,
}

fn parse_slot(token: &str) -> Option<u8> {
    token.trim().trim_start_matches(['c', 'C']).parse().ok()
}

fn parse_slots(token: &str) -> Option<Vec<u8>> {
    match token.split_once('-') {
        Some((a, b)) => Some((parse_slot(a)?..=parse_slot(b)?).collect()),
        None => Some(vec![parse_slot(token)?]),
    }
}

pub fn parse(text: &str) -> Config {
    let mut config = Config::default();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split(',').map(str::trim).filter(|f| !f.is_empty());
        let head = fields.next().unwrap_or_default();
        let (kind, slot_token) = match head.split_once(char::is_whitespace) {
            Some((fighter, slots)) => match fighter_kind(&fighter.to_ascii_lowercase()) {
                Some(kind) => (kind, slots.trim()),
                None => {
                    println!("[persona_joker_colors] config line {}: unknown fighter '{}'", number + 1, fighter);
                    continue;
                }
            },
            None => (FIGHTER_KIND_JACK, head),
        };
        let Some(slots) = parse_slots(slot_token) else {
            println!("[persona_joker_colors] config line {}: bad slot '{}'", number + 1, line);
            continue;
        };
        let mut slot_config = SlotConfig::default();
        for field in fields {
            let Some((key, value)) = field.split_once(':') else {
                match field.to_ascii_lowercase().as_str() {
                    "fixed" => slot_config.fixed = true,
                    other => println!("[persona_joker_colors] config line {}: unknown option '{}'", number + 1, other),
                }
                continue;
            };
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "color" => {
                    let lower = value.to_ascii_lowercase();
                    slot_config.color = named(&lower).or_else(|| hex(&lower));
                    if slot_config.color.is_none() {
                        println!("[persona_joker_colors] config line {}: unknown color '{}'", number + 1, value);
                    }
                }
                "theme" => {
                    let name = value.trim_end_matches(".nus3audio");
                    let name = if name.starts_with("bgm_") { name.to_string() } else { format!("bgm_{}", name) };
                    slot_config.theme = Some(name);
                }
                other => println!("[persona_joker_colors] config line {}: unknown key '{}'", number + 1, other),
            }
        }
        if let Some(color) = &slot_config.color {
            if color.hud_frame.is_none() && !config.extra_hud_colors.contains(&color.hud) {
                config.extra_hud_colors.push(color.hud);
            }
        }
        for slot in slots {
            config.slots.insert((kind, slot), slot_config.clone());
        }
    }
    config
}

impl Config {
    pub fn load() -> Config {
        match std::fs::read_to_string(CONFIG_PATH) {
            Ok(text) => parse(&text),
            Err(_) => {
                println!("[persona_joker_colors] no {}, nothing to do", CONFIG_PATH);
                Config::default()
            }
        }
    }

    /// HUD frame for a color: the vanilla one, or the frame added for it.
    pub fn hud_frame(&self, color: &Color) -> Option<f32> {
        color.hud_frame.or_else(|| {
            self.extra_hud_colors
                .iter()
                .position(|c| *c == color.hud)
                .map(|i| 3.0 + i as f32)
        })
    }
}
