//! Color grading of Joker's victory scene for custom colors.
//!
//! The P3/P4/P5 versions of the scene differ only in six color grades of the result light
//! params (`start_*` at frame 0 animated to `end_*` at frame 200, for the fighter, the stage
//! and the effects; `_p3` / `_p4` suffixes, none for P5). Each grade has gain_r/g/b,
//! offset_r/g/b, contrast and saturation. For a custom color the scene is shown as P3 with
//! its six grades rewritten in memory from that color (same strength as P3's blue) right
//! before the game reads them; the originals are put back for the next real P3 win.

use crate::config::Color;
use core::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

/// Reads one grade of the scene: (out, light param resource, target 0 chara / 1 ground /
/// 2 effect, version 0 P3 / 1 P4 / 2 P5). It looks up `start_<target><suffix>` and
/// `end_<target><suffix>` in the resource; [res + 8] is the param object, whose +8 table
/// has the hash array at +0x20 (right after the 0x10-byte paracobn header of the raw file).
pub const READ_GRADE_OFFSET: usize = 0x149e660;
pub const READ_GRADE_PROLOGUE: [u8; 16] = [
    0xff, 0x03, 0x03, 0xd1, 0xf9, 0x3b, 0x00, 0xf9, 0xf8, 0x5f, 0x08, 0xa9, 0xf6, 0x57, 0x09, 0xa9,
];
const VERSION_P3: u32 = 0;

/// Custom color of the current victory scene (null: none), set when the scene is built.
static CURRENT: AtomicPtr<Color> = AtomicPtr::new(core::ptr::null_mut());

pub fn set_current(color: Option<&'static Color>) {
    CURRENT.store(color.map_or(core::ptr::null_mut(), |c| c as *const Color as *mut Color), Ordering::Relaxed);
}

#[skyline::hook(offset = READ_GRADE_OFFSET)]
pub unsafe fn read_grade(out: *mut u8, res: *const u8, target: u32, version: u32) {
    if version == VERSION_P3 {
        let color = CURRENT.load(Ordering::Relaxed);
        let applied = apply(res, color.as_ref());
        if crate::DEBUG_LOG {
            println!("[persona_joker_colors] grade target {} custom {} applied {}", target, !color.is_null(), applied);
        }
    }
    original!()(out, res, target, version)
}

const GRADES: [&str; 6] = [
    "start_chara_p3",
    "start_ground_p3",
    "start_effect_p3",
    "end_chara_p3",
    "end_ground_p3",
    "end_effect_p3",
];
const GAIN: [&str; 3] = ["gain_r", "gain_g", "gain_b"];
const OFFSET: [&str; 3] = ["offset_r", "offset_g", "offset_b"];
const SATURATION: &str = "saturation";

/// Raw paracobn file held by the game.
struct Params {
    file: *mut u8,
    hashes: *const u64,
    hash_count: usize,
    refs: *const u8,
    root: *mut u8,
}

unsafe fn u32_at(p: *const u8) -> usize {
    core::ptr::read_unaligned(p as *const u32) as usize
}

impl Params {
    unsafe fn get(res: *const u8) -> Option<Params> {
        if res.is_null() {
            return None;
        }
        let obj = *(res.add(8) as *const *const u8);
        if obj.is_null() {
            return None;
        }
        let table = *(obj.add(8) as *const *const u8);
        let root = *(obj.add(0x10) as *const *mut u8);
        if table.is_null() || root.is_null() {
            return None;
        }
        let hashes = *(table.add(0x20) as *const *const u8);
        if hashes.is_null() {
            return None;
        }
        let file = hashes.sub(0x10) as *mut u8;
        if core::slice::from_raw_parts(file, 8) != b"paracobn" {
            return None;
        }
        let hash_size = u32_at(file.add(8));
        Some(Params {
            file,
            hashes: hashes as *const u64,
            hash_count: hash_size / 8,
            refs: file.add(0x10 + hash_size),
            root,
        })
    }

    /// Field of a struct node by name.
    unsafe fn field(&self, node: *mut u8, name: &str) -> Option<*mut u8> {
        if *node != 12 {
            return None;
        }
        let hash = arcropolis_api::hash40(name).as_u64();
        let count = u32_at(node.add(1));
        let table = self.refs.add(u32_at(node.add(5)));
        (0..count).find_map(|i| {
            let index = u32_at(table.add(8 * i));
            (index < self.hash_count && *self.hashes.add(index) == hash).then(|| node.add(u32_at(table.add(8 * i + 4))))
        })
    }

    /// The f32 value of a field, as a pointer into the param data.
    unsafe fn float(&self, node: *mut u8, name: &str) -> Option<*mut f32> {
        let value = self.field(node, name)?;
        (*value == 8).then(|| value.add(1) as *mut f32)
    }
}

/// One grade's values: gain rgb, offset rgb, saturation.
type Grade = [f32; 7];

unsafe fn grade_fields(params: &Params, grade: &str) -> Option<[*mut f32; 7]> {
    let node = params.field(params.root, grade)?;
    let mut out = [core::ptr::null_mut(); 7];
    for (i, name) in GAIN.iter().chain(OFFSET.iter()).chain([SATURATION].iter()).enumerate() {
        out[i] = params.float(node, name)?;
    }
    Some(out)
}

/// Original P3 grades, saved the first time they are rewritten.
static ORIGINAL: Mutex<Option<(usize, [Grade; 6])>> = Mutex::new(None);

unsafe fn fields(res: *const u8) -> Option<(usize, [[*mut f32; 7]; 6])> {
    let params = Params::get(res)?;
    let mut all = [[core::ptr::null_mut(); 7]; 6];
    for (i, grade) in GRADES.iter().enumerate() {
        all[i] = grade_fields(&params, grade)?;
    }
    Some((params.file as usize, all))
}

/// Grade with the strength of P3's blue, pointed at `color` instead (a bit softer, since
/// P3's own values are tuned for a single hue).
fn tinted(original: &Grade, color: &Color) -> Grade {
    let rgb = color.hud.map(|c| c / 255.0);
    let max = rgb.iter().cloned().fold(0.0f32, f32::max);
    let min = rgb.iter().cloned().fold(1.0f32, f32::min);
    let direction = if max > 0.0 { rgb.map(|c| c / max) } else { [1.0; 3] };
    // Dark colors dim the scene, but not to black.
    let brightness = max.max(0.35);
    let chroma = if max > 0.0 { (max - min) / max } else { 0.0 };
    let gain = original[..3].iter().cloned().fold(0.0f32, f32::max) * brightness;
    let offset = original[3..6].iter().cloned().fold(0.0f32, f32::max) * brightness;
    let mut out = *original;
    for i in 0..3 {
        let weight = 0.3 + 0.7 * direction[i];
        out[i] = gain * weight;
        out[3 + i] = offset * direction[i];
    }
    // White, grey and black keep little color.
    out[6] = original[6] * (0.3 + 0.55 * chroma);
    out
}

/// Rewrites the P3 grades for `color` (Some) or restores the originals (None). Returns
/// whether the params were found.
unsafe fn apply(res: *const u8, color: Option<&Color>) -> bool {
    let Some((file, fields)) = fields(res) else {
        return false;
    };
    let Ok(mut original) = ORIGINAL.lock() else {
        return false;
    };
    // The param file can be reloaded; only trust saved values taken from this copy.
    if original.map_or(true, |(saved_file, _)| saved_file != file) {
        let mut values = [[0.0; 7]; 6];
        for (grade, ptrs) in values.iter_mut().zip(fields.iter()) {
            for (value, ptr) in grade.iter_mut().zip(ptrs.iter()) {
                *value = core::ptr::read_unaligned(*ptr);
            }
        }
        *original = Some((file, values));
    }
    let (_, values) = original.unwrap();
    for (grade, ptrs) in values.iter().zip(fields.iter()) {
        let new = match color {
            Some(color) => tinted(grade, color),
            None => *grade,
        };
        for (value, ptr) in new.iter().zip(ptrs.iter()) {
            core::ptr::write_unaligned(*ptr, *value);
        }
    }
    true
}
