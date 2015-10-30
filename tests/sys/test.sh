#!/bin/sh

LIBS="glib"
MORE_LIBS=""

GIR="cargo run --release --"
if [ "$1" = "full" ]; then
	LIBS="$LIBS $MORE_LIBS"
fi
export RUST_LOG="gir=warn"

cd "`dirname $0`"
cargo build --release || exit 1

for LIB in $LIBS; do
	$GIR -c "gir-${LIB}.toml" || exit 2
	cd "${LIB}-sys"
	cargo build || exit 3
	cargo test --features abi_tests || exit 4
	cd ..
done
