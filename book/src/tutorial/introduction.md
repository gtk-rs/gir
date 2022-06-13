# Tutorial

Let's see how to generate Rust bindings of a GNOME library using the [gir] crate.

Note that the `.gir` files often happens that they are invalid (missing or invalid annotations for example). We have a small script to fix the `.gir` files we're using (and only them!) available in the [gir-files repository](https://github.com/gtk-rs/gir-files/blob/master/fix.sh). You can run it like this (at the same level of the `.gir` files you want to patch):

```sh
sh fix.sh
```

All `gtk-rs` generated crates come in two parts: the `sys` part which contains all the C functions and types definitions (direct mapping, everything is unsafe) and the "high-level" part which contains the nice, completely safe and idiomatic Rust API on top of the `sys` part.

As an example, we'll generate the `sourceview` library bindings. So first, let's generate the `sys` part!

[gir]: https://github.com/gtk-rs/gir
