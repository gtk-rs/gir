# FFI Options

In FFI (`-m sys`) mode, `gir` generates as much as it can.
So in this mode, the TOML file is mostly used to ignore some objects.
To do so, you need to add its fullname to an `ignore` array.
Example:

```toml
ignore = ["Gtk.Widget", "Gtk.Window"]
```

And that's all.
Neither `GtkWidget` nor `GtkWindow` (alongside with their functions) will be generated.

You also need to add any needed external libraries in the "external_libraries" parameter.
Example:

```toml
[options]
external_libraries = [
   "GLib",
   "GObject",
]
```

You can specify a few other options:

```toml
[options]
girs_directories = ["../gir-files"]
library = "GtkSource"
version = "3.0"
min_cfg_version = "3.0"
target_path = "."
# Path where lib.rs generated (defaults to <target_path>/src)
# auto_path = "src"
work_mode = "sys"
# If true then build.rs will be split into 2 parts:
# always generated build_version.rs,
# and build.rs that generated only if not exists.
# Defaults to false
split_build_rs = false
# Adds extra versions to features
extra_versions = [
   "3.15",
   "3.17",
]
# Change library version for version
[[lib_version_overrides]]
version = "3.16"
lib_version = "3.16.1"
# Add extra dependencies to feature
[[feature_dependencies]]
version = "3.16"
dependencies = [
  "glib-sys/v3_16"
]
```

Also, you can add rust cfg conditions on objects, functions and constants, for example, when flagging for conditional compilation:

```toml
[[object]]
name = "GstGL.GLDisplayEGL"
status = "generate"
cfg_condition = "feature = \"egl\""
    [[object.function]]
    pattern = ".*"
    cfg_condition = "feature = \"egl\""
```

## Generation in FFI mode

When you're ready, let's generate the FFI part.
In the command we'll execute, `../gir-files` is where the directory with your `.gir` files is.
(But again, you can just clone the [gir-files repository](https://github.com/gtk-rs/gir-files) and add your file(s) in it).
Then let's run the command:

```sh
cargo run --release -- -c YourSysGirFile.toml -d ../gir-files -m sys -o the-output-directory-sys
```

The generated files will be placed in `the-output-directory-sys`.
Just take care about the dependencies and the crate's name generated in the `Cargo.toml` file (update them if they don't work as expected).

You now have the sys part of your binding!
