# TectSkill CAD

**TectSkill CAD** is CADCraft with Thai text support, by [TectSkill](https://tectskill.com).

> Unofficial modified version of [CADCraft](https://github.com/storytold/cadcraft).
> Not made, sponsored or endorsed by the ArtCraft team.

Version numbers follow CADCraft's: TectSkill CAD v0.18.0 is built on CADCraft v0.18.0.

## What's different from CADCraft

- Thai shows in the interface: the in-place text editor, Properties panel, tabs and layer names.
- `FONTALT` and `FONTFALLBACK` are remembered between sessions.
- TectSkill CAD name and icon.

## Download

Get `TectSkill-CAD-windows.zip` from **Releases**, unzip, and run `TectSkill-CAD.exe`. No installation needed.
Optional: run `Thai-Font-Fix.bat` once so Thai text in drawings uses Cordia New.

## Build it yourself

```
cargo build --release -p cadcraft
```

The program is `target/release/cadcraft.exe`. The branding comes from `.cargo/config.toml`.

## License

Same as CADCraft: MIT OR Apache-2.0 — see `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE` and `CHANGES.txt`.
