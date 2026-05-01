# DeskIcons

DeskIcons gives each Windows virtual desktop its own set of desktop icons.

## Installation

Double-click `deskicons.exe`. An install dialog will appear asking you to copy it to your user app data folder. Once installed, DeskIcons runs in the system tray.

To update, simply run the new `deskicons.exe` — it will detect the running copy, offer to update, and restart automatically.

## How It Works

- Each virtual desktop remembers your icon layout independently.
- Switching virtual desktops swaps the files on your Desktop folder and restores the saved icon positions.
- The Public Desktop (`C:\Users\Public\Desktop`) is always left visible and untouched.
- Your files are moved safely: a journal is kept so an interrupted swap can always be recovered.

## Tray Menu

Right-click the tray icon for these options:

| Option | Description |
|---|---|
| **Enabled** | Toggle DeskIcons on or off |
| **Restore Layout** | Re-apply the saved icon layout for the current desktop |
| **Recover Interrupted Swap** | Finish a swap that was interrupted mid-way |
| **Open State Folder** | Open `%LOCALAPPDATA%\DeskIcons` in Explorer |
| **Start with Windows** | Toggle automatic startup at login |
| **Exit** | Stop DeskIcons |

## Uninstall

1. Exit DeskIcons from the tray menu.
2. Turn off **Start with Windows** before exiting (or delete the `DeskIcons` value from `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
3. Delete `%LOCALAPPDATA%\DeskIcons`.

## Notes

- DeskIcons will not overwrite a file that already exists at the destination when swapping. If a conflict occurs it attempts a rollback.
- If your Desktop is redirected to OneDrive, DeskIcons will warn you — full support for OneDrive-redirected Desktops has not been tested yet.
- State (layouts, config, logs) is stored in `%LOCALAPPDATA%\DeskIcons`.
