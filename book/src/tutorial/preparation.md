# Preparation
In order to install gir and nicely structure the project, there are a few things to set up.

## Set up project folder
In order to keep the project folder nicely organized, lets create a folder where we will work in and initialize the repo. We will create two library crates. pango will contain the safe wrapper crate and because it is a wrapper for the unsafe bindings, we create the pango-sys crate within the pango crate. Sometimes there can be some minor issues if a Cargo.toml file is already present in the -sys crate. If no Cargo.toml file is present in the sys create, a new one will be generated, so lets be safe and delete the file before we begin. The following commands will set up the project folder as described. 
```console
> mkdir gir-tutorial
> cd gir-tutorial/
> git init
> cargo new pango --lib
> cd pango
> cargo new pango-sys --lib
> rm pango-sys/Cargo.toml
```
We will also create a file called "Gir.toml" in each of the crates.
```console
> touch Gir.toml
> touch pango-sys/Gir.toml
> cd ..
```

## Installing gir
Of course we also need to download and install [gir].
```console
> git submodule add https://github.com/gtk-rs/gir
> cd gir
> cargo install --path .
> cd ..
```
By adding it as a submodule, we are able to fetch future updates of the tool and we always exactly know which gir version we used to generate our bindings.

If there are any updates to gir in the future, we can install them by opening our project folder `gir-tutorial` and running
```console
> git submodule update --remote
> cd gir
> cargo install --path .
> cd ..
```

## Summary
You should now have a folder looking like this:
```text
gir
  |
  |---- ...
pango/
  |
  |---- Cargo.toml
  |---- Gir.toml
  |---- pango-sys/
  |       |
  |       |---- Gir.toml
  |       |---- src/
  |              |
  |              |---- lib.rs
  |---- src/
          |
          |---- lib.rs
.git
  |
  |---- ...
.gitmodules
```

Now that we installed gir and prepared our project folder, let's get the .gir files.

[gir]: https://github.com/gtk-rs/gir
