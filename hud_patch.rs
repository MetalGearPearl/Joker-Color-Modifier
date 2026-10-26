//! Adds extra color frames to the battle HUD's Joker color animations.
//!
//! `jack_bg` (portrait backdrop) and `jack_guage_color` (rebellion gauge) only have frames
//! 0 = P5 red, 1 = P3 blue, 2 = P4 yellow. For each extra configured color we append one
//! stepped frame: jack_bg gets the color itself, jack_guage_color repeats its non-P5 frame.
//! The patched bflans are appended at the end of the SARC (layout.arc) and the file table
//! is pointed at them, so nothing else in the archive moves.

use std::convert::TryInto;

pub const HUD_LAYOUT: &str = "ui/layout/info/info_melee/info_melee/layout.arc";
const JACK_BG: &str = "anim/p2_info_melee_lct_jack_jack_bg.bflan";
const JACK_GAUGE_COLOR: &str = "anim/p2_info_melee_lct_jack_jack_guage_color.bflan";

fn u16_at(d: &[u8], o: usize) -> usize {
    u16::from_le_bytes(d[o..o + 2].try_into().unwrap()) as usize
}
fn u32_at(d: &[u8], o: usize) -> usize {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize
}
fn f32_at(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn put_u32(d: &mut [u8], o: usize, v: usize) {
    d[o..o + 4].copy_from_slice(&(v as u32).to_le_bytes());
}

struct Target {
    head: [u8; 4], // index, target, curve type, pad
    keys: Vec<[f32; 3]>,
}
struct Info {
    magic: [u8; 4],
    targets: Vec<Target>,
}
struct Content {
    name: [u8; 28],
    kind: u8,
    infos: Vec<Info>,
}

/// Rebuilds a FLAN file, letting `edit` change the pai1 contents and frame count.
fn rebuild_bflan(d: &[u8], edit: impl Fn(&mut Vec<Content>) -> u16) -> Option<Vec<u8>> {
    let header_len = u16_at(d, 6);
    let section_count = u16_at(d, 0x10);
    let mut out = d[..header_len].to_vec();
    let mut i = header_len;
    for _ in 0..section_count {
        let size = u32_at(d, i + 4);
        let section = &d[i..i + size];
        if &section[..4] == b"pai1" {
            out.extend(rebuild_pai1(section, &edit)?);
        } else {
            out.extend_from_slice(section);
        }
        i += size;
    }
    let len = out.len();
    put_u32(&mut out, 0xc, len);
    Some(out)
}

fn rebuild_pai1(s: &[u8], edit: &impl Fn(&mut Vec<Content>) -> u16) -> Option<Vec<u8>> {
    let loop_flag = s[0xa];
    if u16_at(s, 0xc) != 0 {
        return None; // timg file references are not used by these animations
    }
    let content_count = u16_at(s, 0xe);
    let table = u32_at(s, 0x10);
    let mut contents = Vec::new();
    for c in 0..content_count {
        let o = u32_at(s, table + 4 * c);
        let info_count = s[o + 28] as usize;
        let mut infos = Vec::new();
        for ii in 0..info_count {
            let b = o + u32_at(s, o + 32 + 4 * ii);
            let target_count = s[b + 4] as usize;
            let mut targets = Vec::new();
            for ti in 0..target_count {
                let t = b + u32_at(s, b + 8 + 4 * ti);
                if s[t + 2] != 2 {
                    return None; // only hermite curves are expected here
                }
                let key_count = u16_at(s, t + 4);
                let k0 = t + u32_at(s, t + 8);
                let keys = (0..key_count)
                    .map(|k| [f32_at(s, k0 + 12 * k), f32_at(s, k0 + 12 * k + 4), f32_at(s, k0 + 12 * k + 8)])
                    .collect();
                targets.push(Target { head: s[t..t + 4].try_into().unwrap(), keys });
            }
            infos.push(Info { magic: s[b..b + 4].try_into().unwrap(), targets });
        }
        contents.push(Content { name: s[o..o + 28].try_into().unwrap(), kind: s[o + 29], infos });
    }

    let frame_size = edit(&mut contents);

    let mut body = Vec::new();
    let header_len = table + 4 * contents.len();
    let mut offsets = Vec::new();
    for content in &contents {
        offsets.push(header_len + body.len());
        let mut c = content.name.to_vec();
        c.extend([content.infos.len() as u8, content.kind, 0, 0]);
        c.resize(c.len() + 4 * content.infos.len(), 0);
        for (ii, info) in content.infos.iter().enumerate() {
            let at = c.len();
            put_u32(&mut c, 32 + 4 * ii, at);
            let mut inf = info.magic.to_vec();
            inf.extend([info.targets.len() as u8, 0, 0, 0]);
            inf.resize(inf.len() + 4 * info.targets.len(), 0);
            for (ti, target) in info.targets.iter().enumerate() {
                let at = inf.len();
                put_u32(&mut inf, 8 + 4 * ti, at);
                inf.extend(target.head);
                inf.extend((target.keys.len() as u16).to_le_bytes());
                inf.extend([0, 0]);
                inf.extend(12u32.to_le_bytes());
                for key in &target.keys {
                    for v in key {
                        inf.extend(v.to_le_bytes());
                    }
                }
            }
            c.extend(inf);
        }
        body.extend(c);
    }
    let mut out = b"pai1\0\0\0\0".to_vec();
    out.extend(frame_size.to_le_bytes());
    out.extend([loop_flag, 0]);
    out.extend(0u16.to_le_bytes());
    out.extend((contents.len() as u16).to_le_bytes());
    out.extend((table as u32).to_le_bytes());
    out.resize(table, 0);
    for o in offsets {
        out.extend((o as u32).to_le_bytes());
    }
    out.extend(body);
    let len = out.len();
    put_u32(&mut out, 4, len);
    Some(out)
}

/// Appends one stepped frame per value in `values(target, keys)` to every target.
fn append_frames(contents: &mut Vec<Content>, values: impl Fn(u8, &[[f32; 3]]) -> Vec<f32>) -> u16 {
    let mut last = 0.0f32;
    for content in contents.iter_mut() {
        for info in content.infos.iter_mut() {
            for target in info.targets.iter_mut() {
                let new = values(target.head[1], &target.keys);
                let Some(&[mut frame, mut value, _]) = target.keys.last() else { continue };
                for v in new {
                    frame += 1.0;
                    target.keys.push([frame, value, 0.0]);
                    target.keys.push([frame, v, 0.0]);
                    value = v;
                }
                last = last.max(frame);
            }
        }
    }
    last as u16
}

/// Value a stepped target holds from `frame` on (the last key at that frame).
fn value_at(keys: &[[f32; 3]], frame: f32) -> f32 {
    keys.iter().filter(|k| k[0] <= frame).last().map(|k| k[1]).unwrap_or(0.0)
}

struct Sarc<'a> {
    data: &'a [u8],
    data_offset: usize,
    nodes: usize,
    node_count: usize,
    names: usize,
}

impl<'a> Sarc<'a> {
    fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < 0x20 || &data[..4] != b"SARC" || u16_at(data, 6) != 0xfeff {
            return None;
        }
        let sfat = u16_at(data, 4);
        if &data[sfat..sfat + 4] != b"SFAT" {
            return None;
        }
        let node_count = u16_at(data, sfat + 6);
        let nodes = sfat + u16_at(data, sfat + 4);
        let sfnt = nodes + 16 * node_count;
        if &data[sfnt..sfnt + 4] != b"SFNT" {
            return None;
        }
        Some(Sarc { data, data_offset: u32_at(data, 0xc), nodes, node_count, names: sfnt + u16_at(data, sfnt + 4) })
    }

    /// (node offset, file start, file end) of a named file.
    fn find(&self, name: &str) -> Option<(usize, usize, usize)> {
        (0..self.node_count).map(|n| self.nodes + 16 * n).find_map(|node| {
            let attrs = u32_at(self.data, node + 4);
            if attrs >> 24 == 0 {
                return None;
            }
            let at = self.names + (attrs & 0xff_ffff) * 4;
            let end = self.data[at..].iter().position(|&b| b == 0)? + at;
            (&self.data[at..end] == name.as_bytes()).then(|| {
                (node, self.data_offset + u32_at(self.data, node + 8), self.data_offset + u32_at(self.data, node + 12))
            })
        })
    }
}

/// Patches the HUD layout archive held in `buffer[..size]` in place, appending the rebuilt
/// animations after it. Returns the new size, or None when the archive does not look like
/// the expected one or does not fit (then the buffer is left untouched).
pub fn patch_layout_in_place(buffer: &mut [u8], size: usize, extra_colors: &[[f32; 3]]) -> Option<usize> {
    let (bg_node, bg, gauge_node, gauge, data_offset) = {
        let arc = &buffer[..size];
        let sarc = Sarc::parse(arc)?;
        let (bg_node, bg, gauge_node, gauge) = patched_animations(&sarc, arc, extra_colors)?;
        (bg_node, bg, gauge_node, gauge, sarc.data_offset)
    };
    let mut len = size;
    for (node, file) in [(bg_node, bg), (gauge_node, gauge)] {
        let start = (len + 0x7f) & !0x7f;
        if start + file.len() > buffer.len() {
            return None;
        }
        buffer[len..start].fill(0);
        buffer[start..start + file.len()].copy_from_slice(&file);
        len = start + file.len();
        put_u32(buffer, node + 8, start - data_offset);
        put_u32(buffer, node + 12, len - data_offset);
    }
    put_u32(buffer, 8, len);
    Some(len)
}

fn patched_animations(sarc: &Sarc, arc: &[u8], extra_colors: &[[f32; 3]]) -> Option<(usize, Vec<u8>, usize, Vec<u8>)> {
    let (bg_node, bg_start, bg_end) = sarc.find(JACK_BG)?;
    let (gauge_node, gauge_start, gauge_end) = sarc.find(JACK_GAUGE_COLOR)?;

    let bg = rebuild_bflan(&arc[bg_start..bg_end], |contents| {
        append_frames(contents, |target, _| {
            extra_colors.iter().map(|c| c.get((target as usize).wrapping_sub(4)).copied().unwrap_or(255.0)).collect()
        })
    })?;
    let gauge = rebuild_bflan(&arc[gauge_start..gauge_end], |contents| {
        // Its frame 1 (used for every non-P5 color) is padded up to frame 2 so that
        // frame 2 keeps meaning P4, then repeated for each extra color.
        append_frames(contents, |_, keys| {
            let non_p5 = value_at(keys, 1.0);
            let last = keys.last().map(|k| k[0]).unwrap_or(0.0);
            let pad = (2.0 - last).max(0.0) as usize;
            std::iter::repeat(non_p5).take(pad + extra_colors.len()).collect()
        })
    })?;
    Some((bg_node, bg, gauge_node, gauge))
}
