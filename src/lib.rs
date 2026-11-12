//! Persona Joker Colors
//!
//! Per-costume Persona look for Joker slots, configured in
//! `sd:/ultimate/persona_joker_colors/config.txt` (see config.rs): rebellion effects,
//! battle HUD gauge, victory bars/lines, victory scene colors and victory theme. Outside Mementos the
//! configured color/theme is forced; on Mementos everything follows the song: the vanilla
//! P3/P4/P5 versions, or the extra P1/P2 IS/P2 EP/SMT versions (mementos.rs).
//!
//! Offsets are for Smash Ultimate 13.0.x (checked against 13.0.5).

mod config;
mod hud_patch;
mod mementos;
mod scene_grade;

use config::{Color, Config, Source};
use core::arch::aarch64::float32x4_t;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use skyline::hooks::{getRegionAddress, Region};

/// Extra diagnostics in the emulator log (served files, forced colors and themes).
pub(crate) const DEBUG_LOG: bool = false;

static CONFIG: std::sync::OnceLock<Config> = std::sync::OnceLock::new();

fn config() -> &'static Config {
    CONFIG.get_or_init(Config::load)
}

pub(crate) unsafe fn text_base() -> usize {
    getRegionAddress(Region::Text) as usize
}

// ---------------------------------------------------------------- stage / fighter lookups

extern "C" {
    #[link_name = "\u{1}_ZN3app14sv_information8stage_idEv"]
    fn current_stage_id() -> i32;
    #[link_name = "\u{1}_ZN3app8lua_bind40FighterManager__get_top_rank_player_implEPNS_14FighterManagerEi"]
    fn get_top_rank_player(manager: *const u8, rank: i32) -> i32;
}

/// Jack_Mementoes (0x148), End_Jack_Mementoes (0x149) and Battle_Jack_Mementoes (0x14A):
/// there the vanilla song-based P3/P4/P5 colors stay untouched.
fn stage_is_mementos() -> bool {
    matches!(unsafe { current_stage_id() }, 0x148..=0x14a)
}

/// Whether the last battle was fought on Mementos. Recorded during the battle, because on
/// the victory screen the current stage is already the result stage.
static LAST_BATTLE_ON_MEMENTOS: AtomicBool = AtomicBool::new(false);

fn on_mementos() -> bool {
    stage_is_mementos() || LAST_BATTLE_ON_MEMENTOS.load(Ordering::Relaxed)
}

/// FighterManager instance pointer; entries live at [[PTR]] + entry_id * 8 + 0x20.
const FIGHTER_MANAGER_PTR_OFFSET: usize = 0x52b94f8;

/// `entry` is the FighterEntry; the FighterInformation data pointer is at +0xF8
/// and the costume slot at data + 0x51.
unsafe fn entry_slot(entry: *const u8) -> Option<u8> {
    if entry.is_null() {
        return None;
    }
    let data = *(entry.add(0xf8) as *const *const u8);
    if data.is_null() {
        return None;
    }
    Some(*data.add(0x51))
}

unsafe fn fighter_entry(entry_id: i32) -> *const u8 {
    if !(0..8).contains(&entry_id) {
        return core::ptr::null();
    }
    let manager = *((text_base() + FIGHTER_MANAGER_PTR_OFFSET) as *const *const *const u8);
    if manager.is_null() || (*manager).is_null() {
        return core::ptr::null();
    }
    *((*manager).add(entry_id as usize * 8 + 0x20) as *const *const u8)
}

/// Match info: one 0x3860-byte block per player, fighter kind byte at +0x60.
const RESULT_INFO_OFFSET: usize = 0x1465a30;
const RESULT_PLAYER_STRIDE: usize = 0x3860;

unsafe fn entry_kind(entry_id: i32) -> Option<u32> {
    if !(0..8).contains(&entry_id) {
        return None;
    }
    let result_info: extern "C" fn() -> *const u8 = core::mem::transmute(text_base() + RESULT_INFO_OFFSET);
    let info = result_info();
    if info.is_null() {
        return None;
    }
    Some(*info.add(entry_id as usize * RESULT_PLAYER_STRIDE + 0x60) as u32)
}

/// (fighter kind, costume slot) of a player.
unsafe fn entry_key(entry_id: i32) -> Option<(u32, u8)> {
    Some((entry_kind(entry_id)?, entry_slot(fighter_entry(entry_id))?))
}

fn entry_config(entry_id: i32) -> Option<&'static config::SlotConfig> {
    config().slots.get(&unsafe { entry_key(entry_id) }?)
}

/// Whether a slot's configuration applies right now: always when `fixed`, otherwise only
/// outside Mementos.
fn applies(slot_config: &config::SlotConfig) -> bool {
    slot_config.fixed || !on_mementos()
}

/// Configured color of a player (fighter kind read from the match info, which is reliable
/// on the victory screen).
fn entry_color(entry_id: i32) -> Option<&'static Color> {
    if let Some(extra) = unsafe { winner_extra(entry_id) } {
        return Some(&extra.color);
    }
    entry_config(entry_id).filter(|c| applies(c))?.color.as_ref()
}

/// Configured color of a Joker player. Used from Joker-only code (his rebellion color setup
/// and HUD gauge), where the match info may not be filled in yet.
fn joker_entry_color(entry_id: i32) -> Option<&'static Color> {
    let slot = unsafe { entry_slot(fighter_entry(entry_id)) }?;
    let slot_config = config().slots.get(&(config::FIGHTER_KIND_JACK, slot));
    if let Some(slot_config) = slot_config.filter(|c| c.fixed) {
        return slot_config.color.as_ref();
    }
    if let Some(extra) = mementos_extra() {
        return Some(&extra.color);
    }
    slot_config.filter(|c| applies(c))?.color.as_ref()
}

/// Settings of the extra Mementos version (P1 / P2 IS / P2 EP / SMT) when on Mementos with
/// one of its songs.
fn mementos_extra() -> Option<&'static mementos::ExtraConfig> {
    if !on_mementos() {
        return None;
    }
    config().mementos.get(&mementos::current()?)
}

/// After a Mementos battle with an extra version, the winner follows it when it is Joker or a
/// configured fighter (using Joker's victory scene through CSK's result.stage) without `fixed`.
unsafe fn winner_extra(winner: i32) -> Option<&'static mementos::ExtraConfig> {
    let slot_config = entry_config(winner);
    if slot_config.map_or(false, |c| c.fixed) {
        return None;
    }
    let is_joker = entry_kind(winner) == Some(config::FIGHTER_KIND_JACK);
    if !is_joker && slot_config.is_none() {
        return None;
    }
    mementos_extra()
}

unsafe fn winner_entry() -> Option<i32> {
    let manager = *((text_base() + FIGHTER_MANAGER_PTR_OFFSET) as *const *const u8);
    if manager.is_null() {
        return None;
    }
    let winner = get_top_rank_player(manager, 0);
    (0..8).contains(&winner).then_some(winner)
}

/// Mementos version picked from the song (0 = P3, 1 = P4, 2 = P5), recorded from the
/// stage's own calculation so non-Joker winners can follow it on the victory screen.
const MEMENTOS_VARIANT_OFFSET: usize = 0x2a2ae70;
const MEMENTOS_VARIANT_PROLOGUE: [u8; 16] = [
    0xff, 0x03, 0x01, 0xd1, 0xf4, 0x4f, 0x02, 0xa9, 0xfd, 0x7b, 0x03, 0xa9, 0xfd, 0xc3, 0x00, 0x91,
];
static LAST_MEMENTOS_VARIANT: AtomicUsize = AtomicUsize::new(2);

#[skyline::hook(offset = MEMENTOS_VARIANT_OFFSET)]
unsafe fn mementos_variant(stage_kind: u32, bgm: u64) -> u32 {
    let original = original!()(stage_kind, bgm);
    let variant = match stage_kind {
        0x148..=0x14a => mementos::pick(bgm).unwrap_or(original),
        _ => original,
    };
    LAST_MEMENTOS_VARIANT.store(variant as usize, Ordering::Relaxed);
    variant
}

/// Converts a song's special_category hash to the game's category kind (0xE/0xF/0x10 for
/// the P5/P4/P3 Mementos songs).
const CATEGORY_KIND_OFFSET: usize = 0x3255e10;
const CATEGORY_KIND_PROLOGUE: [u8; 16] = [
    0xe8, 0x03, 0x00, 0xaa, 0x69, 0x69, 0x94, 0xd2, 0x89, 0x36, 0xaf, 0xf2, 0x08, 0x9d, 0x40, 0x92,
];

#[skyline::hook(offset = CATEGORY_KIND_OFFSET)]
unsafe fn category_kind(hash: u64) -> u32 {
    original!()(mementos::vanilla_category(hash))
}

/// Looks up the stream set of a song for a stage: (bgm db, ui_bgm_id, ui_stage_id).
const SONG_FOR_STAGE_OFFSET: usize = 0x325c7c0;
const SONG_FOR_STAGE_PROLOGUE: [u8; 16] = [
    0xf6, 0x57, 0xbd, 0xa9, 0xf4, 0x4f, 0x01, 0xa9, 0xfd, 0x7b, 0x02, 0xa9, 0xfd, 0x83, 0x00, 0x91,
];

#[skyline::hook(offset = SONG_FOR_STAGE_OFFSET)]
unsafe fn song_for_stage(db: *const u8, bgm: u64, stage: u64) -> u64 {
    mementos::note_song(bgm);
    original!()(db, bgm, stage)
}

// ---------------------------------------------------------------- Persona color function

/// app::? get_player_persona_color(FighterEntry*, bool second) -> Vector4f
/// Picks color index 9 / 10 / 11 when the match BGM is a Persona 3 / 4 / 5 song,
/// otherwise the player's team / port color.
const GET_PLAYER_COLOR_OFFSET: usize = 0x65d980;
const GET_PLAYER_COLOR_PROLOGUE: [u8; 16] = [
    0xff, 0x43, 0x01, 0xd1, 0xf6, 0x57, 0x02, 0xa9, 0xf4, 0x4f, 0x03, 0xa9, 0xfd, 0x7b, 0x04, 0xa9,
];
/// Global holding the color table: [[[PTR] + 0x1260] + 0x178] + index * 0x18
const COLOR_TABLE_PTR_OFFSET: usize = 0x52bc3b0;

/// Return addresses (text offsets) of the get_player_color calls made by Joker-only
/// code: FighterSpecializer_Jack's color setup (both colors) and the Joker result stage.
const JOKER_CALLERS: [usize; 3] = [0xb30038, 0xb30094, 0x14810dc];

unsafe fn table_color(index: usize, second: bool) -> Option<[f32; 3]> {
    let root = *((text_base() + COLOR_TABLE_PTR_OFFSET) as *const *const u8);
    if root.is_null() {
        return None;
    }
    let holder = *(root.add(0x1260) as *const *const u8);
    if holder.is_null() {
        return None;
    }
    let table = *(holder.add(0x178) as *const *const f32);
    if table.is_null() {
        return None;
    }
    let entry = table.add(index * 6 + if second { 3 } else { 0 });
    Some([*entry, *entry.add(1), *entry.add(2)])
}

/// Effect color of a configured color. The second (highlight) color of custom colors moves
/// the mid channels halfway to 1, like the game's Persona entries (0.4 -> 0.7, 0.6 -> 0.8).
unsafe fn effect_color(color: &Color, second: bool) -> Option<[f32; 3]> {
    match color.effect {
        Source::Table(index) => table_color(index, second),
        Source::Custom(rgb) if second => Some(rgb.map(|c| if c > 0.1 && c < 0.9 { c + (1.0 - c) * 0.5 } else { c })),
        Source::Custom(rgb) => Some(rgb),
    }
}

// The game returns the color packed in q0 (Vector4f), which float32x4_t matches.
#[allow(improper_ctypes_definitions)]
#[skyline::hook(offset = GET_PLAYER_COLOR_OFFSET)]
unsafe fn get_player_color(entry: *const u8, second: bool) -> float32x4_t {
    let caller: usize;
    core::arch::asm!("mov {}, x30", out(reg) caller);
    let caller = caller.wrapping_sub(text_base());
    if caller == JOKER_CALLERS[0] {
        // FighterSpecializer_Jack's setup runs with the battle stage loaded.
        LAST_BATTLE_ON_MEMENTOS.store(stage_is_mementos(), Ordering::Relaxed);
    }
    let forced = if JOKER_CALLERS.contains(&caller) && !entry.is_null() {
        joker_entry_color(*(entry.add(0xe0) as *const i32)).and_then(|c| effect_color(c, second))
    } else {
        None
    };
    let original = original!()(entry, second);
    if DEBUG_LOG && forced.is_some() {
        println!("[persona_joker_colors] color from {:#x} slot={:?} second={} -> {:?}", caller, entry_slot(entry), second, forced);
    }
    match forced {
        Some([r, g, b]) => core::mem::transmute([r, g, b, 1.0f32]),
        None => original,
    }
}

// ---------------------------------------------------------------- victory bars / lines

/// Stage manager instance pointer; [[PTR]] + 0x128 has the set_param(id, f32) vfunc at +0x238.
const STAGE_MANAGER_PTR_OFFSET: usize = 0x532a9d8;
const STAGE_SET_PARAM_VTABLE_OFFSET: usize = 0x238;
/// Result-stage state callback that sets ResultStage_Jack's color params (0x22..0x25).
const JACK_RESULT_STAGE_SETUP_OFFSET: usize = 0x1481030;
const JACK_RESULT_STAGE_SETUP_PROLOGUE: [u8; 16] = [
    0xff, 0xc3, 0x00, 0xd1, 0xf4, 0x4f, 0x01, 0xa9, 0xfd, 0x7b, 0x02, 0xa9, 0xfd, 0x83, 0x00, 0x91,
];
const STAGE_KIND_RESULT_JACK: u32 = 0x14b;
const RESULT_PARAM_USE_COLOR: u32 = 0x22;
const RESULT_PARAM_COLOR_R: u32 = 0x23;

type SetStageParam = extern "C" fn(*const u8, u32, f32);

/// The vanilla setup only applies the Persona color for one of the three win poses
/// (and only when a Persona song played); for configured slots apply it for every pose.
#[skyline::hook(offset = JACK_RESULT_STAGE_SETUP_OFFSET)]
unsafe fn jack_result_stage_setup(state: *mut u8) {
    original!()(state);
    if *(state.add(0xb8) as *const u32) != STAGE_KIND_RESULT_JACK {
        return;
    }
    let winner = *(state.add(0x16c) as *const i32);
    let Some(color) = entry_color(winner).and_then(|c| effect_color(c, false)) else {
        return;
    };
    let manager = *((text_base() + STAGE_MANAGER_PTR_OFFSET) as *const *const *const u8);
    if manager.is_null() || (*manager).is_null() {
        return;
    }
    let params = (*manager).add(0x128);
    let vtable = *(params as *const *const SetStageParam);
    let set_param = *vtable.add(STAGE_SET_PARAM_VTABLE_OFFSET / 8);
    set_param(params, RESULT_PARAM_USE_COLOR, 1.0);
    for (i, value) in color.iter().enumerate() {
        set_param(params, RESULT_PARAM_COLOR_R + i as u32, *value);
    }
}

// ---------------------------------------------------------------- victory scene

/// Look of the winner's victory scene, if it differs from what the song would pick.
enum SceneLook {
    /// One of the game's own scene versions, as a song category kind (0x10 P3, 0xF P4, 0xE P5).
    Vanilla(u32),
    /// The P3 version with its color grades pointed at this color (scene_grade).
    Tinted(&'static Color),
}

const CATEGORY_KIND_P3: u32 = 0x10;
const CATEGORY_KIND_P4: u32 = 0xf;
const CATEGORY_KIND_P5: u32 = 0xe;

fn scene_look(color: &'static Color) -> SceneLook {
    match color.effect {
        Source::Table(9) => SceneLook::Vanilla(CATEGORY_KIND_P3),
        Source::Table(10) => SceneLook::Vanilla(CATEGORY_KIND_P4),
        Source::Table(_) => SceneLook::Vanilla(CATEGORY_KIND_P5),
        Source::Custom(_) => SceneLook::Tinted(color),
    }
}

/// Outside Mementos (or with `fixed`): the winner's configured color. After a Mementos
/// battle, configured non-Joker fighters (using Joker's victory scene through CSK's
/// result.stage) follow the Mementos version; Joker himself is handled by the game.
unsafe fn winner_scene() -> Option<SceneLook> {
    let winner = winner_entry()?;
    if let Some(extra) = winner_extra(winner) {
        return Some(scene_look(&extra.color));
    }
    let slot_config = entry_config(winner)?;
    if applies(slot_config) {
        return Some(scene_look(slot_config.color.as_ref()?));
    }
    let (kind, _) = entry_key(winner)?;
    if kind == config::FIGHTER_KIND_JACK {
        return None;
    }
    Some(SceneLook::Vanilla(match LAST_MEMENTOS_VARIANT.load(Ordering::Relaxed) {
        0 => CATEGORY_KIND_P3,
        1 => CATEGORY_KIND_P4,
        _ => CATEGORY_KIND_P5,
    }))
}

/// Constructor of the result screen scene. The match info's +0x1C578 holds the P3/P4/P5
/// song of a Mementos match (set when the match starts, 0 elsewhere), and the victory scene
/// (sky and ground lighting, fighter light, bars, Joker's jingle) uses its version; without
/// one it is the plain P5 scene. The winner's version is written there before the scene
/// reads it.
const RESULT_SCENE_CTOR_OFFSET: usize = 0x143f930;
const RESULT_SCENE_CTOR_PROLOGUE: [u8; 16] = [
    0xff, 0x83, 0x05, 0xd1, 0xfc, 0x6f, 0x10, 0xa9, 0xfa, 0x67, 0x11, 0xa9, 0xf8, 0x5f, 0x12, 0xa9,
];
const RESULT_PERSONA_SONG: usize = 0x1c578;
/// Type byte of a ui_bgm_id value.
const BGM_ID_TAG: u64 = 0xd9 << 56;

#[skyline::hook(offset = RESULT_SCENE_CTOR_OFFSET)]
unsafe fn result_scene_ctor(this: *mut u8) {
    set_result_persona_song();
    original!()(this)
}

unsafe fn set_result_persona_song() {
    // The scene is built when a match ends (before the game enters result mode); only then
    // is there a winner to ask for.
    if !BATTLE_RUNNING.swap(false, Ordering::Relaxed) {
        if DEBUG_LOG {
            println!("[persona_joker_colors] victory scene: no match result yet");
        }
        return;
    }
    // Custom colors use the P3 scene with its color grades rewritten (scene_grade).
    let scene = winner_scene();
    scene_grade::set_current(match scene {
        Some(SceneLook::Tinted(color)) => Some(color),
        _ => None,
    });
    let kind = match scene {
        Some(SceneLook::Vanilla(kind)) => kind,
        Some(SceneLook::Tinted(_)) => CATEGORY_KIND_P3,
        None => return,
    };
    let Some(song) = mementos::persona_song((kind - CATEGORY_KIND_P5) as usize) else {
        return;
    };
    let song = BGM_ID_TAG | song;
    let result_info: extern "C" fn() -> *mut u8 = core::mem::transmute(text_base() + RESULT_INFO_OFFSET);
    let info = result_info();
    if info.is_null() {
        return;
    }
    *(info.add(RESULT_PERSONA_SONG) as *mut u64) = song;
    if DEBUG_LOG {
        println!("[persona_joker_colors] victory scene song {:#x}", song);
    }
}

// ---------------------------------------------------------------- victory lines

/// The "DNA" tubes behind the winner (Blood_vessel material). Only vanilla Joker costumes
/// keep them: the model is served with no mesh entries for everyone else.
const RESULT_LINES_MODEL: &str = "stage/resultstage_jack/normal/model/resultstage_effect_set/model.numdlb";
const RESULT_LINES_MODEL_SIZE: usize = 0x1000;
/// MODL (SSBH) entry list: relative offset at 0x48, count at 0x50.
const MODL_ENTRY_COUNT: usize = 0x50;
const VANILLA_JOKER_SLOTS: u8 = 8;

unsafe fn winner_keeps_lines() -> bool {
    let Some((kind, slot)) = winner_entry().and_then(|w| entry_key(w)) else {
        return true;
    };
    kind == config::FIGHTER_KIND_JACK && slot < VANILLA_JOKER_SLOTS
}

extern "C" fn result_lines_callback(hash: u64, out: *mut u8, length: usize, out_size: &mut usize) -> bool {
    if unsafe { winner_keeps_lines() } {
        return false;
    }
    let buffer = unsafe { core::slice::from_raw_parts_mut(out, length) };
    let Some(size) = arcropolis_api::load_original_file(hash, &mut *buffer) else {
        return false;
    };
    if size < MODL_ENTRY_COUNT + 8 || &buffer[..4] != b"HBSS" || &buffer[0x10..0x14] != b"LDOM" {
        println!("[persona_joker_colors] victory lines model not patched (unexpected format)");
        return false;
    }
    buffer[MODL_ENTRY_COUNT..MODL_ENTRY_COUNT + 8].fill(0);
    *out_size = size;
    if DEBUG_LOG {
        println!("[persona_joker_colors] victory lines hidden");
    }
    true
}

// ---------------------------------------------------------------- battle HUD

/// Battle HUD (info_melee). One 0xD90-byte block per player is constructed by HUD_CTOR; the
/// Joker animation holder jack_bg (gauge outline and portrait backdrop) lives inside it. Its
/// frame picks the color: 0 = P5 red, 1 = P3 blue, 2 = P4 yellow, 3.. = colors added by
/// hud_patch. The gauge fill (jack_guage_color: 0 = purple, 1 = aqua while the Persona is
/// out) is left to the game.
const HUD_CTOR_OFFSET: usize = 0x1b635a0;
const HUD_CTOR_PROLOGUE: [u8; 16] = [
    0xff, 0x83, 0x01, 0xd1, 0xf9, 0x0b, 0x00, 0xf9, 0xf8, 0x5f, 0x02, 0xa9, 0xf6, 0x57, 0x03, 0xa9,
];
/// HUD animation helper taking the holder: set_frame(holder, frame).
const ANIM_SET_FRAME_OFFSET: usize = 0x377b010;
const ANIM_SET_FRAME_PROLOGUE: [u8; 16] = [
    0xe8, 0x0f, 0x1c, 0xfc, 0xf6, 0x57, 0x01, 0xa9, 0xf4, 0x4f, 0x02, 0xa9, 0xfd, 0x7b, 0x03, 0xa9,
];
const HUD_PLAYER_STRIDE: usize = 0xd90;
/// The HUD layout code addresses it as hud + 0xED0, and the player blocks start at
/// hud + 0x200, so relative to each player block it is at +0xCD0.
const HUD_JACK_BG_HOLDER: usize = 0xcd0;

static HUD_THIS: AtomicUsize = AtomicUsize::new(0);
/// Set when a match's HUD is built, cleared when its result scene is.
static BATTLE_RUNNING: AtomicBool = AtomicBool::new(false);

/// The constructor runs once per player block (8 contiguous blocks, 0xD90 apart);
/// HUD_THIS keeps the first block of the current series, i.e. player 0.
#[skyline::hook(offset = HUD_CTOR_OFFSET)]
unsafe fn hud_ctor(this: *mut u8, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64, a7: u64) {
    original!()(this, a1, a2, a3, a4, a5, a6, a7);
    let block = this as usize;
    let base = HUD_THIS.load(Ordering::Relaxed);
    let continues_series = base != 0
        && block > base
        && (block - base) % HUD_PLAYER_STRIDE == 0
        && (block - base) / HUD_PLAYER_STRIDE < 8;
    if !continues_series {
        HUD_THIS.store(block, Ordering::Relaxed);
        BATTLE_RUNNING.store(true, Ordering::Relaxed);
        LAST_BATTLE_ON_MEMENTOS.store(stage_is_mementos(), Ordering::Relaxed);
    }
}

/// If `holder` is the jack_bg animation of a configured player's HUD block, its frame.
unsafe fn forced_hud_frame(holder: *const u8) -> Option<f32> {
    let hud = HUD_THIS.load(Ordering::Relaxed);
    let holder = holder as usize;
    if hud == 0 || holder < hud {
        return None;
    }
    let rel = holder - hud;
    let player = rel / HUD_PLAYER_STRIDE;
    if player >= 8 || rel - player * HUD_PLAYER_STRIDE != HUD_JACK_BG_HOLDER {
        return None;
    }
    config().hud_frame(joker_entry_color(player as i32)?)
}

#[skyline::hook(offset = ANIM_SET_FRAME_OFFSET)]
unsafe fn anim_set_frame(holder: *mut u8, frame: f32) {
    original!()(holder, forced_hud_frame(holder).unwrap_or(frame))
}

/// Serves the HUD layout with one extra color frame per custom configured color.
extern "C" fn hud_layout_callback(hash: u64, out: *mut u8, length: usize, out_size: &mut usize) -> bool {
    // Loaded straight into the game's buffer (it can be tens of MB with HUD mods) and
    // patched there; the current file, including mods such as Cuphead's HUD, is the base.
    let buffer = unsafe { core::slice::from_raw_parts_mut(out, length) };
    let Some(size) = arcropolis_api::load_original_file(hash, &mut *buffer) else {
        println!("[persona_joker_colors] could not load the HUD layout");
        return false;
    };
    let size = match hud_patch::patch_layout_in_place(buffer, size, &config().extra_hud_colors) {
        Some(patched) => patched,
        None => {
            println!("[persona_joker_colors] HUD layout not patched (unexpected format)");
            size
        }
    };
    *out_size = size;
    if DEBUG_LOG {
        println!("[persona_joker_colors] HUD layout served ({} bytes, {} extra colors)", size, config().extra_hud_colors.len());
    }
    true
}

/// Room for the archive (vanilla 2.1 MB, Cuphead's HUD mod 54.5 MB) plus the appended frames.
const HUD_LAYOUT_BUFFER: usize = 64 * 1024 * 1024;

// ---------------------------------------------------------------- victory theme

/// Stream playback (the function One-Slot-Victory-Theme / CSK hook too):
/// (player, ?, nus3bank_hash, *nus3audio_hash, nus3audio_index).
/// Joker slots use Joker's jingle set (z97a, with z97b/z97c as its P4/P3 variants), so on
/// Mementos they follow the stage like Joker; elsewhere a configured theme replaces it.
const MUSIC_PLAY_OFFSET: usize = 0x373ca60;
const MUSIC_PLAY_PROLOGUE: [u8; 16] = [
    0xfc, 0x6f, 0xba, 0xa9, 0xfa, 0x67, 0x01, 0xa9, 0xf8, 0x5f, 0x02, 0xa9, 0xf6, 0x57, 0x03, 0xa9,
];
const JOKER_JINGLES: [&str; 3] = ["bgm_z97a_f_jack_p5", "bgm_z97b_f_jack_p4", "bgm_z97c_f_jack_p3"];

fn stream_hash(name: &str, ext: &str) -> u64 {
    arcropolis_api::hash40(&format!("stream:/sound/bgm/{}.{}", name, ext)).as_u64()
}

static mut FORCED_AUDIO_HASH: u64 = 0;

#[skyline::hook(offset = MUSIC_PLAY_OFFSET)]
unsafe fn music_play(player: *mut u64, param_2: i64, nus3bank_hash: u64, nus3audio_hash: *const u64, index: usize) {
    let mut bank = nus3bank_hash;
    let mut audio = nus3audio_hash;
    let mut index = index;
    let joker_jingle = !audio.is_null() && JOKER_JINGLES.iter().any(|j| stream_hash(j, "nus3audio") == *audio);
    let winner = if joker_jingle { winner_entry() } else { None };
    if let Some(extra) = winner.and_then(|w| winner_extra(w)) {
        FORCED_AUDIO_HASH = stream_hash(&extra.theme, "nus3audio");
        if DEBUG_LOG {
            println!("[persona_joker_colors] jingle {:#x} -> {} (Mementos)", *audio, extra.theme);
        }
        if JOKER_JINGLES.iter().any(|j| stream_hash(j, "nus3bank") == nus3bank_hash) {
            bank = stream_hash(&extra.theme, "nus3bank");
        }
        return original!()(player, param_2, bank, core::ptr::addr_of!(FORCED_AUDIO_HASH), 0);
    }
    let slot_config = winner.and_then(entry_config);
    if let (Some(winner), Some(slot_config)) = (winner, slot_config) {
        if !applies(slot_config) && entry_key(winner).map_or(false, |(kind, _)| kind != config::FIGHTER_KIND_JACK) {
            // Configured non-Joker fighters follow the Mementos version with its own track 0;
            // per-costume indexes in victory.toml are meant for Joker costumes of that number.
            index = 0;
        }
    }
    if let Some(slot_config) = slot_config.filter(|c| applies(c)) {
        let theme = slot_config.theme.as_deref();
        if DEBUG_LOG {
            println!("[persona_joker_colors] jingle {:#x} index={} -> {:?}", *audio, index, theme);
        }
        if let Some(theme) = theme {
            FORCED_AUDIO_HASH = stream_hash(theme, "nus3audio");
            audio = core::ptr::addr_of!(FORCED_AUDIO_HASH);
            if JOKER_JINGLES.iter().any(|j| stream_hash(j, "nus3bank") == nus3bank_hash) {
                bank = stream_hash(theme, "nus3bank");
            }
            // Per-costume track indexes (victory.toml) refer to the replaced file.
            index = 0;
        }
    }
    original!()(player, param_2, bank, audio, index)
}

// ---------------------------------------------------------------- setup

unsafe fn code_matches(offset: usize, expected: &[u8; 16]) -> bool {
    core::slice::from_raw_parts((text_base() + offset) as *const u8, 16) == expected
}

#[skyline::main(name = "persona_joker_colors")]
pub fn main() {
    unsafe {
        if !code_matches(GET_PLAYER_COLOR_OFFSET, &GET_PLAYER_COLOR_PROLOGUE)
            || !code_matches(JACK_RESULT_STAGE_SETUP_OFFSET, &JACK_RESULT_STAGE_SETUP_PROLOGUE)
            || !code_matches(HUD_CTOR_OFFSET, &HUD_CTOR_PROLOGUE)
            || !code_matches(ANIM_SET_FRAME_OFFSET, &ANIM_SET_FRAME_PROLOGUE)
            || !code_matches(MUSIC_PLAY_OFFSET, &MUSIC_PLAY_PROLOGUE)
            || !code_matches(MEMENTOS_VARIANT_OFFSET, &MEMENTOS_VARIANT_PROLOGUE)
            || !code_matches(CATEGORY_KIND_OFFSET, &CATEGORY_KIND_PROLOGUE)
            || !code_matches(SONG_FOR_STAGE_OFFSET, &SONG_FOR_STAGE_PROLOGUE)
            || !code_matches(RESULT_SCENE_CTOR_OFFSET, &RESULT_SCENE_CTOR_PROLOGUE)
            || !code_matches(scene_grade::READ_GRADE_OFFSET, &scene_grade::READ_GRADE_PROLOGUE)
        {
            println!("[persona_joker_colors] Unsupported game version, not installing.");
            return;
        }
    }
    println!(
        "[persona_joker_colors] {} configured slots, {} extra HUD colors",
        config().slots.len(),
        config().extra_hud_colors.len()
    );
    skyline::install_hooks!(get_player_color, jack_result_stage_setup, hud_ctor, anim_set_frame, music_play, mementos_variant, category_kind, song_for_stage, result_scene_ctor, scene_grade::read_grade);
    mementos::register_stage_files();
    arcropolis_api::register_callback(
        arcropolis_api::hash40(RESULT_LINES_MODEL),
        RESULT_LINES_MODEL_SIZE,
        result_lines_callback,
    );
    if !config().extra_hud_colors.is_empty() {
        arcropolis_api::register_callback(arcropolis_api::hash40(hud_patch::HUD_LAYOUT), HUD_LAYOUT_BUFFER, hud_layout_callback);
    }
    println!("[persona_joker_colors] Installed.");
}
