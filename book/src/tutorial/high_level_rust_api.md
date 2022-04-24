# Generating the Rust API
In the previous step we successfully created the unsafe bindings of the -sys crate. Let's change into the directory of the safe wrapper crate.

## The Cargo.toml file
The Cargo.toml file will not be replaced when you run gir. So it is our responsibility to make sure the information in it is correct. Open the Cargo.toml file and have a look at it. Make sure everything under `[package]` is to your liking.

Add the following lines to the file:
```toml
[package.metadata.docs.rs]
features = ["dox"]
```
This automatically activates the `dox` feature if you chose to publish the bindings and docs.rs tries to build the documentation. If you are not going to maintain the crate and don't want to publish it, this line is not going to hurt.

We also need to add libc, bitflags, glib and glib-sys and all other dependencies we used in the sys crate as dependencies. Because we are creating a wrapper for the sys crate, which we generated in the previous chapter, we also need to add the sys crate to the list of dependencies. In the automatically generated code, the sys crate is always called `ffi`, so we need to rename the sys crate in our Cargo.toml. For our example, this results in the following dependencies:
```toml
[dependencies]
libc = "0.2"
bitflags = "1.0"

[dependencies.ffi]
package = "sourceview-sys"
path = "../sourceview-sys"

[dependencies.glib]
git = "https://github.com/gtk-rs/glib"

[dependencies.glib-sys]
git = "https://github.com/gtk-rs/sys" # all gtk-rs sys crates are in the sys repository

[dependencies.gtk]
git = "https://github.com/gtk-rs/gtk"
```

In order to make the features of the sys crate available for users of your safe wrapper, you need to add features. Copy the `[features]` part of the Cargo.toml of your sys crate and paste it into the Cargo.toml of the normal crate. The features are supposed to activate the corresponding features of the sys crate, so you need to make some changes. If for example you have the following sys features:

```toml
[features]
v0_4 = []
v0_5 = ["v0_4"]
v0_6 = ["v0_5"]
default = ["v0_6"]
dox = []
```

You need to change the features in the Cargo.toml of your normal crate to

```toml
[features]
v0_4 = ["ffi/v0_4"]
v0_5 = ["ffi/v0_5", "v0_4"]
v0_6 = ["ffi/v0_6", "v0_5"]
default = ["v0_6"]
dox = ["ffi/dox"]
```

## The lib.rs file
The lib.rs file will not be replaced when you run gir. All the code that gir will generate for us is going to be in src/auto. We need to include all `auto` files in our library. To do so, let's update the `src/lib.rs` file as follows:

```rust
#![cfg_attr(feature = "dox", feature(doc_cfg))]

pub use auto::*;
mod auto;
```


## The Gir.toml file
As you certainly guessed, we have to fill our `Gir.toml` file for the normal crate as well. Let's write it:

```toml
[options]
library = "GtkSource"
version = "3.0"
min_cfg_version = "3.0"
target_path = "."
girs_directories = ["../gir-files"]
work_mode = "normal"
generate_safety_asserts = true
deprecate_by_min_version = true
single_version_file = true

generate = []
```

Many of these options look familiar from the last chapter but there are also a few new things in here. Let's take a look at them:

* `work_mode` value is now set to `normal`, it means it'll generate the high-level Rust api instead of the sys-level.
* `generate_safety_asserts` is used to generates checks to ensure that, or any other kind of initialization needed before being able to use the library.
* `deprecate_by_min_version` is used to generate a [Rust "#[deprecated]"](https://doc.rust-lang.org/edition-guide/rust-2018/the-compiler/an-attribute-for-deprecation.html) attribute based on the deprecation information provided by the `.gir` file.
* `single_version_file` is a very useful option when you have a lot of generated files (like we'll have). Instead of generating the gir hash commit used for the generation in the header of all generated files, it'll just write it inside one file, removing `git diff` noise **a lot**.
* `generate = []`: this line currently does nothing. We say to [gir] to generate nothing. We'll fulfill it later on.

Let's make a first generation of our high-level Rust API!

```console
> gir -o .
```

If you take a look at which files and folders were created, you'll see a new "auto" folder inside "src". This folder contains all the generated code. It doesn't contain anything though. Which makes sense since we're generating nothing.

Now it's time to introduce you to a whole new [gir] mode: `not_bound`. Let's give it a try:

```console
> gir -o . -m not_bound
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

We now have the list of all the non-yet generated items. Quite convenient! There can be different kinds of not generated items:

* `[NOT GENERATED]`
Objects marked with `[NOT GENERATED]` are objects that we can generate, but we did not (yet) add to the `generate` array.
* `[NOT GENERATED PARENT]`
These objects live in a dependency of the current library. These are the objects we will add to the `manual` array in the following steps.
* `[NOT GENERATED FUNCTION]`
These are global functions that were not generated. This will not be the case in our example, but if you see this with your own library, just add `"NameOfYourLibrary.*"` to the `generate` array in the Git.toml and add the following line to your src/lib.rs file:
```rust
pub use auto::functions::*;
```

## Generating the code
In order to generate the code for the safe wrapper, we follow these steps until all objects have been generated:

- Run `gir -o . -m not_bound` to see which objects have not been generated yet
- Pick one of the types marked with `[NOT GENERATED]`
- Add it to the `generate` array in the Gir.toml file
- Run `gir -o .` to generate the code
- Open the generated files under src/auto and have a look at them
- Search for `/*Ignored*/`. If the type name following `/*Ignored*/` is prepended by `[crate_name]::` (e.g `/*Ignored*/&gtk::TextIter`),
    - then we add it to the `manual` array (e.g gtk). By doing so we tell [gir] that those types have been generated somewhere else and that they can be used just like the other types.
    - Otherwise, the type comes from the current crate and we just put it into the `generate` list of the `Gir.toml` file.
- Start with the first step again
    
The names of the objects are not the same as the crates names. You have to use the names of the corresponding gir files.

Okay, lets go through that process for a few objects of our example.

TODO: Add steps of example

Again, if you do it on another library and it fails and you can't figure out why, don't hesitate to reach us!

At this point, you should have almost everything you need. There is just one last case we need to talk about.


[gir]: https://github.com/gtk-rs/gir
