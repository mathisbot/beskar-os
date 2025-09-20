# DOOM

Play the original, shareware, version of the great DOOM directly on BeskarOS !

## Requirements

- clang
- [PureDOOM](https://github.com/Daivuk/PureDOOM)

## Getting started

Place the `src/DOOM` folder as well as the WAD file of [PureDOOM](https://github.com/Daivuk/PureDOOM) in this folder so your filetree looks like :

```
doom/
    DOOM/
        *.c
        doom1.wad
    src/
        main.rs
```

If compilation fails or the program crashes, try using the commit hash [48376dd](https://github.com/Daivuk/PureDOOM/tree/48376ddd6bbdb70085dab91feb1c6ceef80fa9b7).

Finally, you will have to edit the root `build.rs`/`Cargo.toml` to add doom as a dependency and edit the ramdisk accordingly (temporary).

## Usage

Default bindings are:

- Right: Right arrow
- Left: Left arrow
- Forward: Up arrow
- Backward: Down arrow
- Shoot: Control
- Strafe: Alt
- Run: Shift
- Interact/Use: Space
