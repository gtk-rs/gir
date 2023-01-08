# Configuration

GIR uses two configurations files, one for generating the FFI part of the bindings and the other file for the Rust API.
The configuration files must be named `Gir.toml`

- The FFI configuration allows things such as ignoring objects, overriding the minimum required version for a specific type or renaming the generated crate name.

- The Rust API configuration is a bit more complex as it allows configuring Objects, Enums, Bitfields, Functions, Properties, Signals and a few other things.
