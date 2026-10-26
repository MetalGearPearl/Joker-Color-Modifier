# Persona Joker Colors

A Skyline plugin for **Super Smash Bros. Ultimate 13.0.5** that gives each Joker costume (and other characters) its own defined color, as if battles were always fought on Mementos with a P3, P4, or other Persona soundtrack.

For each costume, you can configure:

- **Color**: Rebellion effects, Rebellion Gauge, HUD portrait background, victory screen bars/lines, and the sky in Joker's victory scene.
- **Victory theme**: Any installed victory theme track.

On **Mementos**, nothing is changed (except for costumes marked with `fixed`): everything continues to follow the song version (P3 blue, P4 yellow, P5 red), just like in the original game. Characters other than Joker that use Joker's victory scene (for example, through CSK's `result.stage`) also follow that version there.

## Requirements

- Smash Ultimate **13.0.5** (the plugin will not activate on other versions, to prevent crashes).
- [Skyline](https://github.com/skyline-dev/skyline) and [ARCropolis](https://github.com/Raytwo/ARCropolis).
- The CSK Collection if you use new echo fighters or new victory themes.

## Installation

Copy the contents of the `sdcard` folder to the root of your SD card (or your emulator's SD card):

```
sdcard/
├── atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libpersona_joker_colors.nro
└── ultimate/persona_joker_colors/
    ├── config.txt
    └── sky/<color>/
```

## Configuration (`ultimate/persona_joker_colors/config.txt`)

Use one line per costume or costume range:

```
[character] cXX, color: <color>, theme: <theme>[, fixed]
```

- `character`: Optional. Defaults to `jack` (Joker) if omitted. Use the character's folder name: `roy`, `chrom`, `ike`, etc.
- `cXX` or `cXX-cYY`: A single costume or a range of costumes.
- `color`: `red`, `blue`, `yellow`, `green`, `pink`, `purple`, `orange`, `white`, `black`, or `#RRGGBB`. `red`, `blue`, and `yellow` use the game's original P5/P3/P4 colors for effects and the HUD.
- `theme`: Victory theme track name, for example, `bgm_z97c_f_jack_p3`.
- `fixed`: Optional. The costume keeps its configured color and theme on Mementos as well.

Color and theme are optional. Lines beginning with `#` are treated as comments. If a costume is configured more than once, the last entry takes precedence. Changes are loaded when the game starts.

Example:

```
c21-c23, color: blue, theme: bgm_z97c_f_jack_p3
c24-c30, color: yellow, theme: bgm_z97b_f_jack_p4
c10, color: pink, theme: bgm_z97f_f_jack_p2batsu
c11, color: black, theme: bgm_z97f_f_jack_p2batsu, fixed
roy c21, color: white, theme: bgm_z97f_f_jack_p2batsu
```

Here, c10 is pink outside Mementos but still follows the song version on Mementos. Meanwhile, c11 is black and plays `p2batsu` on every stage, including Mementos.

### Victory Themes

The plugin changes the theme when one of the base game's Joker victory themes is about to play (`z97a_f_jack_p5`, `z97b_f_jack_p4`, `z97c_f_jack_p3`). For characters added through CSK, set their `fighter_jingle` to `z97a_f_jack_p5` (Joker's theme) in your CSK `database/*.json` file. This allows their themes to change according to the version on Mementos, just like Joker's, while outside Mementos (or everywhere when using `fixed`), the configured `theme` will play.

## Compatibility

- **HUD mods**: The plugin adds the new colors to whichever HUD version you have installed (`ui/layout/info/info_melee/info_melee/layout.arc`) without replacing it.
- **CSK costume-specific themes (`victory.toml`)**: When the configured theme is applied, it takes priority over costume-specific tracks.

### Building

```
cargo skyline build --release
```

## Dependencies

Plugin (Rust, versions pinned in `Cargo.lock`):

| Dependency | Version | Source |
|---|---|---|
| skyline (skyline-rs) | 0.6.0 | https://github.com/ultimate-research/skyline-rs (commit `6b52d32`) |
| arcropolis-api | 0.1.1 | https://github.com/Raytwo/arcropolis_api (commit `5d100d3`) |
| nnsdk, libc-nnsdk, crc32fast, lazy_static, and others | — | crates.io (dependencies of the above) |

Tools:

- [cargo-skyline](https://github.com/jam1garner/cargo-skyline) 3.5.0 (installs its own Rust toolchain for Switch).

Required in-game plugins: Skyline, ARCropolis, and The CSK Collection if you use added characters or new themes.

## AI Usage

This mod was developed with the assistance of **Claude** (Anthropic), used through Claude Code:

- Claude wrote the game code analysis (locating the Persona color functions, HUD, victory scene, and theme playback), the plugin code, the sky-generation script, and this documentation, following the author's instructions.
- The author defined the mod's intended behavior, tested each version in-game, and reported the results using screenshots.
- The sky textures are the original game textures with their colors changed using the script; they were not generated with AI image tools.

## Credits

- Plugin: (MG Pearl)
- Skyline, ARCropolis, and The CSK Collection by their respective authors.
