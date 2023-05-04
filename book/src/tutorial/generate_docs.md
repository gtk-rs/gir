# Generating documentation
And finally the last feature.
Just run the following command in the folder of the safe wrapper crate:

```sh
gir -c Gir.toml -d ../gir-files --doc-target-path docs.md -m doc
```

* `-d ../gir-files`: flag to select the folder containing the .gir files.
* `--doc-target-path docs.md`: flag to select the name of the markdown file containing the documentation.
* `-m doc`: flag to select the work mode for generating the documentation.

It'll generate a markdown file if everything went fine.
It contains all of the crate's documentation.
If you want to put it into your crate's source code like "normal" doc comments, run:

```sh
cargo install rustdoc-stripper
rustdoc-stripper -g -o docs.md
```

And now your crate should be completely documented as expected!

Running the above commands again would duplicate the doc comments.
Make sure to first remove the old ones before running the command again.
You can do this by running the following commands:

```sh
rustdoc-stripper -s -n
rustdoc-stripper -g -o docs.md
```

Try building the documentation and also try it with the various features you might have:

```sh
cargo doc
```

For building the docs on systems without the required dependencies installed (for example docs.rs or CI jobs), you can use the `docsrs` attribute.
This will disable the linking of the underlying C libraries and won't throw an error if those libraries are not available.

On top of that, the `docsrs` attribute will enable the [doc_cfg feature](https://doc.rust-lang.org/beta/unstable-book/language-features/doc-cfg.html) which is only available in the nightly compiler toolchain at the time of writing.
This is mostly useful for clarifying feature requirements through the docs.

To build the docs with the `docsrs` attribute, you can use the following command:

```sh
RUSTFLAGS='--cfg docsrs' RUSTDOCFLAGS='--cfg docsrs' cargo +nightly doc --all-features
```

Congratulations, we are done.
You have successfully created the safe wrapper for a C library!

You can easily publish your generated bindings and the wrapper to crates.io to allow others to use it.
Publishing crates is easy but keep in mind that they need to be maintained as well.
We set up the project folder in a way that easily allows sharing the code.
All that is needed is to add some information to your Cargo.toml.
Gir will not override them when you re-generate bindings.
Easy, right.
If this is your first time publishing a crate, you can find a detailed guide [here](https://doc.rust-lang.org/cargo/reference/publishing.html).

Before you publish the crate, please ensure docs.rs will activate the `docsrs` attribute.
Feel free to go back to the chapter about the [Cargo.toml file of the safe wrapper](high_level_rust_api.md#the-cargotoml-file) to read more about it.
If you skip this step, your crate and all crates depending on it will not have documentation available on docs.rs.