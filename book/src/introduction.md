# Introduction
[gir] is a tool to automatically generate safe wrappers for a C library with [GObject introspection](https://gi.readthedocs.io/en/latest/) information.
In order to use it you need the `.gir` file containing the introspection data for the library you want to create the bindings for, as well as the `.gir` files for all its dependencies.
Have a look at the tutorial if you don't know how to [find the .gir files](tutorial/finding_gir_files.md).
If your library does not provide a `.gir` file, unfortunately you cannot use [gir], but maybe you can try [rust-bindgen](https://github.com/rust-lang/rust-bindgen).

This book contains a tutorial on how to use [gir].
As an example we will create the bindings for [Pango](https://docs.gtk.org/Pango/).
In many cases you will be able to follow the same steps with your library.
If you are already familiar with [gir] and you just want to look up details about the configuration files, feel free to skip ahead to the documentation of the [configuration files](config/introduction.md).

## General steps
[gir] tries to make it as simple as possible to generate a safe wrapper for your C library.
The process can be divided into four steps that correspond to the four operating modes gir has.

- Generating unsafe bindings:
In this step, the low-level FFI bindings are created from the supplied `*.gir` files.
These are essentially direct calls into the related C library and are unsafe.
The resulting crate is typically appended with -sys.
The operating mode is `sys`.

- Generating a safe wrapper:
Next, another crate for a layer on top of these unsafe (sys) bindings is created, which makes them safe for use in general Rust.
The operating mode is `normal`.

- Checking for missing types/methods:
The operating mode `not_bound` allows you to see the detected types/methods that will not be generated for whatever reasons.

- Adding documentation:
After the safe wrapper is created, gir can even generate the documentation for us.
Use the operating mode `doc` to do so.


## Regenerating the bindings and wrapper
In order to generate the bindings and the wrapper for the first time, the above-mentioned steps should be followed.
When you want to regenerate the crates because e.g.
the library was updated, you can simplify the process by running the helper script `./generator.py`.
The script detects `Gir.toml` configurations in the current directory and subdirectories (or the paths passed on the command-line) and generates "normal" or "sys" crates for it.
Alternatively `--embed-docs` can be passed to prepare source-code for a documentation built by moving all documentation into it.
For a complete overview of available options, pass `--help`.

## GIR format reference
It can always be useful to look at the [reference](https://gi.readthedocs.io/en/latest/annotations/giannotations.html) or [schema](https://gitlab.gnome.org/GNOME/gobject-introspection/blob/master/docs/gir-1.2.rnc).

## Contact us
If you use [gir] on another library and it fails and you can't figure out why, don't hesitate to [contact us](https://gtk-rs.org/contact)!

[gir]: https://github.com/gtk-rs/gir