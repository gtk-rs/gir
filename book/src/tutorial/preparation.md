# Preparation
In order to install gir and nicely structure the project, there are a few things to set up.

## Set up repo
TODO

Let's create folders for the unsafe bindings and the safe wrapper:
```console
> cargo new sourceview-sys --lib
> cargo new sourceview --lib
```

```console
> rm sourceview-sys/Cargo.* # we remove Cargo files
```

## Installing gir
First, you'll need to download and install [gir]:

```console
> git clone --depth 1 https://github.com/gtk-rs/gir
> cd gir
> cargo install --path . # so we can use gir binary directly
```

## Summary
TODO
You should now have a folder looking like this:
```text
sourceview-sys/
  |
  |---- Cargo.toml
  |---- Gir.toml
  |---- src/
  |       |
  |       |---- lib.rs
sourceview/
  |
  |---- Cargo.toml
  |
  |---- src/
          |
          |---- lib.rs
```

[gir]: https://github.com/gtk-rs/gir
