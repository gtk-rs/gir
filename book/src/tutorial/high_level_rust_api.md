# Generating the Rust API
In the previous step we successfully created the unsafe bindings of the -sys crate.
We are now in the directory of the safe wrapper crate (`gir-tutorial/pango`).

## The Cargo.toml file
The Cargo.toml file will not be replaced when you run gir.
So it is our responsibility to make sure the information in it is correct.
Open the Cargo.toml file and have a look at it.
Make sure everything under `[package]` is to your liking.

Add the following lines to the file:
```toml
[package.metadata.docs.rs]
all-features = true
# For build.rs scripts
rustc-args = ["--cfg", "docsrs"]
# For rustdoc
rustdoc-args = ["--cfg", "docsrs"]
```
This automatically activates the `docsrs` attribute if you chose to publish the bindings and docs.rs tries to build the documentation.
With the `docsrs` attribute, the crate skips linking the C libraries and allows docs.rs to build the documentation without having the underlying libraries installed.
Even if you don't plan to publish it, this line is not going to hurt.

We also need to add `libc`, `bitflags`, `glib` and `glib-sys` and all other dependencies we used in the sys crate as dependencies.
Because we are creating a wrapper for the sys crate, which we generated in the previous chapter, we also need to add the sys crate to the list of dependencies.
In the automatically generated code, the sys crate is always called `ffi`, so we need to rename the sys crate in our `Cargo.toml`.
For our example, this results in the following dependencies:
```toml
[dependencies]
libc = "0.2"
bitflags = "2.2"

[dependencies.ffi]
package = "pango-sys"
path = "./pango-sys"

[dependencies.glib]
package = "glib-sys"
git = "https://github.com/gtk-rs/gtk-rs-core"

[dependencies.gobject]
package = "gobject-sys"
git = "https://github.com/gtk-rs/gtk-rs-core"
```

In order to make the features of the sys crate available for users of your safe wrapper, you need to add features.
Copy the `[features]` part of the Cargo.toml of your sys crate and paste it into the Cargo.toml of the normal crate.
The features are supposed to activate the corresponding features of the sys crate, so you need to make some changes.
If for example you have the following sys features:

```toml
[features]
v1_2 = []
v1_4 = ["v1_2"]
v1_6 = ["v1_4"]
v1_8 = ["v1_6"]
v1_10 = ["v1_8"]
v1_12 = ["v1_10"]
v1_14 = ["v1_12"]
v1_16 = ["v1_14"]
v1_18 = ["v1_16"]
v1_20 = ["v1_18"]
v1_22 = ["v1_20"]
v1_24 = ["v1_22"]
v1_26 = ["v1_24"]
v1_30 = ["v1_26"]
v1_31 = ["v1_30"]
v1_32 = ["v1_31"]
v1_32_4 = ["v1_32"]
v1_34 = ["v1_32_4"]
v1_36_7 = ["v1_34"]
v1_38 = ["v1_36_7"]
v1_42 = ["v1_38"]
v1_44 = ["v1_42"]
v1_46 = ["v1_44"]
v1_48 = ["v1_46"]
v1_50 = ["v1_48"]
v1_52 = ["v1_50"]
```

You need to change the features in the Cargo.toml of your normal crate to

```toml
[features]
v1_2 = ["ffi/v1_2"]
v1_4 = ["ffi/v1_4", "v1_2"]
v1_6 = ["ffi/v1_6", "v1_4"]
v1_8 = ["ffi/v1_8", "v1_6"]
v1_10 = ["ffi/v1_10", "v1_8"]
v1_12 = ["ffi/v1_12", "v1_10"]
v1_14 = ["ffi/v1_14", "v1_12"]
v1_16 = ["ffi/v1_16", "v1_14"]
v1_18 = ["ffi/v1_18", "v1_16"]
v1_20 = ["ffi/v1_20", "v1_18"]
v1_22 = ["ffi/v1_22", "v1_20"]
v1_24 = ["ffi/v1_24", "v1_22"]
v1_26 = ["ffi/v1_26", "v1_24"]
v1_30 = ["ffi/v1_30", "v1_26"]
v1_31 = ["ffi/v1_31", "v1_30"]
v1_32 = ["ffi/v1_32", "v1_31"]
v1_32_4 = ["ffi/v1_32_4", "v1_32"]
v1_34 = ["ffi/v1_34", "v1_32_4"]
v1_36_7 = ["ffi/v1_36_7", "v1_34"]
v1_38 = ["ffi/v1_38", "v1_36_7"]
v1_42 = ["ffi/v1_42", "v1_38"]
v1_44 = ["ffi/v1_44", "v1_42"]
v1_46 = ["ffi/v1_46", "v1_44"]
v1_48 = ["ffi/v1_48", "v1_46"]
v1_50 = ["ffi/v1_50", "v1_48"]
v1_52 = ["ffi/v1_52", "v1_50"]
```

## The lib.rs file
The lib.rs file will not be replaced when you run gir.
All the code that gir will generate for us is going to be in src/auto.
We need to include all `auto` files in our library.
To do so, let's update the `src/lib.rs` file as follows:

```rust
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use auto::*;
mod auto;
```


## The Gir.toml file
As you certainly guessed, we have to fill our `Gir.toml` file for the normal crate as well.
Let's write it:

```toml
[options]
library = "Pango"
version = "1.0"
min_cfg_version = "1.0"
target_path = "."
girs_directories = ["../gir-files"]
work_mode = "normal"
single_version_file = true
generate_safety_asserts = true
deprecate_by_min_version = true

generate = []

manual = []
```

Many of these options look familiar from the last chapter but there are also a few new things in here.
Let's take a look at them:

* `work_mode` value is now set to `normal`, it means it'll generate the high-level Rust api instead of the sys-level.
* `generate_safety_asserts` is used to generates checks to ensure that, or any other kind of initialization needed before being able to use the library.
* `deprecate_by_min_version` is used to generate a [Rust "#[deprecated]"](https://doc.rust-lang.org/edition-guide/rust-2018/the-compiler/an-attribute-for-deprecation.html) attribute based on the deprecation information provided by the `.gir` file.
* `generate = []`: this line currently does nothing.
We say to [gir] to generate nothing.
We'll fill it later on.
* `manual = []`: this line currently does nothing.
We can let [gir] know about objects which it does not have to generate code for.

Let's make a first generation of our high-level Rust API!

```sh
gir -o .
```

If you take a look at which files and folders were created, you'll see a new "auto" folder inside "src".
This folder contains all the generated code.
It doesn't contain anything though.
Which makes sense since we're generating nothing.

Now it's time to introduce you to a whole new [gir] mode: `not_bound`.
Let's give it a try:

```console
> gir -o . -m not_bound
[NOT GENERATED] Pango.Glyph
[NOT GENERATED] Pango.GlyphUnit
[NOT GENERATED] Pango.GlyphItem
[NOT GENERATED] Pango.LayoutRun
[NOT GENERATED] Pango.Alignment
[NOT GENERATED] Pango.Font
[NOT GENERATED] Pango.Language
[NOT GENERATED] Pango.Analysis
[NOT GENERATED] Pango.AttrType
[NOT GENERATED] Pango.Attribute
[NOT GENERATED] Pango.Color
[NOT GENERATED] Pango.AttrColor
[NOT GENERATED] Pango.AttrFloat
[NOT GENERATED] Pango.FontDescription
[NOT GENERATED] Pango.AttrFontDesc
[NOT GENERATED] Pango.AttrFontFeatures
[NOT GENERATED] Pango.AttrInt
[NOT GENERATED] Pango.AttrIterator
[NOT GENERATED] Pango.AttrLanguage
[NOT GENERATED] Pango.AttrList
[NOT GENERATED] Pango.Rectangle
[NOT GENERATED] Pango.AttrShape
[NOT GENERATED] Pango.AttrSize
[NOT GENERATED] Pango.AttrString
[NOT GENERATED] Pango.BaselineShift
[NOT GENERATED] Pango.BidiType (deprecated in 1.44)
[NOT GENERATED] Pango.Context
[NOT GENERATED] Pango.Direction
[NOT GENERATED] Pango.Gravity
[NOT GENERATED] Pango.FontMap
[NOT GENERATED] Pango.GravityHint
[NOT GENERATED] Pango.Matrix
[NOT GENERATED] Pango.FontMetrics
[NOT GENERATED] Pango.FontFamily
[NOT GENERATED] Pango.Fontset
[NOT GENERATED] Pango.Coverage
[NOT GENERATED] Pango.CoverageLevel
[NOT GENERATED] Pango.EllipsizeMode
[NOT GENERATED] Pango.FontFace
[NOT GENERATED] Pango.FontMask
[NOT GENERATED] Pango.Stretch
[NOT GENERATED] Pango.Style
[NOT GENERATED] Pango.Variant
[NOT GENERATED] Pango.Weight
[NOT GENERATED] Pango.FontScale
[NOT GENERATED] Pango.FontsetSimple
[NOT GENERATED PARENT] Pango.Fontset
[NOT GENERATED] Pango.GlyphGeometry
[NOT GENERATED] Pango.GlyphVisAttr
[NOT GENERATED] Pango.GlyphInfo
[NOT GENERATED] Pango.Item
[NOT GENERATED] Pango.GlyphString
[NOT GENERATED] Pango.LogAttr
[NOT GENERATED] Pango.GlyphItemIter
[NOT GENERATED] Pango.Script
[NOT GENERATED] Pango.Layout
[NOT GENERATED] Pango.LayoutDeserializeFlags
[NOT GENERATED] Pango.LayoutIter
[NOT GENERATED] Pango.LayoutLine
[NOT GENERATED] Pango.TabArray
[NOT GENERATED] Pango.WrapMode
[NOT GENERATED] Pango.LayoutSerializeFlags
[NOT GENERATED] Pango.LayoutDeserializeError
[NOT GENERATED] Pango.Overline
[NOT GENERATED] Pango.RenderPart
[NOT GENERATED] Pango.Renderer
[NOT GENERATED] Pango.Underline
[NOT GENERATED] Pango.ScriptIter
[NOT GENERATED] Pango.ShapeFlags
[NOT GENERATED] Pango.ShowFlags
[NOT GENERATED] Pango.TabAlign
[NOT GENERATED] Pango.TextTransform
[NOT GENERATED FUNCTION] Pango.attr_allow_breaks_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_background_alpha_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_background_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_baseline_shift_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_break because of Pango.AttrList and Pango.LogAttr
[NOT GENERATED FUNCTION] Pango.attr_fallback_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_family_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_font_scale_new because of Pango.FontScale and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_foreground_alpha_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_foreground_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_gravity_hint_new because of Pango.GravityHint and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_gravity_new because of Pango.Gravity and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_insert_hyphens_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_letter_spacing_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_line_height_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_line_height_new_absolute because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_overline_color_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_overline_new because of Pango.Overline and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_rise_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_scale_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_sentence_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_show_new because of Pango.ShowFlags and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_stretch_new because of Pango.Stretch and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_strikethrough_color_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_strikethrough_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_style_new because of Pango.Style and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_text_transform_new because of Pango.TextTransform and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_underline_color_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_underline_new because of Pango.Underline and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_variant_new because of Pango.Variant and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_weight_new because of Pango.Weight and Pango.Attribute
[NOT GENERATED FUNCTION] Pango.attr_word_new because of Pango.Attribute
[NOT GENERATED FUNCTION] Pango.break (deprecated in 1.44) because of Pango.Analysis and Pango.LogAttr
[NOT GENERATED FUNCTION] Pango.default_break because of Pango.Analysis and Pango.LogAttr
[NOT GENERATED FUNCTION] Pango.extents_to_pixels because of Pango.Rectangle and Pango.Rectangle
[NOT GENERATED FUNCTION] Pango.find_base_dir because of Pango.Direction
[NOT GENERATED FUNCTION] Pango.get_log_attrs because of Pango.Language and Pango.LogAttr
[NOT GENERATED FUNCTION] Pango.itemize because of Pango.Context, Pango.AttrList, Pango.AttrIterator and Pango.Item
[NOT GENERATED FUNCTION] Pango.itemize_with_base_dir because of Pango.Context, Pango.Direction, Pango.AttrList, Pango.AttrIterator and Pango.Item
[NOT GENERATED FUNCTION] Pango.log2vis_get_embedding_levels because of Pango.Direction
[NOT GENERATED FUNCTION] Pango.markup_parser_finish because of GLib.MarkupParseContext, Pango.AttrList and GLib.Error
[NOT GENERATED FUNCTION] Pango.markup_parser_new because of GLib.MarkupParseContext
[NOT GENERATED FUNCTION] Pango.parse_markup because of Pango.AttrList and GLib.Error
[NOT GENERATED FUNCTION] Pango.parse_stretch because of Pango.Stretch
[NOT GENERATED FUNCTION] Pango.parse_style because of Pango.Style
[NOT GENERATED FUNCTION] Pango.parse_variant because of Pango.Variant
[NOT GENERATED FUNCTION] Pango.parse_weight because of Pango.Weight
[NOT GENERATED FUNCTION] Pango.read_line (deprecated in 1.38) because of GLib.String
[NOT GENERATED FUNCTION] Pango.reorder_items because of Pango.Item and Pango.Item
[NOT GENERATED FUNCTION] Pango.scan_string (deprecated in 1.38) because of GLib.String
[NOT GENERATED FUNCTION] Pango.scan_word (deprecated in 1.38) because of GLib.String
[NOT GENERATED FUNCTION] Pango.shape because of Pango.Analysis and Pango.GlyphString
[NOT GENERATED FUNCTION] Pango.shape_full because of Pango.Analysis and Pango.GlyphString
[NOT GENERATED FUNCTION] Pango.shape_item because of Pango.Item, Pango.LogAttr, Pango.GlyphString and Pango.ShapeFlags
[NOT GENERATED FUNCTION] Pango.shape_with_flags because of Pango.Analysis, Pango.GlyphString and Pango.ShapeFlags
[NOT GENERATED FUNCTION] Pango.tailor_break because of Pango.Analysis and Pango.LogAttr
[NOT GENERATED FUNCTION] Pango.unichar_direction because of Pango.Direction
```

We now have the list of all the not-yet generated items.
Quite convenient.
There can be different kinds of not generated items:

* `[NOT GENERATED]`:
Objects marked with `[NOT GENERATED]` are objects that we can generate, but we did not (yet) add to the `generate` array.
* `[NOT GENERATED PARENT]`:
These objects live in a dependency of the current library.
These are the objects we will add to the `manual` array in the following steps.
* `[NOT GENERATED FUNCTION]`:
These are global functions that were not generated.
To fix it, we just add `"NameOfYourLibrary.*"` to the `generate` array in the Git.toml and add the following line to your src/lib.rs file:
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
- Search for `/*Ignored*/`.
If the type name following `/*Ignored*/` is prepended by `[crate_name]::` (e.g `Ignored*/&glib::MarkupParseContext`),
    - then we add it to the `manual` array.
      By doing so we tell [gir] that those types have been generated somewhere else and that they can be used just like the other types.
    - Otherwise, the type comes from the current crate and we just put it into the `generate` list of the `Gir.toml` file.
- Start with the first step again
    
The names of the objects are not the same as the crates names.
You have to use the names of the corresponding gir files.

Okay, let's go through that process for a few objects of our example.

🚧 TODO: Add remaining steps of the pango example 🚧

Again, if you do it on another library and it fails and you can't figure out why, don't hesitate to [contact us](https://gtk-rs.org/contact)!

At this point, you should have almost everything you need.
Let's have a look at errors that can happen in this process.


[gir]: https://github.com/gtk-rs/gir
