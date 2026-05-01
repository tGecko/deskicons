# DeskIcons

DeskIcons gibt jedem virtuellen Windows-Desktop seine eigene Sammlung von Desktop-Icons.

## Installation

Doppelklicke auf `deskicons.exe`. Ein Installationsdialog erscheint und fragt, ob das Programm in den App-Daten-Ordner des aktuellen Benutzers kopiert werden soll. Nach der Installation läuft DeskIcons im Infobereich der Taskleiste.

Für ein Update einfach die neue `deskicons.exe` starten — sie erkennt die laufende Instanz, bietet das Update an und startet sich anschließend automatisch neu.

## Funktionsweise

- Jeder virtuelle Desktop merkt sich sein Icon-Layout unabhängig von den anderen.
- Beim Wechsel des virtuellen Desktops werden die Dateien im Desktop-Ordner ausgetauscht und die gespeicherten Icon-Positionen wiederhergestellt.
- Der öffentliche Desktop (`C:\Users\Public\Desktop`) bleibt immer sichtbar und wird nicht verändert.
- Dateien werden sicher verschoben: Ein Journal wird geführt, damit ein unterbrochener Tausch jederzeit wiederhergestellt werden kann.

## Kontextmenü im Infobereich

Rechtsklick auf das Tray-Icon öffnet folgende Optionen:

| Option | Beschreibung |
|---|---|
| **Aktiviert** | DeskIcons ein- oder ausschalten |
| **Layout wiederherstellen** | Das gespeicherte Icon-Layout für den aktuellen Desktop erneut anwenden |
| **Unterbrochenen Tausch fortsetzen** | Einen halbfertigen Dateientausch abschließen |
| **Statusordner öffnen** | `%LOCALAPPDATA%\DeskIcons` im Explorer öffnen |
| **Mit Windows starten** | Automatischen Start beim Anmelden ein- oder ausschalten |
| **Beenden** | DeskIcons beenden |

## Deinstallation

1. DeskIcons über das Tray-Menü beenden.
2. **Mit Windows starten** vor dem Beenden deaktivieren (oder den Wert `DeskIcons` unter `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` in der Registrierung löschen).
3. Den Ordner `%LOCALAPPDATA%\DeskIcons` löschen.

## Hinweise

- DeskIcons überschreibt keine Datei, die am Zielort bereits vorhanden ist. Bei einem Konflikt wird ein Rollback versucht.
- Wenn der Desktop zu OneDrive umgeleitet ist, gibt DeskIcons eine Warnung aus — vollständige Unterstützung für OneDrive-umgeleitete Desktops wurde noch nicht ausreichend getestet.
- Zustand (Layouts, Konfiguration, Protokolle) wird unter `%LOCALAPPDATA%\DeskIcons` gespeichert.
