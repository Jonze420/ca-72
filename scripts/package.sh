#!/usr/bin/env bash
# Makes the installer for the platform it runs on from the bundles in target/bundled
# (decisions.md R17), into target/packages:
#
#   macOS    CA-72-<version>-macOS.pkg            (pkgbuild, productbuild; build the bundles
#                                                 with `cargo xtask bundle-universal` for
#                                                 Apple silicon and Intel)
#   Windows  CA-72-<version>-Windows-setup.exe    (Inno Setup 6's ISCC, from Git Bash)
#   Linux    CA-72-<version>-Linux-x86_64.tar.gz  (with install.sh)
#
# Each carries LICENSE and THIRD-PARTY-NOTICES.txt, inside the bundles as well. On macOS the
# bundles are signed with $CA72_MACOS_SIGN_APP (a "Developer ID Application" identity), the
# package with $CA72_MACOS_SIGN_INSTALLER ("Developer ID Installer"), and the package
# notarised with the notarytool keychain profile $CA72_NOTARY_PROFILE, when they are set;
# otherwise the bundles are signed ad hoc and the package is unsigned. Set
# $CA72_REQUIRE_NOTARIZATION=1 for a public Mac release: missing credentials or a failed
# Apple/Gatekeeper check then stops packaging. $CA72_ISCC names
# ISCC.exe when it is not in its usual place.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
bundles="$root/target/bundled"
out="$root/target/packages"
work="$root/target/packaging"
rm -rf "$work"
mkdir -p "$work" "$out"

for b in CA-72.vst3 CA-72.clap; do
    if [ ! -e "$bundles/$b" ]; then
        echo "package.sh: $bundles/$b is missing: build the bundles first (README, Building)" >&2
        exit 1
    fi
done
for f in LICENSE THIRD-PARTY-NOTICES.txt; do
    [ -f "$root/$f" ] || { echo "package.sh: $root/$f is missing" >&2; exit 1; }
done

# The licence and the notices beside the plug-ins, and inside each bundle that has room.
docs() {
    cp "$root/LICENSE" "$1/LICENSE.txt"
    cp "$root/THIRD-PARTY-NOTICES.txt" "$1/THIRD-PARTY-NOTICES.txt"
}

macos() {
    # Never notarise an unsigned payload, or sign just the outer package.
    if [ -n "${CA72_MACOS_SIGN_APP:-}${CA72_MACOS_SIGN_INSTALLER:-}${CA72_NOTARY_PROFILE:-}" ] ||
        [ "${CA72_REQUIRE_NOTARIZATION:-0}" = 1 ]; then
        : "${CA72_MACOS_SIGN_APP:?Set the Developer ID Application identity}"
        : "${CA72_MACOS_SIGN_INSTALLER:?Set the Developer ID Installer identity}"
    fi
    if [ "${CA72_REQUIRE_NOTARIZATION:-0}" = 1 ]; then
        : "${CA72_NOTARY_PROFILE:?Set the notarytool keychain profile for a public Mac release}"
    fi
    local stage="$work/stage" pkgs="$work/pkgs"
    mkdir -p "$stage/VST3" "$stage/CLAP" "$pkgs"
    cp -R "$bundles/CA-72.vst3" "$stage/VST3/"
    cp -R "$bundles/CA-72.clap" "$stage/CLAP/"
    local kind dir b plist
    for kind in vst3 clap; do
        dir="$(echo "$kind" | tr '[:lower:]' '[:upper:]')"
        b="$stage/$dir/CA-72.$kind"
        plist="$b/Contents/Info.plist"
        # nih-plug's bundler writes its own identifier and version 1.0.0.
        plutil -replace CFBundleIdentifier -string "com.idlefoundry.ca-72.$kind" "$plist"
        plutil -replace CFBundleShortVersionString -string "$version" "$plist"
        plutil -replace CFBundleVersion -string "$version" "$plist"
        plutil -replace LSMinimumSystemVersion -string "11.0" "$plist"
        plutil -replace NSHumanReadableCopyright -string "© 2026 Idle Foundry Ltd. GNU GPL version 3 or later." "$plist"
        mkdir -p "$b/Contents/Resources"
        docs "$b/Contents/Resources"
        if [ -n "${CA72_MACOS_SIGN_APP:-}" ]; then
            codesign --force --timestamp --options runtime --sign "$CA72_MACOS_SIGN_APP" "$b"
        else
            codesign --force --sign - "$b"
        fi
        codesign --verify --strict "$b"
        # Not relocatable: Installer would otherwise put it wherever it finds a bundle of the
        # same identifier (a build folder, say) rather than in the plug-ins' folder. And not
        # version-checked, so that any version installs over any other.
        pkgbuild --analyze --root "$stage/$dir" "$work/$kind.plist" > /dev/null
        plutil -replace 0.BundleIsRelocatable -bool NO "$work/$kind.plist"
        plutil -replace 0.BundleIsVersionChecked -bool NO "$work/$kind.plist"
        # (On macOS 15 and later pkgbuild prints "write: Permission denied" for each file
        # that carries the protected com.apple.provenance attribute; the payload is whole.)
        pkgbuild --quiet --root "$stage/$dir" --component-plist "$work/$kind.plist" \
            --install-location "/Library/Audio/Plug-Ins/$dir" \
            --identifier "com.idlefoundry.ca-72.$kind.pkg" --version "$version" \
            "$pkgs/$kind.pkg"
    done
    cat > "$work/distribution.xml" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>CA-72 $version</title>
    <organization>com.idlefoundry</organization>
    <domains enable_localSystem="true"/>
    <options customize="allow" require-scripts="false" hostArchitectures="arm64,x86_64"/>
    <volume-check>
        <allowed-os-versions><os-version min="11.0"/></allowed-os-versions>
    </volume-check>
    <choices-outline>
        <line choice="vst3"/>
        <line choice="clap"/>
    </choices-outline>
    <choice id="vst3" title="VST3" description="CA-72.vst3, in /Library/Audio/Plug-Ins/VST3">
        <pkg-ref id="com.idlefoundry.ca-72.vst3.pkg"/>
    </choice>
    <choice id="clap" title="CLAP" description="CA-72.clap, in /Library/Audio/Plug-Ins/CLAP">
        <pkg-ref id="com.idlefoundry.ca-72.clap.pkg"/>
    </choice>
    <pkg-ref id="com.idlefoundry.ca-72.vst3.pkg" version="$version" onConclusion="none">vst3.pkg</pkg-ref>
    <pkg-ref id="com.idlefoundry.ca-72.clap.pkg" version="$version" onConclusion="none">clap.pkg</pkg-ref>
</installer-gui-script>
EOF
    local pkg="$work/CA-72-$version-macOS.pkg" final="$out/CA-72-$version-macOS.pkg" sign=()
    [ -n "${CA72_MACOS_SIGN_INSTALLER:-}" ] && sign=(--sign "$CA72_MACOS_SIGN_INSTALLER")
    rm -f "$pkg"
    productbuild --quiet --distribution "$work/distribution.xml" --package-path "$pkgs" \
        ${sign[@]+"${sign[@]}"} "$pkg"
    if [ -n "${CA72_NOTARY_PROFILE:-}" ]; then
        # notarytool's process exit alone does not prove Apple accepted the submission.
        local result="$work/notarization.json"
        xcrun notarytool submit "$pkg" --keychain-profile "$CA72_NOTARY_PROFILE" \
            --wait --output-format json > "$result"
        if [ "$(plutil -extract status raw -o - "$result")" != Accepted ]; then
            echo "package.sh: Apple did not accept the installer; see $result" >&2
            cat "$result" >&2
            exit 1
        fi
        xcrun stapler staple "$pkg"
        xcrun stapler validate "$pkg"
        pkgutil --check-signature "$pkg"
        spctl --assess --type install --verbose=4 "$pkg"
    fi
    # Only replace the distributable after every requested verification passes.
    mv -f "$pkg" "$final"
    echo "$final"
}

windows() {
    local stage="$work/stage"
    mkdir -p "$stage"
    cp -R "$bundles/CA-72.vst3" "$bundles/CA-72.clap" "$stage/"
    mkdir -p "$stage/CA-72.vst3/Contents/Resources"
    docs "$stage/CA-72.vst3/Contents/Resources"
    docs "$stage"
    local iscc="${CA72_ISCC:-}"
    if [ -z "$iscc" ]; then
        for c in "/c/Program Files (x86)/Inno Setup 6/ISCC.exe" \
            "$(cygpath -u "${LOCALAPPDATA:-C:\\}")/Programs/Inno Setup 6/ISCC.exe"; do
            [ -f "$c" ] && { iscc="$c"; break; }
        done
    fi
    [ -n "$iscc" ] || { echo "package.sh: Inno Setup 6 (ISCC.exe) not found; set CA72_ISCC" >&2; exit 1; }
    # Git Bash would take ISCC's /switches for paths and rewrite them.
    MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q "/DVersion=$version" "/DStage=$(cygpath -w "$stage")" "/DOutput=$(cygpath -w "$out")" \
        "$(cygpath -w "$root/scripts/installer/CA-72.iss")"
    echo "$out/CA-72-$version-Windows-setup.exe"
}

linux() {
    local name="CA-72-$version-Linux-$(uname -m)"
    local stage="$work/$name"
    mkdir -p "$stage"
    cp -R "$bundles/CA-72.vst3" "$bundles/CA-72.clap" "$stage/"
    mkdir -p "$stage/CA-72.vst3/Contents/Resources"
    docs "$stage/CA-72.vst3/Contents/Resources"
    docs "$stage"
    cp "$root/scripts/installer/install.sh" "$stage/install.sh"
    chmod +x "$stage/install.sh"
    tar -C "$work" -czf "$out/$name.tar.gz" "$name"
    echo "$out/$name.tar.gz"
}

case "$(uname -s)" in
Darwin) macos ;;
Linux) linux ;;
MINGW* | MSYS* | CYGWIN*) windows ;;
*) echo "package.sh: $(uname -s) is not a platform the plug-in is built for" >&2; exit 1 ;;
esac
