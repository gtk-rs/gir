## Generate something from GIR

Running:
```shell
cargo run --release -- -c Gir_Gtk.toml -d ../gir-files -o gtk
```
where `gir-files` contains the [GIR definitions](https://github.com/gkoz/gir-files).
The generated files will be placed in `gtk`
