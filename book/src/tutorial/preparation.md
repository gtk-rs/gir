# Preparation
In order to install gir and nicely structure the project, there are a few things to set up.

## Set up the project folder
In order to keep the project folder nicely organized, let's create a folder where we will work in and initialize the repo.
We will create two library crates.
pango will contain the safe wrapper crate and because it is a wrapper for the unsafe bindings, we create the pango-sys crate within the pango crate.
If no Cargo.toml file is present in the sys create, a new one will be generated, so let's be safe and delete the automatically created file before we begin.
The following commands will set up the project folder as described.

```sh
mkdir gir-tutorial
cd gir-tutorial/
git init
cargo new pango --lib
cd pango
cargo new pango-sys --lib
rm pango-sys/Cargo.toml
```
We will also create a file called "Gir.toml" in each of the crates.
```sh
touch Gir.toml
touch pango-sys/Gir.toml
cd ..
```

## Installing gir
Of course we also need to download and install [gir].
```sh
git submodule add https://github.com/gtk-rs/gir
git config -f .gitmodules submodule.gir.update none
git submodule set-branch --branch master -- ./gir
cd gir
cargo install --path .
cd ..
```
By adding it as a submodule, we are able to fetch future updates of the tool and we always exactly know which gir version we used to generate our bindings.
We also change the setting so that the submodule is not automatically checked out, otherwise anyone using your library from git will have the useless submodule checked out.
Run `git submodule update --checkout` if you want to update the submodule.
Then we set the branch of the submodule to master.

If there are any updates to gir in the future, we can install them by opening our project folder `gir-tutorial` and running
```sh
git submodule update --remote
cd gir
cargo install --path .
cd ..
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
