#!/bin/sh
# Installs the CA-72 from this folder.
#
#   ./install.sh                     for you: ~/.vst3 and ~/.clap
#   ./install.sh --system            for everyone: /usr/lib/vst3 and /usr/lib/clap (uses sudo)
#   ./install.sh --uninstall         removes it (add --system for the system-wide copy)
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
vst3="$HOME/.vst3"
clap="$HOME/.clap"
sudo=""
uninstall=0
for arg in "$@"; do
    case "$arg" in
    --system) vst3=/usr/lib/vst3 clap=/usr/lib/clap ;;
    --uninstall) uninstall=1 ;;
    -h | --help) sed -n '2,7s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "install.sh: unknown option $arg (try --help)" >&2; exit 2 ;;
    esac
done
if [ "$vst3" = /usr/lib/vst3 ] && [ "$(id -u)" -ne 0 ]; then
    sudo=sudo
fi

if [ "$uninstall" = 1 ]; then
    $sudo rm -rf "$vst3/CA-72.vst3" "$clap/CA-72.clap"
    echo "Removed the CA-72 from $vst3 and $clap."
    exit 0
fi

for f in CA-72.vst3 CA-72.clap; do
    [ -e "$here/$f" ] || { echo "install.sh: $here/$f is missing; unpack the whole archive" >&2; exit 1; }
done
$sudo mkdir -p "$vst3" "$clap"
$sudo rm -rf "$vst3/CA-72.vst3" "$clap/CA-72.clap"
$sudo cp -R "$here/CA-72.vst3" "$vst3/"
$sudo cp "$here/CA-72.clap" "$clap/"
echo "Installed CA-72.vst3 in $vst3 and CA-72.clap in $clap."

# The editor needs X11's client libraries, which a desktop already has.
if command -v ldd > /dev/null 2>&1; then
    missing="$(ldd "$clap/CA-72.clap" 2> /dev/null | sed -n 's/^[[:space:]]*\([^ ]*\) => not found.*/\1/p')"
    if [ -n "$missing" ]; then
        echo "Warning: the plug-in needs libraries this system lacks:" $missing >&2
        echo "  (Debian/Ubuntu: libx11-6 libxcb1 libx11-xcb1; Fedora: libX11 libxcb; Arch: libx11 libxcb)" >&2
    fi
fi
echo "Rescan your host's plug-ins (or restart it) to find the CA-72."
