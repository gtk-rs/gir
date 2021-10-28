# Generating the Rust API

Time to go back to the "global" sourceview folder:

```console
> cd ..
```

As you certainly guessed, we'll need a new `Gir.toml` file. Let's write it:

```toml
[options]
girs_directories = ["../gir-files"]
library = "GtkSource"
version = "3.0"
min_cfg_version = "3.0"
target_path = "."
work_mode = "normal"
generate_safety_asserts = true
deprecate_by_min_version = true
single_version_file = true

generate = []
```

A few new things in here. Let's take a look at them:

* `work_mode` value is now set to `normal`, it means it'll generate the high-level Rust api instead of the sys-level.
* `generate_safety_asserts` is used to generates checks to ensure that, or any other kind of initialization needed before being able to use the library.
* `deprecate_by_min_version` is used to generate a [Rust "#[deprecated]"](https://doc.rust-lang.org/edition-guide/rust-2018/the-compiler/an-attribute-for-deprecation.html) attribute based on the deprecation information provided by the `.gir` file.
* `single_version_file` is a very useful option when you have a lot of generated files (like we'll have). Instead of generating the gir hash commit used for the generation in the header of all generated files, it'll just write it inside one file, removing `git diff` noise **a lot**.
* `generate = []`: this line currently does nothing. We say to [gir] to generate nothing. We'll fulfill it later on.

Let's make a first generation of our high-level Rust API!

```console
> gir
```

Now if you take a look around, you'll see a new "auto" folder inside "src". Doesn't contain much though. Which makes sense since we're generating nothing. Time to introduce you to a whole new [gir] mode: `not_bound`. Let's give it a try:

```console
> gir -m not_bound
[NOT GENERATED] GtkSource.Buffer
[NOT GENERATED PARENT] Gtk.TextBuffer
[NOT GENERATED] GtkSource.Language
[NOT GENERATED] GtkSource.Mark
[NOT GENERATED PARENT] Gtk.TextMark
[NOT GENERATED] GtkSource.StyleScheme
[NOT GENERATED] GtkSource.UndoManager
[NOT GENERATED] GtkSource.SortFlags
[NOT GENERATED] GtkSource.Completion
[NOT GENERATED PARENT] Gtk.Buildable
[NOT GENERATED] GtkSource.CompletionProvider
[NOT GENERATED] GtkSource.CompletionContext
[NOT GENERATED PARENT] GObject.InitiallyUnowned
[NOT GENERATED] GtkSource.CompletionInfo
[NOT GENERATED PARENT] Gtk.Window
[NOT GENERATED PARENT] Gtk.Bin
[NOT GENERATED PARENT] Gtk.Container
[NOT GENERATED PARENT] Gtk.Widget
[NOT GENERATED PARENT] Atk.ImplementorIface
[NOT GENERATED] GtkSource.View
[NOT GENERATED PARENT] Gtk.TextView
[NOT GENERATED PARENT] Gtk.Scrollable
[NOT GENERATED] GtkSource.CompletionActivation
[NOT GENERATED] GtkSource.CompletionProposal
[NOT GENERATED] GtkSource.CompletionError
[NOT GENERATED] GtkSource.CompletionItem
[NOT GENERATED PARENT] GtkSource.CompletionProposal
[NOT GENERATED] GtkSource.CompletionWords
[NOT GENERATED PARENT] GtkSource.CompletionProvider
[NOT GENERATED] GtkSource.DrawSpacesFlags (deprecated in 3.24)
[NOT GENERATED] GtkSource.Encoding
[NOT GENERATED] GtkSource.File
[NOT GENERATED] GtkSource.MountOperationFactory
[NOT GENERATED] GtkSource.FileLoader
[NOT GENERATED] GtkSource.FileLoaderError
[NOT GENERATED] GtkSource.FileSaver
[NOT GENERATED] GtkSource.FileSaverFlags
[NOT GENERATED] GtkSource.FileSaverError
[NOT GENERATED] GtkSource.Gutter
[NOT GENERATED] GtkSource.GutterRenderer
[NOT GENERATED] GtkSource.GutterRendererState
[NOT GENERATED] GtkSource.GutterRendererAlignmentMode
[NOT GENERATED] GtkSource.GutterRendererPixbuf
[NOT GENERATED PARENT] GtkSource.GutterRenderer
[NOT GENERATED] GtkSource.GutterRendererText
[NOT GENERATED] GtkSource.LanguageManager
[NOT GENERATED] GtkSource.Map
[NOT GENERATED PARENT] GtkSource.View
[NOT GENERATED] GtkSource.MarkAttributes
[NOT GENERATED] GtkSource.PrintCompositor
[NOT GENERATED] GtkSource.Region
[NOT GENERATED] GtkSource.RegionIter
[NOT GENERATED] GtkSource.SearchContext
[NOT GENERATED] GtkSource.SearchSettings
[NOT GENERATED] GtkSource.Style
[NOT GENERATED] GtkSource.SpaceDrawer
[NOT GENERATED] GtkSource.SpaceTypeFlags
[NOT GENERATED] GtkSource.SpaceLocationFlags
[NOT GENERATED] GtkSource.StyleSchemeChooser
[NOT GENERATED] GtkSource.StyleSchemeChooserButton
[NOT GENERATED PARENT] Gtk.Button
[NOT GENERATED PARENT] Gtk.Actionable
[NOT GENERATED PARENT] Gtk.Activatable
[NOT GENERATED PARENT] GtkSource.StyleSchemeChooser
[NOT GENERATED] GtkSource.StyleSchemeChooserInterface
[NOT GENERATED] GtkSource.StyleSchemeChooserWidget
[NOT GENERATED] GtkSource.StyleSchemeManager
[NOT GENERATED] GtkSource.Tag
[NOT GENERATED PARENT] Gtk.TextTag
[NOT GENERATED] GtkSource.ViewGutterPosition
```

We now have the list of all the non-yet generated items. Quite convenient! You can also see that we have two kinds of not generated items:

* `[NOT GENERATED]`
* `[NOT GENERATED PARENT]`

`[NOT GENERATED PARENT]` means that this object lives in a dependency of the current library. We'll come back on how to add them a bit later.

Let's start by generating one type. Let's update the "generate" array as follows:

```toml
generate = [
    "GtkSource.Language",
]
```

Another `gir` run:

```console
> gir
```

(Again, if you do it on another library and it fails and you can't figure out why, don't hesitate to reach us!)

We now have a `src/auto/language.rs` file. We need to include all `auto` files in our library. To do so, let's update the `src/lib.rs` file as follows:

```rust
pub use auto::*;

mod auto;
```

Let's compile:

```console
> cargo build
```

It completely failed with a lot of errors. Yeay!

You guessed it, we need to add a few dependencies to make it work. A lot of those errors were about the fact that the `Language` type didn't exist. Which is weird since we generated it, right? Well, if you take a look at the `src/auto/language.rs` file, you'll see this at the top:

```rust
glib_wrapper! {
    pub struct Language(Object<ffi::GtkSourceLanguage, ffi::GtkSourceLanguageClass, LanguageClass>);

    match fn {
        get_type => || gtk_source_sys::gtk_source_language_get_type(),
    }
}
```

This macro comes from the `glib` crate. We didn't import it, therefore the Rust compiler can't find it. We'll also need its `sys` part (the case of `glib` is a bit special).

A second issue is that we didn't import the `sourceview-sys` crate we generated. Gir produces code expecting this crate to be imported as "ffi" (which you can see in the definition of `Language` above), so we need to rename it in the `Cargo.toml` file, too.

Alongside those two (three if we count `glib-sys`!), we'll need both `libc` and `bitflags`. Let's fix all of those issues at once! For that, we need to update the `Cargo.toml`:

```toml
[package]
name = "sourceview"
version = "0.1.0"
authors = ["Guillaume Gomez <guillaume1.gomez@gmail.com>"]

[dependencies]
libc = "0.2"
bitflags = "1.0"

[dependencies.ffi]
package = "sourceview-sys"
path = "./sourceview-sys"

[dependencies.glib]
git = "https://github.com/gtk-rs/glib"

[dependencies.glib-sys]
git = "https://github.com/gtk-rs/sys" # all gtk-rs sys crates are in the sys repository
```

Let's try to rebuild:

```console
> cargo build
```

It worked! We have generated the `Language` item! I'll let you take a look at the `src/auto/language.rs` file, then we can continue.

Again, if you encounter any issue at this stage (if the generated code is invalid for example), don't hesitate to reach us so we can give you a hand!

We'll now generate the `GtkSource.Region` type. Why this one? Well, I don't want to spoil the surprise so just wait for a bit!

First, we need to add it into the types to generate into our `Gir.toml` file:

```toml
generate = [
    "GtkSource.Language",
    "GtkSource.Region",
]
```

We regenerate:

```console
> gir
```

We rebuild:

```console
> cargo build
```

Everything works, yeay! Now if we take a look at our newly generated `src/auto/region.rs`, we'll see code like this:

```rust
//#[cfg(any(feature = "v3_22", feature = "dox"))]
//fn add_subregion(&self, _start: /*Ignored*/&gtk::TextIter, _end: /*Ignored*/&gtk::TextIter);

//#[cfg(any(feature = "v3_22", feature = "dox"))]
//fn get_buffer(&self) -> /*Ignored*/Option<gtk::TextBuffer>;
```

Some functions are commented. Why so? The reason is simple: we need to tell to `gir` that those types have been generated and that it can generate code using them. We can do it by adding the type into the "manual" list. To put it simply, when [gir] sees an item into this "manual" list, it means to it "this type has been generated somewhere else, you can use it just like the others".

Let's update our `Gir.toml` file once again:

```toml
generate = [
    "GtkSource.Language",
    "GtkSource.Region",
]

manual = [
    "Gtk.TextIter",
    "Gtk.TextBuffer",
]
```

We'll also need to import the `gtk` crate. Let's add it into our `Cargo.toml` file:

```toml
[dependencies.gtk]
git = "https://github.com/gtk-rs/gtk"
```

We regenerate and rebuild:

```console
> gir
> cargo build
```

Everything is working, yeay! If you take another look at `src/auto/region.rs`, you'll see a lot less commented functions. Amongst the remaining ones, you'll see this one:

```rust
//#[cfg(any(feature = "v3_22", feature = "dox"))]
//fn get_start_region_iter(&self, iter: /*Ignored*/RegionIter);
```

If a type name isn't prepend by `[crate_name]::`, then it means it comes from the current crate. To add it, just put it into the "generate" list of `Gir.toml`.

At this point, you should have almost everything you need. There is just one last case we need to talk about.

[gir]: https://github.com/gtk-rs/gir
