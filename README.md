# GIR

The `GIR` is used to generate both sys level and Rust-user API level.

## Generate FFI (sys level)

You first need to get the Gir file corresponding to the project you want to bind. You can get them from [here](https://github.com/gtk-rs/gir-files) or directly on [ubuntu website](http://packages.ubuntu.com/) (for example: http://packages.ubuntu.com/zesty/amd64/libgtk-3-dev).

Then you need to write a TOML file (let's call it YourSysGirFile.toml) that you'll pass to gir (you can take a look to [gtk-rs/sys/gir-gtk.toml](https://github.com/gtk-rs/sys/blob/master/conf/gir-gtk.toml) to see an example).

### TOML file

In FFI mode, Gir generates as much as it can. So in this mode, the TOML file is mostly used to ignore some objects. To do so, you need to add its fullname to an `ignore` array. Example:

```toml
ignore = ["Gtk.Widget", "Gtk.Window"]
```

And that's all! Neither `GtkWidget` nor `GtkWindow` (alongside with their functions) will be generated.

### Generation

When you're ready, let's generate the FFI part:

```shell
cargo run --release -c YourSysGirFile.toml -d ../gir-files -m sys -o the-output-directory-sys
```

The generated files will be placed in `the-output-directory-sys`. You now have the sys part of your binding!

## Generate the Rust-user API level

You'll now have to write another GIR file (take a look to [gtk/Gir.toml](https://github.com/gtk-rs/gtk/blob/master/Gir.toml) for an example).

### TOML file

At the opposite of the FFI mode, this one only generates the specified objects. You can either add the object's fullname to the `generate` array or add it to the `manual` array (but in this case, it won't be generated, just used in other functions/methods instead of generating an "ignored" argument). Example:

```toml
generate = ["Gtk.Widget", "Gtk.Window"]
manual = ["Gtk.Button"]
```

So in here, both `GtkWidget` and `GtkWindow` will be fully generated and functions/methods using `GtkButton` will be uncommented.

Sometimes Gir understands the object definition incorrectly or the `.gir` file contains incomplete or wrong definition, to fix it, you can use the full object configuration:

```toml
[[object]]
# object's fullname
name = "Gtk.SomeClass"
# can be also "manual" and "ignore" but it's simpler to just put the object in the same array
status = "generate"
# replace the parameter name for the child in child properties (instead "child")
child_name = "item"
# force trait SomeClassExt generation even if class has no children
trait = true
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
    [[object.signal]]
    name = "activate-link"
    # replace trampoline bool return type with `Inhibit`
    inhibit = true
    ignore = true
    version = "3.10"
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

Since there is no child properties in `.gir` files, it needs to be added for classes manually:

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

### Generation

To generate the Rust-user API level, The command is very similar to the previous one. It's better to not put this output in the same directory as where the FFI files are. Just run:

```shell
cargo run --release -- -c YourGirFile.toml -d ../gir-files -o the-output-directory
```

Now it should be done. Just go to the output directory (so `the-output-directory/auto` in our case) and try to build using `cargo build`. Don't forget to update your dependencies in both projects: nothing much to do in the FFI/sys one but the Rust-user API level will need to have a dependency over the FFI/sys one.
