#!/usr/bin/env bash
# Validates the bundles in target/bundled (`cargo xtask bundle ca72-plugin --profile bundle`) with
# clap-validator, pluginval at strictness 10 and Steinberg's VST3 validator. Each is fetched
# from its GitHub release (checked against its SHA-256), or for Steinberg's built from the
# VST3 SDK, into $CA72_VALIDATORS (default target/validators). Needs curl, unzip, git, cmake
# and a C++ compiler (on Windows, Visual Studio's); on Linux without a display, xvfb-run for
# pluginval's editor tests, or CA72_SKIP_GUI_TESTS=1 to leave those out (as on any machine
# whose screen is in use).
set -euo pipefail

CLAP_VALIDATOR=0.4.1
CLAP_BUILD=0.4.1-127-g152b982
PLUGINVAL=v1.0.4
VST3_SDK=v3.8.1_build_84

root="$(cd "$(dirname "$0")/.." && pwd)"
bundles="$root/target/bundled"
dir="${CA72_VALIDATORS:-$root/target/validators}"
mkdir -p "$dir"

case "$(uname -s)" in
Darwin)
    os=macos
    clap_asset=clap-validator-$CLAP_BUILD-macos-universal.zip
    clap_sha=bbec8cd7d18274e549d5d8c12ece3cec54be966129388dd2e742b9957f2ba9f1
    pluginval_asset=pluginval_macOS.zip
    pluginval_sha=3c4c533bda0c5059eea3ddaea752d757ee2025041f0f47e6bcb0e87f6082b29f
    ;;
Linux)
    os=linux
    clap_asset=clap-validator-$CLAP_BUILD-ubuntu-22.04.zip
    clap_sha=49edadcfb407ea0dd946ce418300e853fbd2660fa4b0d00e4f19ff8eef24ad90
    pluginval_asset=pluginval_Linux.zip
    pluginval_sha=c01c49d8063965c4c2dea8324468336768f5c9139e0b1caebde14c2400b55352
    if [ "$(uname -m)" != x86_64 ]; then
        echo "validate.sh: the validators' Linux builds are for x86-64 only" >&2
        exit 1
    fi
    ;;
MINGW* | MSYS* | CYGWIN*)
    os=windows
    clap_asset=clap-validator-$CLAP_BUILD-windows.zip
    clap_sha=d935c3af0a45c3911ea2e900f4aa5d6709dac82bb485f0c4ce28648ab2cd0c10
    pluginval_asset=pluginval_Windows.zip
    pluginval_sha=c08e61ce3b96db41636f8ec7e76f4c7e2c13ebdac7fa1b5a1f52b4f32ec715ab
    ;;
*) echo "validate.sh: $(uname -s) is not a platform the plug-in is built for" >&2; exit 1 ;;
esac
exe=""
[ "$os" = windows ] && exe=.exe

sha() {
    if command -v sha256sum > /dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

# A release asset into a folder, checked against its SHA-256 and unpacked.
fetch() {
    local repo="$1" tag="$2" asset="$3" sum="$4" into="$5"
    mkdir -p "$into"
    curl -fsSL -o "$into/$asset" "https://github.com/$repo/releases/download/$tag/$asset"
    local got
    got="$(sha "$into/$asset")"
    if [ "$got" != "$sum" ]; then
        echo "validate.sh: $asset has SHA-256 $got, not $sum" >&2
        rm -f "$into/$asset"
        exit 1
    fi
    (cd "$into" && unzip -o -q "$asset")
}

# clap-validator's macOS and Linux archives hold a tarball, keeping the binary executable
# (on macOS, in a folder of its own).
work="$dir/clap-validator-$CLAP_VALIDATOR"
clap=""
[ -d "$work" ] && clap="$(find "$work" -type f -name "clap-validator$exe" | head -n 1)"
if [ -z "$clap" ]; then
    fetch free-audio/clap-validator "$CLAP_VALIDATOR" "$clap_asset" "$clap_sha" "$work"
    (cd "$work" && for t in ./*.tar.gz; do [ -e "$t" ] && tar xzf "$t"; done; true)
    clap="$(find "$work" -type f -name "clap-validator$exe" | head -n 1)"
fi

pluginval_dir="$dir/pluginval-$PLUGINVAL"
case "$os" in
macos) pluginval="$pluginval_dir/pluginval.app/Contents/MacOS/pluginval" ;;
*) pluginval="$pluginval_dir/pluginval$exe" ;;
esac
if [ ! -x "$pluginval" ]; then
    fetch Tracktion/pluginval "$PLUGINVAL" "$pluginval_asset" "$pluginval_sha" "$pluginval_dir"
    chmod +x "$pluginval"
fi

# Only the validator's own submodules, and none of the SDK's examples or VSTGUI.
sdk="$dir/vst3sdk-$VST3_SDK"
vst3=""
[ -d "$sdk/build/bin" ] && vst3="$(find "$sdk/build/bin" -type f -name "validator$exe" | head -n 1)"
if [ -z "$vst3" ]; then
    rm -rf "$sdk"
    git clone -q --depth 1 --branch "$VST3_SDK" https://github.com/steinbergmedia/vst3sdk "$sdk"
    git -C "$sdk" submodule update -q --init --depth 1 base cmake pluginterfaces public.sdk
    cmake -S "$sdk" -B "$sdk/build" -DCMAKE_BUILD_TYPE=Release \
        -DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF -DSMTG_ENABLE_VST3_HOSTING_EXAMPLES=OFF \
        -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF
    cmake --build "$sdk/build" --config Release --target validator --parallel
    vst3="$(find "$sdk/build/bin" -type f -name "validator$exe" | head -n 1)"
fi

gui=()
display=()
if [ "${CA72_SKIP_GUI_TESTS:-0}" = 1 ]; then
    gui=(--skip-gui-tests)
elif [ "$os" = linux ] && [ -z "${DISPLAY:-}" ]; then
    if ! command -v xvfb-run > /dev/null; then
        echo "validate.sh: no display and no xvfb-run: install xvfb, or set CA72_SKIP_GUI_TESTS=1" >&2
        exit 1
    fi
    display=(xvfb-run -a)
fi

echo "== clap-validator $CLAP_VALIDATOR"
"$clap" validate "$bundles/CA-72.clap"
echo "== pluginval $PLUGINVAL, strictness 10${gui[*]:+, editor tests skipped}"
${display[@]+"${display[@]}"} "$pluginval" --strictness-level 10 --validate-in-process \
    ${gui[@]+"${gui[@]}"} --validate "$bundles/CA-72.vst3"
echo "== Steinberg's VST3 validator, SDK $VST3_SDK"
"$vst3" "$bundles/CA-72.vst3"
