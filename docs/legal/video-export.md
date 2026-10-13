# Video export: FFmpeg, x264 and Rhubarb

How Fanimate ships the programs that make video files and lip sync, what that obliges us to do, and what is left for counsel. This is research to brief a lawyer, not legal advice; section 4 of the [review scope](review-scope.md) still needs sign-off before 1.0.

## What ships

| Program | Version | Licence | Used for | Built by |
|---|---|---|---|---|
| `ffmpeg/ffmpeg` | FFmpeg 9.0.2 with x264 (stable branch, commit `b35605ac`) | GPL 2.0 or later (x264 is GPL, so the build is) | MP4 and MKV export (H.264 video, AAC sound) | [scripts/build-ffmpeg.sh](../../scripts/build-ffmpeg.sh), in the release workflow |
| `rhubarb/rhubarb` | Rhubarb Lip Sync 1.14.0 | MIT; its `LICENSE.md` lists the libraries it includes | Lip sync from a dialogue sound | Downloaded and checksum-checked by [scripts/fetch-rhubarb.sh](../../scripts/fetch-rhubarb.sh) |

GIF and PNG export use only Fanimate's own code and MIT/Apache crates.

## Why a separate program

Fanimate is MIT/Apache, and CLAUDE.md forbids GPL and LGPL code in the app and engine. Linking a GPL FFmpeg into the app would make every build we ship a GPL work, require plugins to be GPL-compatible (which matters for the Phase 3 marketplace), and has caused trouble in app stores.

So FFmpeg is never linked. The app (in the render queue process) starts `ffmpeg` with arguments and writes raw frames to its standard input; FFmpeg writes the file. That is two programs shipped together, which is widely treated as an aggregate rather than one combined work, so only the `ffmpeg` file carries the GPL. Counsel should confirm this. Rhubarb is MIT, and is run the same way only because it is a ready-made program.

Blender, which is itself GPL, links FFmpeg with x264 directly. Its builds are only kept GPL-compatible: for example, it dropped libfaac in 2011 because libfaac cannot be distributed under the GPL.

## What we owe, and how the release does it

These follow [FFmpeg's legal checklist](https://ffmpeg.org/legal.html), applied to a separate GPL program.

| Duty | How it is met |
|---|---|
| Give the exact source of what we ship | The release workflow publishes `ffmpeg-9.0.2.tar.xz` and `x264-<commit>.tar.xz` as files of the same GitHub release as the builds. GPLv2 section 3 accepts source offered from the same place as the binaries. The FFmpeg tarball is checked against a pinned SHA-256 (its signature by the FFmpeg release key `FCF986EA15E6E293A5644F10B4322F04D67658D8` was verified when the hash was pinned). x264 is fetched by commit hash and archived with `git archive`. |
| Say how it was built | `scripts/build-ffmpeg.sh` holds every configure flag; there are no patches. `ffmpeg/README.txt` in each download says where the sources and the script are. |
| Include the licence | `ffmpeg/LICENSE.txt` (GPLv2) beside the program; x264's `COPYING` is inside its source tarball. |
| Credit FFmpeg | The About dialog names FFmpeg, x264 and Rhubarb and their licences. |
| No non-free parts | `--enable-nonfree` is never used, and the build holds only x264 and FFmpeg's own code (`--disable-autodetect` stops system libraries being pulled in). |
| Do not restrict users | Fanimate has no EULA. Nothing forbids replacing or examining the bundled programs; Fanimate uses whichever `ffmpeg` it finds beside itself, else on PATH. |

## Patents

The GPL does not cover patents. H.264 is covered by patents pooled by Via LA:

- The pool's terms are $0 for the first 100,000 units a year per company, then $0.20 a unit, up to an annual cap. End users' personal use is free. The terms do not say clearly whether free downloads count as units; Via LA answers at info@via-la.com.
- The last major US H.264 patent (US 7826532) reportedly expires around 29 November 2027; some countries run into the 2030s. Fanimate 1.0 is planned for no earlier than the end of June 2027.
- Pool holders have historically pursued companies selling products with H.264, not free open-source projects; Blender, OBS and Krita have shipped x264 for years. The risk is low but not zero, and it sits with whoever distributes the builds.
- MP4 and MKV as containers carry no patent issue, and AAC encoding by FFmpeg's own encoder is in the same position as H.264 (a pool, with personal use free).

**For counsel before 1.0:** decide whether to contact Via LA or accept the remaining risk until the US patents expire.

## Not done yet

- **AV1 export** (royalty-free) was recommended as an option. It needs an AV1 encoder (SVT-AV1 or libaom, both BSD) added to the FFmpeg build; planned, not built.
- **macOS signing.** Release builds are not signed or notarised yet, so macOS may quarantine the bundled programs as it does the app. Rhubarb's macOS build is for Intel Macs and runs on Apple silicon through Rosetta 2.
