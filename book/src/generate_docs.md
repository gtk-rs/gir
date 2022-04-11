# Generating documentation

And finally the last feature! Just run the following command (note the `-m doc` at the end):

```console
> gir -c Gir.toml -d ../gir-files --doc-target-path docs.md -m doc
```

It'll generate a markdown file if everything went fine. That's where all this crate's documentation is. If you want to put it back into your crate's source code like "normal" doc comments, run:

```console
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
