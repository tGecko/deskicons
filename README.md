# DeskIcons

DeskIcons assigns different visible user Desktop contents to different Windows virtual desktops.

## Build

```powershell
cargo build --release
```

The executable is written to:

```powershell
target\release\deskicons.exe
```

OneDrive Desktop redirection is detected as a warning in `status`, but it still needs broader testing.

## Known Limitations

- Virtual desktop detection uses Explorer's current-user virtual desktop registry state.
- Public Desktop items are global.

## Coffee ☕

If this program helps you, consider fueling my caffeine addiction at https://ko-fi.com/gek ♡
