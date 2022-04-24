# Introduction
[gir] is a tool to automatically generate bindings and a safe wrapper for a library written in C. All you need to be able to use it is a `.gir` file containing the [GObject introspection](https://gi.readthedocs.io/en/latest/) data for the library you want to create the bindings for, as well as the `.gir` files for all its dependencies. If your library does not provide a `.gir` file, unfortunately you cannot use [gir], but maybe you can try [rust-bindgen](https://github.com/rust-lang/rust-bindgen).

## Where can I find the .gir files
There are multiple ways you can get the needed `.gir` files. The `*.gir` you need will correspond to the project you want to generate bindings for.


 There are multiple ways to get the .gir files you need. You can get them from [here](https://github.com/gtk-rs/gir-files). They are also usually included in the package of the library you want to use. The or example: <https://packages.ubuntu.com/jammy/libgtk-3-dev>).

- The `*.toml` is what is used to pass various settings and options to [gir] for use when generating the bindings - you will need to write one to suit your needs, for an example you can take a look to gtk-rs/sys/gir-gtk.toml.

It operates on 4 different modes:

- `sys`: is what creates the low-level FFI bindings from the supplied `*.gir` file - these are essentially direct calls in to the related C library and are typically unsafe. The resulting crate is typically appended with -sys.

- `normal`: generates another crate for a layer on top of these unsafe (sys) bindings which makes them safe for use in general Rust.

- `not_bound`: allows you to see the detected types/methods that will not be generated for whatever reasons.

- `doc`: used for documentation generation

[gir]: https://github.com/gtk-rs/gir

## Helpers

[gir] includes a wrapper script `./generator.py` that detects `Gir.toml` configurations in the current directory (or the path(s) passed on the command-line) and generates "normal" or "sys" crates for it. Alternatively --embed-docs can be passed to prepare source-code for a documentation build by moving all documentation into it. For a complete overview of available options, pass --help.

## GIR format reference

It can always be useful to look at the [reference](https://gi.readthedocs.io/en/latest/annotations/giannotations.html) or [schema](https://gitlab.gnome.org/GNOME/gobject-introspection/blob/master/docs/gir-1.2.rnc).
