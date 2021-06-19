# Handling generation errors

There are a few kinds of errors (not much luckily) which can happen with [gir] generation. Let's take a look at them.

## Missing memory management functions

If [gir] generation fails (for whatever reason), it means you'll have to implement the type yourself. Just like types from other `gtk-rs` crates, you'll need to put it into the "manual" list. Then you need to put the type into the `src` folder (or inside a subfolder, you know how Rust works).

/!\ Don't forget to reexport the type inside your `src/lib.rs` file! For example, let's take a look at the [requisition.rs](https://github.com/gtk-rs/gtk/blob/master/src/requisition.rs) file from the `gtk` crate.

Since it's a "simple" type (no pointer, therefore no memory management to do), [gir] doesn't know how to generate it. You'll need to implement some traits by hand like `ToGlibPtr` or `ToGlibPtrMut` (depending on your needs).

## Bad function generation

In some cases, the generated code isn't correct (array parameters are often an issue). In such cases, it's better to just make the implementation yourself. As an example, let's say you want to implement `Region::is_empty` yourself. A few changes have to be made. Let's start with `Gir.toml`:

```toml
generate = [
    "GtkSource.Language",
]

[[object]]
name = "GtkSource.Region"
status = "generate"
    [[object.function]]
    name = "is_empty"
    ignore = true
```

So to sum up what I wrote above: we removed "GtkSource.Region" from the "generate" list and we created a new entry for it. Then we say to [gir] that it should generate (through `status = "generate"`). However, we also tell it that we don't want the "is_empty" to be generated.

Now that we've done that, we need to reimplement it. Let's create a `src/region.rs` file:

```rust
use glib::object::IsA;
use glib::translate::*;
use Region;

pub trait RegionExtManual: 'static {
    pub fn is_empty(&self) -> bool;
}

impl<O: IsA<Region>> RegionExtManual for O {
    pub fn is_empty(&self) -> bool {
        // blablabla
        true
    }
}
```

You might wonder: "why not just implementing it on the `Region` type directly?". Because like this, a subclass will also be able to use this trait easily as long as it implements `IsA<Region>`. For instance, in gtk, everything that implements `IsA<Widget>` (so almost every GTK types) can use those methods.

As usual, don't forget to reexport the trait. A little tip about reexporting manual traits: in `gtk-rs`, we create a `src/prelude.rs` file which reexports all traits (both manual and generated ones), making it simpler for users to use them through `use [DEPENDENCY]::prelude::*`. It looks like this:

```rust
pub use auto::traits::*;

pub use region::RegionExtManual;
```

Then it's reexported as follows from the `src/lib.rs` file:

```rust
pub mod prelude;

pub use prelude::*;
```

[gir]: https://github.com/gtk-rs/gir
