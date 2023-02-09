# Crate name overrides

`gir` uses simple rule to convert a namespace to a crate name and it sometimes goes wrong.
For example, "WebKit2WebExtension" namespace will be converted to "web_kit2_web_extension", which looks bad.

To fix it, the `crate_name_overrides` option can be used.

It also replaces FFI crates' name.

```toml
[crate_name_overrides]
"web_kit2_web_extension" = "webkit2_webextension"
```
