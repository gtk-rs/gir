#!/bin/sh

GIR="cargo run --release --"
LIBS="glib gobject gio pango"
export RUST_LOG="gir=warn"

cd "`dirname $0`"
cargo build --release || exit 1

for LIB in $LIBS; do
	mkdir -p ${LIB}-sys/src
	$GIR -c ${LIB}.toml || exit 2
done

cd sys_build
cargo build || exit 3
cd ..
