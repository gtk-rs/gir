# Introduction
[gir] is a tool to automatically generate bindings and a safe wrapper for a library written in C. All you need to be able to use it is a `.gir` file containing the [GObject introspection](https://gi.readthedocs.io/en/latest/) data for the library you want to create the bindings for, as well as the `.gir` files for all its dependencies. If your library does not provide a `.gir` file, unfortunately you cannot use [gir], but maybe you can try [rust-bindgen](https://github.com/rust-lang/rust-bindgen).

## Where can I find the .gir files
There are multiple ways you can get the needed `.gir` files. The `*.gir` you need will correspond to the project you want to generate bindings for. If you have the library installed, you can search for it under `/usr/share/gir-1.0/`. Otherwise you should be able to get it from the package that installs the library. Ubuntu for example allows you to [search](https://packages.ubuntu.com/) for and download packages via their website.

The recommended way to get the `.gir` files for your dependencies is to clone the [gir-files repo](https://github.com/gtk-rs/gir-files). This is the recommended way, because some of the `.gir` files included in the libraries have small errors that are already fixed in the gir-files repo. Otherwise you could use the above mentioned methods to find the files.

## General steps
[gir] tries to make it as simple as possible to generate a safe wrapper for your C library. The process can be divided into four steps that correspond with the four operating modes gir has.

- Generating unsafe bindings
In this step, the low-level FFI bindings are created from the supplied `*.gir` file. These are essentially direct calls into the related C library and are typically unsafe. The resulting crate is typically appended with -sys. The operating mode is `sys`.

- Generating a safe wrapper
Next, another crate for a layer on top of these unsafe (sys) bindings is created, which makes them safe for use in general Rust. The operating mode is `normal`.

- Checking for missing types/methods
The operating mode `not_bound` allows you to see the detected types/methods that will not be generated for whatever reasons.

- Adding documentation
After the safe wrapper is created, gir can even generate the documentation for us. Use the operating mode `doc` to do so.


## Regenerating the bindings and wrapper
In order to generate the bindings and the wrapper for the first time, the above mentioned steps should be followed. When you want to regenerate the crates because e.g. the library was updated, you can simplify the process by running the helper script `./generator.py`. The script detects `Gir.toml` configurations in the current directory (or the path(s) passed on the command-line) and generates "normal" or "sys" crates for it. Alternatively --embed-docs can be passed to prepare source-code for a documentation build by moving all documentation into it. For a complete overview of available options, pass --help.

## GIR format reference
It can always be useful to look at the [reference](https://gi.readthedocs.io/en/latest/annotations/giannotations.html) or [schema](https://gitlab.gnome.org/GNOME/gobject-introspection/blob/master/docs/gir-1.2.rnc).

[gir]: https://github.com/gtk-rs/gir