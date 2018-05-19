#!/bin/sh

# Minimize .gir file to faster loading
# Removing docs, functions
# Usage: ./minimize.sh GLib-2.0.gir

xmlstarlet ed -P -L \
    -d '//_:doc' \
    -d '//_:doc-deprecated' \
    -d '//_:function' \
    "$1"

sed -i '/^\s*$/d' "$1"
