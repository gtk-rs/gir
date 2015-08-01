#!/bin/sh

GIR="cargo run --release --"
FEATURES=""
if [ "$1" = "full" ]; then
	FEATURES="$FEATURES full"
fi
export RUST_LOG="gir=warn"

cd "`dirname $0`"
cargo build --release || exit 1

for TOML in gir-*.toml; do
	$GIR -c ${TOML} || exit 2
done

cd sys_build
cargo build --features "$FEATURES" || exit 3
cd ..
