# Handling generation errors

Luckily there are only a few errors which can happen with [gir] generation.
Let's take a look at them.

### Cannot find macros

Compilation of the generated bindings may fail with errors like the following:

```console
error: cannot find macro `skip_assert_initialized` in this scope
  --> src/auto/enums.rs:83:9
   |
83 |         skip_assert_initialized!();
   |         ^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: have you added the `#[macro_use]` on the module/import?

error: cannot find macro `assert_initialized_main_thread` in this scope
  --> src/auto/example.rs:33:9
   |
33 |         assert_initialized_main_thread!();
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: have you added the `#[macro_use]` on the module/import?
```

In this case you’ll have to implement them yourself.
Macros are order-dependent and you *must* insert this code before declaring modules that use it (e.g. `mod auto`).
For example, you can add the following to your `lib.rs` file:

```rust
/// No-op.
macro_rules! skip_assert_initialized {
    () => {};
}

/// Asserts that this is the main thread and either `gdk::init` or `gtk::init` has been called.
macro_rules! assert_initialized_main_thread {
    () => {
        if !::gtk::is_initialized_main_thread() {
            if ::gtk::is_initialized() {
                panic!("GTK may only be used from the main thread.");
            } else {
                panic!("GTK has not been initialized. Call `gtk::init` first.");
            }
        }
    };
}
```


One complication here is that the `assert_initialized_main_thread!` macro depends on the exact library.
If it's GTK-based then the above macro is likely correct, unless the library has its own initialization function.
If it has its own initialization function it would need to be handled in addition to GTK's here in the same way.

For non-GTK-based libraries the following macro would handle the initialization function of that library in the same way, or if there is none it would simply do nothing:

```rust
/// No-op.
macro_rules! assert_initialized_main_thread {
    () => {};
}
```

## Missing memory management functions

If [gir] generation fails (for whatever reason), it means you'll have to implement the type yourself.
Just like types from other `gtk-rs` crates, you'll need to put it into the "manual" list.
Then you need to put the type into the `src` folder (or inside a subfolder, you know how Rust works).

/!\ Don't forget to reexport the type inside your `src/lib.rs` file.
For example, let's take a look at the [requisition.rs](https://github.com/gtk-rs/gtk3-rs/blob/master/gtk/src/requisition.rs) file from the `gtk3` crate.

Since it's a "simple" type (no pointer, therefore no memory management to do), [gir] doesn't know how to generate it.
You'll need to implement some traits by hand like `ToGlibPtr` or `ToGlibPtrMut` (depending on your needs).

## Bad function generation
In some cases, the generated code isn't correct (array parameters are often an issue).
In such cases, it's better to just make the implementation yourself.
As an example, let's say you want to implement `Region::is_empty` yourself.
A few changes have to be made.
Let's start with `Gir.toml`:

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

So to sum up what was written above: we removed "GtkSource.Region" from the "generate" list and we created a new entry for it.
Then we say to [gir] that it should generate (through `status = "generate"`).
However, we also tell it that we don't want the "is_empty" function to be generated.

Now that we've done that, we need to implement it.
Let's create a `src/region.rs` file:

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

You might wonder: "why not just implementing it on the `Region` type directly?".
Because like this, a subclass will also be able to use this trait easily as long as it implements `IsA<Region>`.
For instance, in gtk, everything that implements `IsA<Widget>` (so almost every GTK types) can use those methods.

As usual, don't forget to reexport the trait.
A little tip about reexporting manual traits: in `gtk3-rs`, we create a `src/prelude.rs` file which reexports all traits (both manual and generated ones), making it simpler for users to use them through `use [DEPENDENCY]::prelude::*`.
The `src/prelude.rs` file looks like this:

```rust
pub use auto::traits::*;
pub use region::RegionExtManual;
```

Then it's reexported as follows from the `src/lib.rs` file:

```rust
pub mod prelude;
pub use prelude::*;
```

## Manually defined traits missing from the documentation
If you defined traits manually, you can add them to the "Implements" section in the documentation for classes and interfaces by using the `manual_traits = []` option in the `Gir.toml` file.
Here is an example:

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


[gir]: https://github.com/gtk-rs/gir
