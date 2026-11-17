#!/bin/sh
# Runs a board's example in the board simulator on this computer, rather than on the board, e.g.
#   ./sim.sh wave-esp32s3-rlcd-42
#   ./sim.sh guition-jc3248w535c --screenshot guitron.png --after 3 --headless
# with the stable toolchain, for this computer's own target, as the boards' settings are for their ESP32s. See
# bruce-embedded-std's docs/Simulator.md.
#   ./sim.sh --build <board>
# only builds it, a debug build, copied to the board's target/sim/demo (demo.exe on Windows), where VS Code's launch
# configurations run it, as where cargo puts it depends on this computer's target.
set -e
build_only=
if [ "$1" = "--build" ]; then
    build_only=1
    shift
fi
board="$1"
if [ -z "$board" ] || [ ! -d "$(dirname "$0")/$board" ]; then
    echo "Usage: $0 [--build] <board> [simulator options], one of: wave-esp32s3-rlcd-42 guition-jc3248w535c" >&2
    exit 1
fi
shift
host=$(rustc +stable -vV | sed -n 's/^host: //p')
cd "$(dirname "$0")/$board"
if [ -z "$build_only" ]; then
    exec cargo +stable run --no-default-features --features sim --target "$host" -- "$@"
fi
cargo +stable build --no-default-features --features sim --target "$host"
case "$host" in
    *windows*) exe=.exe ;;
    *) exe= ;;
esac
mkdir -p target/sim
# A new file, rather than copied over the old, which macOS kills, as it keeps the old file's identity.
rm -f "target/sim/demo$exe"
cp "target/$host/debug/bruce-esp-idf-example-$board$exe" "target/sim/demo$exe"
echo "Built target/sim/demo$exe"
