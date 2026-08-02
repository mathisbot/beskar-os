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
        *.rs
```

If compilation fails or the program crashes, try using the commit hash [355cfbd](https://github.com/Daivuk/PureDOOM/commit/355cfbd16fac119718879239336ee2ea408886bd).

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
