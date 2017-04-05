# GIR

The `GIR` is used to generate both sys level and Rust-user API level.

## Generate FFI (sys level)

You first need to get the Gir file corresponding to the project you want to bind. You can get them from [here](https://github.com/gtk-rs/gir-files) or directly on [ubuntu website](http://packages.ubuntu.com/) (for example: http://packages.ubuntu.com/zesty/amd64/libgtk-3-dev).

Then you need to write a TOML file (let's call it YourSysGirFile.toml) that you'll pass to gir (you can take a look to [gtk-rs/sys/gir-gtk.toml](https://github.com/gtk-rs/sys/blob/master/conf/gir-gtk.toml) to see an example).

*This part needs more information about this toml file*

When you're ready, let's generate the FFI part:

```shell
cargo run --release -c YourSysGirFile.toml -d ../gir-files -m sys -o the-output-directory-sys
```

The generated files will be placed in `the-output-directory-sys`. You now have the sys part of your binding!

## Generate the Rust-user API level

You'll now have to write another GIR file (take a look to [gtk/Gir.toml](https://github.com/gtk-rs/gtk/blob/master/Gir.toml) for an example).

*This part needs more information about this toml file*

To generate the Rust-user API level, The command is very similar to the previous one. It's better to not put this output in the same directory as where the FFI files are. Just run:

```shell
cargo run --release -- -c YourGirFile.toml -d ../gir-files -o the-output-directory
```

Now it should be done. Just go to the output directory and try building using `cargo build`. Don't forget to update your dependencies in both projects: nothing much to do in the FFI/sys one but the Rust-user API level will need to have a dependency over the FFI/sys one.
