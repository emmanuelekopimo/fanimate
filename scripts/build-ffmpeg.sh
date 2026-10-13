#!/usr/bin/env bash
# Build the small FFmpeg that Fanimate ships for video export, from pinned
# sources, and put it with its licence and its exact sources in <out>.
#
#   scripts/build-ffmpeg.sh <out>
#
# The build holds only what export needs: raw frames and WAV sound in,
# H.264 (x264) and AAC out, MP4 and Matroska files. It is a separate program
# that Fanimate runs; nothing of it is linked into Fanimate. x264 is GPL, so
# this FFmpeg is GPL too: the sources it was built from go in <out>/source,
# to be published beside every release that ships it
# (docs/legal/video-export.md).
#
# Needs a C compiler, make, nasm and git; on Windows, an MSYS2 MinGW shell.
set -euo pipefail

FFMPEG_VERSION=9.0.2
# signed by the FFmpeg release key FCF986EA15E6E293A5644F10B4322F04D67658D8
FFMPEG_SHA256=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
# x264 publishes no release tarballs; a commit of its stable branch is as
# exact (git checks the content against the hash)
X264_COMMIT=b35605ace3ddf7c1a5d67a2eb553f034aef41d55

out=$(mkdir -p "$1" && cd "$1" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
prefix="$work/prefix"
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)
exe=""
ldflags="-L$prefix/lib"
case "$(uname -s)" in
  # no MinGW runtime DLLs to ship alongside
  MINGW* | MSYS* | CYGWIN*) exe=".exe" ldflags="$ldflags -static" ;;
esac

mkdir -p "$out/source"
cd "$work"

echo "== x264 $X264_COMMIT"
git init -q x264
git -C x264 fetch -q --depth 1 https://code.videolan.org/videolan/x264.git "$X264_COMMIT"
git -C x264 checkout -q FETCH_HEAD
test "$(git -C x264 rev-parse HEAD)" = "$X264_COMMIT"
git -C x264 archive --format=tar --prefix="x264-$X264_COMMIT/" HEAD | xz -9 > "$out/source/x264-$X264_COMMIT.tar.xz"
(
  cd x264
  ./configure --prefix="$prefix" --enable-static --disable-cli --enable-pic
  make -j"$jobs"
  make install
)

echo "== FFmpeg $FFMPEG_VERSION"
tarball="ffmpeg-$FFMPEG_VERSION.tar.xz"
curl -sSfL -o "$tarball" "https://ffmpeg.org/releases/$tarball"
if command -v sha256sum > /dev/null; then
  echo "$FFMPEG_SHA256  $tarball" | sha256sum -c -
else
  echo "$FFMPEG_SHA256  $tarball" | shasum -a 256 -c -
fi
cp "$tarball" "$out/source/"
tar -xf "$tarball"
(
  cd "ffmpeg-$FFMPEG_VERSION"
  # --disable-autodetect: no system libraries or frameworks picked up, so
  # the program runs on any machine of its platform
  PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure \
    --prefix="$prefix" \
    --pkg-config-flags=--static \
    --extra-cflags="-I$prefix/include" \
    --extra-ldflags="$ldflags" \
    --enable-gpl --enable-libx264 \
    --disable-everything --disable-autodetect --disable-doc \
    --disable-ffplay --disable-ffprobe --disable-network \
    --enable-protocol=file,pipe,fd \
    --enable-demuxer=rawvideo,wav \
    --enable-decoder=rawvideo,pcm_s16le,pcm_f32le \
    --enable-encoder=libx264,aac \
    --enable-muxer=mp4,matroska \
    --enable-filter=buffer,buffersink,abuffer,abuffersink,format,aformat,scale,aresample,null,anull,trim,atrim \
    --enable-swscale --enable-swresample
  make -j"$jobs"
  cp "ffmpeg$exe" "$out/"
  cp COPYING.GPLv2 "$out/LICENSE.txt"
)
cp "$work/x264/COPYING" "$out/source/x264-COPYING.txt"

cat > "$out/README.txt" <<EOF
FFmpeg $FFMPEG_VERSION with x264 ($X264_COMMIT), built for Fanimate's video export.

Fanimate runs this as a separate program to make MP4 and MKV files. It is
free software under the GNU General Public License, version 2 or later
(LICENSE.txt), and is not part of Fanimate's own code or licence.

The exact sources it was built from are published with each Fanimate
release that includes it, as release files named
ffmpeg-$FFMPEG_VERSION.tar.xz and x264-$X264_COMMIT.tar.xz:
https://github.com/toonittmedia/fanimate/releases

How it was built: scripts/build-ffmpeg.sh in the Fanimate repository.
EOF

"$out/ffmpeg$exe" -hide_banner -encoders 2> /dev/null | grep -q libx264
echo "== built $out/ffmpeg$exe"
