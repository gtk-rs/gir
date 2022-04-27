# Generating documentation
And finally the last feature! Just run the following command in the folder of the safe wrapper crate:

```console
> gir -c Gir.toml -d ../gir-files --doc-target-path docs.md -m doc
```

* `-d ../gir-files`: flag to select the folder containing the .gir files.
* `--doc-target-path docs.md`: flag to select the name of the markdown file containing the documentation.
* `-m doc`: flag to select the work mode for generating the documentation.

It'll generate a markdown file if everything went fine. It contains all of the crate's documentation. If you want to put it into your crate's source code like "normal" doc comments, run:

```console
> cargo install rustdoc-stripper
> rustdoc-stripper -g -o docs.md
```

And now your crate should be completely documented as expected!

Running the above commands again would duplicate the doc comments. Make sure to first remove the old ones before running the command again. You can do this by running the following commands:

```console
> rustdoc-stripper -s -n
> rustdoc-stripper -g -o docs.md
```

Try building the documentation and also try it with the various features you might have

```console
> cargo doc
```

Congratulations, we are done! You have successfully created the safe wrapper for a C library!