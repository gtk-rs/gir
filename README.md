# GIR

The `GIR` is used to generate both the sys level crate and a safe API crate to use the sys level (FFI) crate.

## Generate FFI (sys level)

Using `gir` requires both a `*.toml` and a `*.gir` for generation of the bindings.

The `*.gir` you need will correspond to the project you want to generate bindings for. You can get them from [here](https://github.com/gtk-rs/gir-files) or directly on [ubuntu website](http://packages.ubuntu.com/) (for example: http://packages.ubuntu.com/zesty/amd64/libgtk-3-dev).

The `*.toml` is what is used to pass various settings and options to gir for use when generating the bindings - you will likely need to write one to suit your needs, for an example you can take a look to [gtk-rs/sys/gir-gtk.toml](https://github.com/gtk-rs/sys/blob/master/conf/gir-gtk.toml).

## `gir` Modes

There are two main modes of generation for `gir`; *FFI* and *API*.

The *FFI* mode is what creates the low-level FFI bindings from the supplied `*.gir` file - these are essentially direct calls in to the related C library and are typically unsafe. The resulting crate is typically appended with `-sys`.

The *API* mode generates another crate for a layer on top of these unsafe (*sys*) bindings whihc makes them safe for use in general Rust.

### The FFI mode TOML config

In FFI (`-m sys`) mode, Gir generates as much as it can. So in this mode, the TOML file is mostly used to ignore some objects. To do so, you need to add its fullname to an `ignore` array. Example:

```toml
ignore = ["Gtk.Widget", "Gtk.Window"]
```

And that's all! Neither `GtkWidget` nor `GtkWindow` (alongside with their functions) will be generated.

Also you can mark some functions that it has suffix `_utf8` on Windows:

```toml
[[object]]
name = "GdkPixbuf.PixbufAnimation"
status = "generate"
    [[object.function]]
    name = "new_from_file"
    is_windows_utf8 = true
```

### Generation in FFI mode

When you're ready, let's generate the FFI part:

```shell
cargo run --release -- -c YourSysGirFile.toml -d ../gir-files -m sys -o the-output-directory-sys
```

The generated files will be placed in `the-output-directory-sys`. You now have the sys part of your binding!

## The API mode TOML config

This mode requires you to write another TOML file. [gtk/Gir.toml](https://github.com/gtk-rs/gtk/blob/master/Gir.toml) is a good example.

This mode generates only the specified objects. You can either add the object's fullname to the `generate` array or add it to the `manual` array (but in this case, it won't be generated, just used in other functions/methods instead of generating an "ignored" argument). Example:

```toml
generate = ["Gtk.Widget", "Gtk.Window"]
manual = ["Gtk.Button"]
```

So in here, both `GtkWidget` and `GtkWindow` will be fully generated and functions/methods using `GtkButton` will be uncommented. To generate code for all global functions, add `Gtk.*` to the `generate` array.

Sometimes Gir understands the object definition incorrectly or the `.gir` file contains an incomplete or wrong definition, to fix it, you can use the full object configuration:

```toml
[[object]]
# object's fullname
name = "Gtk.SomeClass"
# can be also "manual" and "ignore" but it's simpler to just put the object in the same array
status = "generate"
# replace the parameter name for the child in child properties (instead "child")
child_name = "item"
# don't generate trait SomeClassExt for this object, but implement all functions in impl SomeClass
trait = false
# allow rename result file
module_name = "soome_class"
# prefixed object in mod.rs with #[cfg(mycond)]
cfg_condition = "mycond"
    # define overrides for function
    [[object.function]]
    # filter functions from object
    name = "set_website_label"
    # alternative way to apply override for many functions. Will be used with '^' and '$' on both sides
    # can be used instead of `name` almost anywhere
    # pattern = "[gs]et_value"
    # don't generate function
    ignore = true
    # override starting version
    version = "3.12"
    # prefixed function with #[cfg(mycond)]
    cfg_condition = "mycond"
    # prefixed function with #[doc(hidden)]
    doc_hidden = true
        # override for parameter
        [[object.function.parameter]]
        # filter by name
        name = "website_label"
        # allow to remove/add Option<>
        nullable = true
        # allow to make parameter immutable
        const = true
        # override for return value
        [[object.function.return]]
        name = "website_label"
        # allow to remove/add Option<> to return value
        nullable = true
        # convert bool return types to Result<(), glib::BoolError> with
        # the given error message on failure
        bool_return_is_error = "Function failed doing what it is supposed to do"
    [[object.signal]]
    name = "activate-link"
    # replace trampoline bool return type with `Inhibit`
    inhibit = true
    ignore = true
    version = "3.10"
    doc_hidden = true
        [[object.signal.parameter]]
        name = "path_string"
        # allow to use different names in closure
        new_name = "path"
        # can be also "borrow" and "none": Add some transformation beetween ffi trampoline parameters and rust closure
        transformation = "treepath"
        nullable = true
        [object.signal.return]
        nullable = true
    # override for properties
    [[object.property]]
    name = "baseline-position"
    version = "3.10"
    ignore = true
```

Since there are no child properties in `.gir` files, it needs to be added for classes manually:

```toml
[[object]]
name = "Gtk.SomeClassWithChildProperties"
status = "generate"
# replace parameter name for child in child properties (instead of "child")
child_name = "item"
# define concrete child type (instead of "Widget")
child_type = "Gtk.MenuItem"
    [[object.child_prop]]
    name = "position"
    type = "gint"
    doc_hidden = true
```

For enumerations, you can configure the members:

```toml
[[object]]
name = "Gdk.EventType"
status = "generate"
    [[object.member]]
    name = "2button_press"
    # allows to skip elements with bad names, other members with same value used instead
    alias = true
    [[object.member]]
    name = "touchpad_pinch"
    # define starting version when member added
    version = "3.18"
```

For global functions, the members can be configured by configuring the `Gtk.*` object:

```toml
[[object]]
name = "Gtk.*"
status = "generate"
    [[object.member]]
    name = "stock_list_ids"
    # allows to ignore global functions
    ignore = true
```

Note that you must not place `Gtk.*` into the `generate` array and
additionally configure its members.

In various cases, GObjects or boxed types can be used from multiple threads
and have certain concurrency guarantees. This can be configured with the
`concurrency` setting at the top-level options or per object. It will
automatically implement the `Send` and `Sync` traits for the resulting object
and set appropriate trait bounds for signal callbacks. The default is `none`,
and apart from that `send` and `send+sync` are supported.

```toml
[[object]]
# object's fullname
name = "Gtk.SomeClass"
# can be also "manual" and "ignore" but it's simpler to just put the object in the same array
status = "generate"
# concurrency of the object, default is set in the top-level options or
# otherwise "none". Valid values are "none", "send" and "send+sync"
concurrency = "send+sync"
```

Note that `send` is only valid for types that are either not reference counted
(i.e. `clone()` copies the object) or that are read-only (i.e. no API for
mutating the object exists). `send+sync` is valid if the type can be sent to
different threads and all API allows simultaneous calls from different threads
due to internal locking via e.g. a mutex.

### Generation in API mode

To generate the Rust-user API level, The command is very similar to the previous one. It's better to not put this output in the same directory as where the FFI files are. Just run:

```shell
cargo run --release -- -c YourGirFile.toml -d ../gir-files -o the-output-directory
```

Now it should be done. Just go to the output directory (so `the-output-directory/auto` in our case) and try to build using `cargo build`. Don't forget to update your dependencies in both projects: nothing much to do in the FFI/sys one but the Rust-user API level will need to have a dependency over the FFI/sys one.

## Nightly Rust Only Features

### Unions

By default union generation is disabled except for some special cases due to unions not yet being a stable feature. However if you are using *nightly* rust, then you can enable union generation using `cargo run --release --features "use_unions"`.

Keep in mind that to access union members, you are required to use `unsafe` blocks, for example;

```
union myUnion {
    test : u32
}

let testUnion = myUnion { test : 42 };
unsafe { println!("{}", myUnion.test };
```

This is required as the rust compiler can not guarantee the safety of the union, or that the member being addressed exsits. The union RFC is [here](https://github.com/tbu-/rust-rfcs/blob/master/text/1444-union.md) and the tracking issue is [here](https://github.com/rust-lang/rust/issues/32836).
