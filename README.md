# GIR

"[GObject introspection (abbreviated ‘GIR’)](https://developer.gnome.org/programming-guidelines/stable/introspection.html.en) is a system which extracts APIs from C code and produces binary type libraries which can be used by non-C language bindings, and other tools, to introspect or wrap the original C libraries. It uses a system of annotations in documentation comments in the C code to expose extra information about the APIs which is not machine readable from the code itself."


The `GIR` is used to generate both the sys level crate and a safe API crate to use the sys level (FFI) crate.

## How to use

This README files is more about the options and little overview. If you want a tutorial on how to generate a crate using `gir`, we recommend you to read this [tutorial](https://gtk-rs.org/docs-src/tutorial/gir_tutorial) instead.

`gir` includes a wrapper script `./generator.py` that detects `Gir.toml` configurations in the current directory (or the path(s) passed on the command-line) and generates "normal" or "sys" crates for it. Alternatively `--embed-docs` can be passed to prepare source-code for a documentation build by moving all documentation into it. For a complete overview of available options, pass `--help`.

## Introduction to `gir` generation

Using `gir` requires both a `*.toml` and a `*.gir` for generation of the bindings.

The `*.gir` you need will correspond to the project you want to generate bindings for. You can get them from [here](https://github.com/gtk-rs/gir-files) or directly on [ubuntu website](http://packages.ubuntu.com/) (for example: http://packages.ubuntu.com/zesty/amd64/libgtk-3-dev).

The `*.toml` is what is used to pass various settings and options to gir for use when generating the bindings - you will likely need to write one to suit your needs, for an example you can take a look to [gtk-rs/sys/gir-gtk.toml](https://github.com/gtk-rs/sys/blob/master/conf/gir-gtk.toml).

Keep it in mind that since `gir` is still under development, it generates warnings when running. As long as it's not errors, it's fine. However, if something you asked to be generated wasn't, you should definitely take a look to the warnings to see what failed.

## `gir` Modes

There are two main modes of generation for `gir`; _FFI_ and _API_.

There is also a third one used for documentation generation: _doc_ and a fourth one used to display all not bound types in your crate: _not_bound_.

The _FFI_ mode is what creates the low-level FFI bindings from the supplied `*.gir` file - these are essentially direct calls in to the related C library and are typically unsafe. The resulting crate is typically appended with `-sys`.

The _API_ mode generates another crate for a layer on top of these unsafe (_sys_) bindings which makes them safe for use in general Rust.

### The FFI mode TOML config

In FFI (`-m sys`) mode, `gir` generates as much as it can. So in this mode, the TOML file is mostly used to ignore some objects. To do so, you need to add its fullname to an `ignore` array. Example:

```toml
ignore = ["Gtk.Widget", "Gtk.Window"]
```

And that's all! Neither `GtkWidget` nor `GtkWindow` (alongside with their functions) will be generated.

You also need to add any needed external libraries in the "external_libraries" parameter. Example:

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
# If true then build.rs will be splitted into 2 parts:
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
# Add features to the "dox" feature declaration in `Cargo.toml`. So with the following
# config, it'll generate:
# dox = ["whatever"]
dox_feature_dependencies = ["whatever"]
```

You can mark some functions that has suffix `_utf8` on Windows:

```toml
[[object]]
name = "GdkPixbuf.PixbufAnimation"
status = "generate"
    [[object.function]]
    name = "new_from_file"
    is_windows_utf8 = true
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

### Generation in FFI mode

When you're ready, let's generate the FFI part. In the command we'll execute, `../gir-files` is where the directory with your `.gir` files is. (But again, you can just clone the [gir-files repository](https://github.com/gtk-rs/gir-files) and add your file(s) in it). Then let's run the command:

```shell
cargo run --release -- -c YourSysGirFile.toml -d ../gir-files -m sys -o the-output-directory-sys
```

The generated files will be placed in `the-output-directory-sys`. Just take care about the dependencies and the crate's name generated in the `Cargo.toml` file (update them if they don't work as expected).

You now have the sys part of your binding!

## The API mode TOML config

This mode requires you to write another TOML file. [gtk/Gir.toml](https://github.com/gtk-rs/gtk/blob/master/Gir.toml) is a good example.

```toml
[options]
girs_directories = ["gir-files"]
library = "Gtk"
version = "3.0"
min_cfg_version = "3.4"
target_path = "."
# Path where objects generated (defaults to <target_path>/src/auto)
# auto_path = "src/auto"
work_mode = "normal"
generate_safety_asserts = true
deprecate_by_min_version = true
# With this option enabled, versions for gir and gir-files saved only to one file to minimize noise,
# can also take path to the directory for saving "versions.txt" or filename with extension.
# Relative to target_path
single_version_file = true
# Generation of Display trait enabled for all enums, classes, etc.,
# which do not have an override for `generate_display_trait`
# (defaults to "true")
generate_display_trait = true
# Trust the nullability information about return values. If this is disabled
# then any pointer return type is assumed to be nullable unless there is an
# explicit override for it.
# This has to be used carefully as many libraries are missing nullable
# annotations for return values, which then will cause a panic once an
# unexpected NULL is returned.
trust_return_value_nullability = false
# Disable running `cargo fmt` on generated files
# (defaults to false)
disable_format = true
# Always generate a Builder if possible. This is mostly a convenient setter as most of the
# time you might want the Builder to be generated. Ignoring none-desired ones can still be done with per object `generate_builder` configuration.
# (defaults to false)
generate_builder = true
```

This mode generates only the specified objects. You can either add the object's fullname to the `generate` array or add it to the `manual` array (but in this case, it won't be generated, just used in other functions/methods instead of generating an "ignored" argument). Example:

```toml
generate = ["Gtk.Widget", "Gtk.Window"]
manual = ["Gtk.Button"]
```

So in here, both `GtkWidget` and `GtkWindow` will be fully generated and functions/methods using `GtkButton` will be uncommented. To generate code for all global functions, add `Gtk.*` to the `generate` array.

To also generate a `Builder` struct for a widget, it needs to be set with the `generate_builder` flag in object configuration:

```toml
[[object]]
name = "Gtk.TreeView"
status = "generate"
generate_builder = true
```

If you want to remove warning messages about the not bound `Builders` during the generation you don't want to be generated, you can ignore them with the `generate_builder` flag in object configuration:

```toml
[[object]]
name = "Gtk.TreeView"
status = "generate"
generate_builder = false
```

If there is some work which has to be done post-construction before the builder's
`build` method returns, you can set the `builder_postprocess` value in the object configuration:

```toml
[[object]]
name = "Gtk.Application"
status = "generate"
generate_builder = true
builder_postprocess = "Application::register_startup_hook(&ret);"
```

For the duration of the code in `builder_postprocess` the binding `ret` will be the
value to be returned from the `build` method.

Sometimes Gir understands the object definition incorrectly or the `.gir` file contains an incomplete or wrong definition, to fix it, you can use the full object configuration:

```toml
[[object]]
# object's fullname
name = "Gtk.SomeClass"
# can be also "manual" and "ignore" but it's simpler to just put the object in the same array
status = "generate"
# replace the parameter name for the child in child properties (instead "child")
child_name = "item"
# mark object as final type, i.e. one without any further subclasses. this
# will not generate trait SomeClassExt for this object, but implement all
# functions in impl SomeClass
final_type = true
# allow rename result file
module_name = "soome_class"
# override starting version
version = "3.12"
# prefixed object in mod.rs with #[cfg(mycond)]
cfg_condition = "mycond"
# if you want to override default option Ex. for write your own Display implementation
generate_display_trait = false
# if you want to generate builder with name SomeClassBuilder
generate_builder = true
# trust return value nullability annotations for this specific type.
# See above for details and use with care
trust_return_value_nullability = false
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
    # define a list of function parameters to be ignored when the documentation is generated
    doc_ignore_parameters = ["some_user_data_param"] 
    # disable length_of autodetection
    disable_length_detect = true
    # write function docs to trait other than default "xxxExt",
    # also works in [object.signal] and [object.property]
    doc_trait_name = "SocketListenerExtManual"
    # disable generation of future for async function
    no_future = true
    # to rename the generated function
    rename = "something_else"
    # to override the default safety assertions: "none", "skip",
    # "not-initialized", "in-main-thread"
    assertion = "in-main-thread"
        # override for parameter
        [[object.function.parameter]]
        # filter by name
        name = "website_label"
        # allow to remove/add Option<>
        nullable = true
        # allow to make parameter immutable
        const = true
        # parameter is calculated as length of string or array and removed from function declaration
        # (for length of return value use "return")
        length_of = "str"
        # change string type. Variants: "utf8", "filename", "os_string"
        string_type = "os_string"
        # make function unsafe to call (emits `fn unsafe`)
        unsafe = true

        # override for return value
        [object.function.return]
        # allow to remove/add Option<> to return value
        nullable = true
        # convert bool return types to Result<(), glib::BoolError> with
        # the given error message on failure
        bool_return_is_error = "Function failed doing what it is supposed to do"
        # convert Option return types to Result<T, glib::BoolError> with
        # the given error message on failure
        nullable_return_is_error = "Function failed doing what it is supposed to do"
        # always include the return value of throwing functions in the returned Result<...>,
        # without this option bool and guint return values are assumed to indicate success or error,
        # and are not included in the returned Result<...>
        use_return_for_result = true
        # change string type. Variants: "utf8", "filename", "os_string"
        string_type = "os_string"
        # overwrite type
        type = "Gtk.Widget"
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
    [[object.property]]
    name = "events"
    # generate only `connect_property_events_notify`, without `get_property_events` and `set_property_events`
    # supported values: "get", "set", "notify"
    generate = ["notify"]
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

For enumerations and bitflags, you can configure the members and mark the type
as `#[must_use]`:

```toml
[[object]]
name = "Gdk.EventType"
status = "generate"
# generates #[must_use] attribute for the type
must_use = true
# override starting version
version = "3.12"
    [[object.member]]
    name = "2button_press"
    # allows to skip elements with bad names, other members with same value used instead
    alias = true
    # Allow to add a cfg condition
    cfg_condition = "target_os = \"linux\""
    [[object.member]]
    name = "touchpad_pinch"
    # define starting version when member added
    version = "3.18"
```

For enumerations and bitflags, you can also configure additional `#[derive()]`
clauses optionally conditioned to a `cfg`.

```toml
[[object]]
name = "Gst.Format"
status = "generate"
    [[object.derive]]
    name = "Serialize, Deserialize"
    cfg_condition = "feature = \"ser_de\""
```

Gir auto-detects `copy`/`free` or `ref`/`unref` function pairs for memory management
on records. It falls back to generic `g_boxed_copy`/`g_boxed_free` if these are not
found, based on an existing implementation of `get_type`. Otherwise no record
implementation can be generated.

Some boxed types are passed as `out` parameters to functions and the caller is
required to allocate them. For this it is necessary to provide Rust
expressions in the configuration for initializing newly allocated memory for
them, and to free any resources that might be stored in values of that boxed
types. By default the memory is zero-initialized and it is valid to provide an
empty closure like below.

```toml
[[object]]
name = "Gtk.TreeIter"
status = "generate"
init_function_expression = "|_ptr| ()"
clear_function_expression = "|_ptr| ()"
```

For global functions, the members can be configured by configuring the `Gtk.*` object:

```toml
[[object]]
name = "Gtk.*"
status = "generate"
    [[object.function]]
    name = "stock_list_ids"
    # allows to ignore global functions
    ignore = true
```

Which will prevent gir from generating `stock_list_ids`. If you want to specify
that a function will be manually implemented, you can use:


```toml
[[object]]
name = "Gtk.Entry"
status = "generate"
    [[object.function]]
    name = "get_invisible_char"
    manual = true
```

This will prevent gir from generating `get_invisible_char` and it won't generate
`get_property_invisible_char` which would have been generated if we had used
"ignore = true".

Note that you must not place `Gtk.*` into the `generate` array and
additionally configure its members.

You can control the generation of constants in a similar fashion:

```
[[object]]
name = "Gtk.*"
status = "generate"
    [[object.constant]]
    pattern = "*"
    # No constants will be generated
    ignore = true
```

Constants also support `version` and `cfg_condition` fields.

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

```toml
[[object]]
name = "Gtk.Something"
status = "manual"
# Can also be "ref-mut", "ref-immut"
ref_mode = "ref"
```

When manually generating bindings, it can happen that the reference mode
detected by GIR is different than what was implemented and conversion to the C
types are wrong in autogenerated functions that have such objects as argument.
This can be overridden with the `ref_mode` configuration.

Getters are automatically renamed to comply with Rust codying style guidelines.
However, this can cause name clashes with existing functions. If you want to
bypass the automatic renaming mechanism, use `bypass_auto_rename = true`:

```toml
[[object]]
name = "Gtk.TextBuffer"
[...]
    [[object.function]]
    name = "get_insert"
    # Avoid clash with the `insert` operation.
    bypass_auto_rename = true
```

Some constructors are not annotated as `constructor` in the `gir` files. In
order for the naming convention to be applied, you can force a function to be
considered as a constructor:

```toml
[[object.function]]
name = "new_for_path"
# Not annotated as constructor in Gir => force it to apply naming convention
constructor = true
```

#### conversion_type "Option"

The `conversion_type` variant `Option` is available for types `T` implementing
`glib::TryFromGlib<Error=GlibNoneError>`. As a reminder, this allows
implementing `FromGlib` for `Option<T>` and usually goes alongside with `ToGlib`
for both `T` and `Option<T>`. In this case, `Option<T>` will be used for return
values (including ffi output arguments). For in-arguments, except if the
parameter is declared `mandatory`, `impl Into<Option<T>>` so that either an
`Option<T>` or `T` can be used.

Ex. from `gstreamer-rs`:

``` rust
[[object]]
name = "Gst.ClockTime"
status = "manual"
conversion_type = "Option"
```

The type `ClockTime` implements `glib::TryFromGlib<Error=GlibNoneError>` (and
`OptionToGlib`), which means that its Rust representation can take advantage of
`Option<ClockTime>`.

Additionally, the user can instruct `gir` to `expect` `Some` or `Ok` results for
specific arguments or return values. E.g.:

``` rust
[[object]]
name = "Gst.Clock"
status = "generate"
manual_traits = ["ClockExtManual"]
    [[object.function]]
    name = "get_calibration"
        [[object.function.parameter]]
        name = "internal"
        mandatory = true
```

In the above example, the user instructs gir to consider the `internal` argument
(which also happens to be an out argument) with type gir `Gst.ClockTime` can be
represented as a `ClockTime` without the `Option`. This argument is actually
part of a set of output arguments. With the above gir declaration, the generated
signature is the following (the implementation takes care of `expect`ing the
value to be defined):

``` rust
    fn get_calibration(
        &self,
    ) -> (
        ClockTime,
        Option<ClockTime>,
        Option<ClockTime>,
        Option<ClockTime>,
    );
```

For a return value, the mandatory declaration reads:

``` rust
    [[object.function]]
    name = "util_get_timestamp"
    /.../
        [object.function.return]
        # always returns a value
        mandatory = true
```

#### conversion_type "Result"

The `conversion_type` variant `Result` is available for types `T` implementing
`glib::TryFromGlib<Error=Err>` where `Err` is neither `GlibNoneError` nor
`GlibNoneOrInvalidError`. In this case, `Result<T, ErrorType>` will be used for
return values (including `ffi` output arguments) and the type itself in argument
position.

In `gstreamer-rs`, the C type `GstStateChangeReturn` can represent both a
successful or an error return value. In Rust, the `Result` `enum` is the
idiomatic way of returning an error. In `gstreamer-rs`, bindings to functions
returning `GstStateChangeReturn` had to be manually implemented so as to return
`Result<StateChangeSuccess, StateChangeError>`. Note that in this case, the type
implementing `TryFromGlib` is `StateChangeSuccess` and not
`GstStateChangeReturn`. These functions can be auto-generated using:

``` rust
[[object]]
name = "Gst.StateChangeReturn"
status = "generate"
must_use = true
    [object.conversion_type]
    variant = "Result"
    ok_type = "gst::StateChangeSuccess"
    err_type = "gst::StateChangeError"
```

### Generation in API mode

To generate the Rust-user API level, The command is very similar to the previous one. It's better to not put this output in the same directory as where the FFI files are. Just run:

```shell
cargo run --release -- -c YourGirFile.toml -d ../gir-files -o the-output-directory
```

Now it should be done. Just go to the output directory (so `the-output-directory/auto` in our case) and try to build using `cargo build`. Don't forget to update your dependencies in both projects: nothing much to do in the FFI/sys one but the Rust-user API level will need to have a dependency over the FFI/sys one.

Now, at your crate entry point (generally `lib.rs`), add the following to include all generated files:

```rust
pub use auto::*;
```

### Add manual bindings alongside generated code

Unfortunately, `gir` isn't perfect (yet) and will certainly not be able to generate all the code on its own. So here's what a `gir` generated folder looks like:

```
- your_folder
|
|- Cargo.toml
|- src
 |
 |- lib.rs
 |- auto
  |
  |- (all files generated by gir)
```

You can add your manual bindings directly inside the `src` folder (at the same level as `lib.rs`). Then don't forget to reexport them. Let's say you added a `Color` type in a `color.rs` file. You need to add in `lib.rs`:

```rust
// We make the type public for the API users.
pub use color::Color;

mod color;
```

## Generating documentation

And finally the last feature! Just run the following command (note the `-m doc` at the end):

```shell
cargo run --release -- -c YourGirFile.toml -d ../gir-files --doc-target-path the-output-file-name -m doc
```

It'll generate a markdown file if everything went fine. That's where all this crate's documentation is. If you want to put it back into your crate's source code like "normal" doc comments, run:

```shell
> cargo install rustdoc-stripper
> rustdoc-stripper -g -o docs.md
```

And now your crate should be completely documented as expected!

If you defining traits manually you can add them to "Implements" section for classes and interfaces:

```toml
[[object]]
name = "Gtk.Assistant"
status = "generate"
#add link to trait from current crate
manual_traits = ["AssistantExtManual"]

[[object]]
name = "Gtk.Application"
status = "generate"
#add link to trait from other crate
manual_traits = ["gio::ApplicationExtManual"]
```

### Unions

`gir` now has the ability to generate c-like unions using newly stabilised `union` in rustc 1.19. As such this means `gir` requires a minimum version rustc of 1.19

Keep in mind that to access union members, you are required to use `unsafe` blocks, for example;

```
union myUnion {
    test : u32
}

let testUnion = myUnion { test : 42 };
unsafe { println!("{}", myUnion.test };
```

This is required as the rust compiler can not guarantee the safety of the union, or that the member being addressed exists. The union RFC is [here](https://github.com/tbu-/rust-rfcs/blob/master/text/1444-union.md) and the tracking issue is [here](https://github.com/rust-lang/rust/issues/32836).

### Crate name overrides

`gir` uses simple rule to convert a namespace to a crate name and it sometimes goes wrong. For example, "WebKit2WebExtension" namespace will be converted to "web_kit2_web_extension", which looks bad.

To fix it, the `crate_name_overrides` option can be used.

It also replaces FFI crates' name.

```toml
[crate_name_overrides]
"web_kit2_web_extension" = "webkit2_webextension"
```

## GIR format reference

It can always be useful to look at the [reference](https://gi.readthedocs.io/en/latest/annotations/giannotations.html) or [schema](https://gitlab.gnome.org/GNOME/gobject-introspection/blob/master/docs/gir-1.2.rnc), right?
