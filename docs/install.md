# Installing and updating Fanimate

Every release on the [releases page](https://github.com/toonittmedia/fanimate/releases) has an installer for each desktop platform, a portable archive, and the browser build. Fanimate needs no account and no network connection.

## Installers

| Platform | File | Notes |
|---|---|---|
| Windows 10 and 11 (64-bit) | `Fanimate-<version>-windows-x64-setup.exe` | Installs for your user account only, so it needs no administrator rights. Start menu entry, optional desktop shortcut, uninstall from Settings › Apps |
| macOS 13 or later (Apple silicon) | `Fanimate-<version>-macos-arm64.dmg` | Drag Fanimate to Applications |
| Linux (any distribution, x86-64) | `Fanimate-<version>-linux-x86_64.AppImage` | Make it executable and run it |
| Debian, Ubuntu and derivatives | `fanimate_<version>_amd64.deb` | Installs to `/opt/fanimate`, with a menu entry |
| Flatpak | `Fanimate-<version>-linux-x86_64.flatpak` | `flatpak install --user Fanimate-*.flatpak` |
| Any (portable) | `fanimate-<version>-<platform>.zip` / `.tar.gz` | Unpack and run `fanimate`; nothing is installed |

FFmpeg (video export) and Rhubarb (lip sync) come with every package as separate programs; see the [video export legal note](legal/video-export.md).

### Not signed yet

Releases are not yet signed with an Apple Developer ID or a Windows code-signing certificate (planned before 1.0). Until then:

- **Windows** may show "Windows protected your PC" for the installer: choose **More info › Run anyway**.
- **macOS** says the app "cannot be opened because the developer cannot be verified": right-click Fanimate in Applications, choose **Open**, then **Open** again. This is needed once.

Each release lists SHA-256 checksums (`SHA256SUMS.txt`) to check a download against.

## Updates

Fanimate looks for a new version a few seconds after it starts, when **Check for updates automatically** is on (Edit › Preferences, or the first-run dialog). It asks GitHub for the list of releases and nothing else; no information about you or your work is sent. Turn it off to stay on one version, for example during a production. **Help › Check for Updates** checks at any time.

When there is a new version, a button appears at the right of the status bar. Nothing is downloaded until you choose **Download**, and nothing is installed until you choose **Restart and Update**:

- The download is checked against the release's signature, made by the release workflow with Fanimate's update key. A file that does not match is deleted, never run.
- Before installing, Fanimate asks you to save changes, and waits while the render queue is rendering.
- **Windows:** the new installer replaces Fanimate and starts it again. If anything fails, the installer puts the old version back.
- **macOS:** the new app is unpacked beside the old one and swapped in by renaming, so a failure leaves the old app in place.
- **AppImage:** the new file replaces the old one in a single rename.
- **Flatpak and .deb** update through your system's software tools; Fanimate tells you so instead of updating itself.
- **Portable copies** cannot replace themselves; Fanimate links to the download page instead.

**Skip this version** hides a version until a newer one appears.

## Crash reports

If Fanimate closes unexpectedly, the next start first offers to recover your unsaved work (it is kept up to the last change), then asks whether to report the crash. **Report on GitHub…** opens a new issue in your browser with the report filled in (Fanimate's version, your system and graphics, what failed and where, and how many sessions on this computer ended in a crash), for you to read, edit and send. Fanimate never sends a report itself. Uncheck **Offer to report crashes** (also in Preferences) to stop being asked.

## Uninstalling

- **Windows:** Settings › Apps › Fanimate › Uninstall.
- **macOS:** move Fanimate from Applications to the Bin.
- **AppImage and portable copies:** delete the file or folder.
- **.deb:** `sudo apt remove fanimate`. **Flatpak:** `flatpak uninstall io.github.toonittmedia.Fanimate`.

Settings, crash recovery files and the render queue live in your user data folder (`%APPDATA%\Fanimate` on Windows, `~/Library/Application Support/Fanimate` on macOS, `~/.local/share/fanimate` on Linux); delete it to remove everything.
