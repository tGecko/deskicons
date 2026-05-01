# DeskIcons

DeskIcons assigns different visible user Desktop contents to different Windows virtual desktops.

## Build

```powershell
cmake -S . -B build -G "Visual Studio 18 2026" -A x64
cmake --build build --config Release
```

Create the release ZIP:

```powershell
cmake --build build --config Release --target PACKAGE
```

OneDrive Desktop redirection is detected as a warning in `status`, but it still needs broader testing.

## Known Limitations

- Virtual desktop detection uses Explorer's current-user virtual desktop registry state.
- Public Desktop items are global.

## Coffee ☕

If this program helps you, consider fueling my caffeine addiction at https://ko-fi.com/gek ♡
