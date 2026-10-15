# Video export: encoders and Rhubarb

How Fanimate makes each export format, which code does the encoding, what that obliges us to do, and what is left for counsel. This is research to brief a lawyer, not legal advice; section 4 of the [review scope](review-scope.md) still needs sign-off before 1.0.

Version 0.1.0 shipped a GPL build of FFmpeg with x264 beside the app. Later versions do not ship FFmpeg: MP4 is made by the operating system's own encoder, several formats are written by Fanimate itself, and the rest use an FFmpeg the user installs.

## How each format is made

| Format | Made by | Licence of the code we ship |
|---|---|---|
| PNG, TIFF and OpenEXR sequences, animated GIF, animated PNG | Fanimate, with the `image`, `png`, `gif` and `exr` crates | MIT/Apache; `exr` is BSD-3-Clause |
| AVI (uncompressed, OpenDML) | Fanimate's own writer (`fanimate-export/src/avi.rs`) | MIT/Apache |
| MP4 (AV1, no sound) | Fanimate, with `rav1e` (encoder) and `muxide` (MP4 writer) | rav1e BSD-2-Clause; muxide MIT/Apache |
| MP4 (H.264 video, AAC sound) | Windows: Media Foundation. macOS: AVFoundation. Linux: the user's FFmpeg. | Fanimate's calls into the system (`fanimate-oscodec`), MIT/Apache |
| MP4 (H.265), WebM (VP9), MKV (H.264), ProRes 4444, ProRes 422 HQ, DNxHR HQ, animated WebP | The user's FFmpeg | Nothing shipped |

Every route converts frames to video colour the same way (BT.709, limited range) and tags the file to say so, so a render looks the same whichever encoder made it.

## The system's H.264 and AAC encoders

Windows 10 and 11 include H.264 and AAC encoders in Media Foundation, except the "N" editions, which get them from Microsoft's free Media Feature Pack. macOS includes them in AVFoundation and VideoToolbox. Fanimate calls these through the operating system's own interfaces; it ships no H.264 or AAC encoder of its own. Where a graphics card encoder is certified, Windows uses it.

The OS vendors license the H.264 and AAC patent pools (Via LA) for the encoders they ship, so a program that asks the system to encode is understood not to distribute an encoder itself. This is the usual reading, and how many free programs make MP4 without bundling an encoder, but counsel should confirm it.

On Linux there is no system H.264 encoder, so MP4 (H.264) needs the user's FFmpeg there, as the other FFmpeg formats do. AV1 MP4 needs nothing.

Calling the system needs `unsafe` Rust, which the workspace otherwise denies. It is confined to `crates/fanimate-oscodec`, which exposes only a safe interface.

## FFmpeg is the user's

Fanimate finds an FFmpeg the user has installed (the path chosen in Preferences, then PATH, then the usual install folders for winget, Chocolatey, Scoop, Homebrew, MacPorts and Linux packages), asks it which encoders it has, and offers the formats it can make. The render queue starts it as a separate program and writes raw frames to its standard input.

Because Fanimate no longer distributes FFmpeg, the GPL duties that came with shipping it (publishing the exact sources, the build script and the licence with each release) no longer apply to us. Nothing in Fanimate is linked to FFmpeg, and Fanimate never downloads it: the [install guide](../install.md#more-video-formats-with-ffmpeg) points users to their platform's usual source.

Patents on the formats FFmpeg makes (H.264, H.265, AAC) are a matter between the user and whoever gave them their FFmpeg; personal use is free under the pools' terms. ProRes and DNxHR are made by FFmpeg's own independent encoders.

## AV1

AV1 was designed to be royalty-free. rav1e is BSD-2-Clause and includes the Alliance for Open Media patent licence (`PATENTS` in its source), which grants a royalty-free licence to AV1 patents held by the Alliance's members. muxide writes the MP4 container, which carries no patent issue. Fanimate's AV1 export has no sound, because the crates we found for Opus or AAC encoding in Rust were either not pure Rust, not MIT/Apache-compatible, or not mature enough to trust.

## Rhubarb Lip Sync

| Program | Version | Licence | Used for | Built by |
|---|---|---|---|---|
| `rhubarb/rhubarb` | Rhubarb Lip Sync 1.14.0 | MIT; its `LICENSE.md` lists the libraries it includes | Lip sync from a dialogue sound | Downloaded and checksum-checked by [scripts/fetch-rhubarb.sh](../../scripts/fetch-rhubarb.sh) |

Rhubarb still ships beside Fanimate as a separate program, because it is a ready-made program; it is MIT, so this places no GPL duties on the release. The About dialog credits it.

## For counsel before 1.0

- Confirm that calling the operating system's H.264 and AAC encoders needs no patent licence from us.
- Confirm that offering formats through an FFmpeg the user installs, without distributing it, places no GPL duty on Fanimate.

## Not done yet

- **macOS signing.** Release builds are not signed or notarised yet, so macOS may quarantine Rhubarb as it does the app. Rhubarb's macOS build is for Intel Macs and runs on Apple silicon through Rosetta 2.
- **Sound in AV1 files**, once a suitable Opus encoder exists in Rust.
