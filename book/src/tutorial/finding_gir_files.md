# Where can I find those .gir files?
There are multiple ways you can get the needed `.gir` files.
The `*.gir` you need corresponds to the library you want to generate bindings for.
If you have the library installed, you can search for the `.gir` file under `/usr/share/gir-1.0/`.
Having the library installed is a good idea so that you can test the generated code.
Otherwise you should be able to get it from the package that installs the library.
Ubuntu for example allows you to [search](https://packages.ubuntu.com/) for packages and download them via their website.
You can copy the `.gir` file of your library to the root of your project folder.
You don't have to store all `.gir` files in the same folder.
You can add multiple paths by changing the `girs_directories` field in the Gir.toml files.
More on this in the next chapters.

Have a look at the .gir file of your library.
At the beginning of the file, you probably see something similar to `<include name="GObject" version="2.0"/>`.
"GObject" in this case would be a dependency and you will have to find the .gir file for your dependencies as well.
In most cases it will be enough to follow the next two steps of the tutorial to get all needed files.

## GTK dependencies
If your library depends on GTK libraries, the recommended way to get the `.gir` files for them is to add the [gir-files repo](https://github.com/gtk-rs/gir-files) as a submodule as well.
It's the recommended way, because some of the `.gir` files included in the libraries are invalid (missing or invalid annotations for example).
These errors are already fixed in the gir files from the repo.
Otherwise you could use the above-mentioned methods to find the files and run the [script](https://github.com/gtk-rs/gir-files/blob/master/fix.sh) to fix the `.gir` files available in the gir-files repository (and only them!).
You can run it like this (at the same level of the `.gir` files you want to patch):

```sh
sh fix.sh
```

## GStreamer dependencies
For GStreamer related dependencies, follow the above-mentioned steps but add this [repo](https://gitlab.freedesktop.org/gstreamer/gir-files-rs) instead.

## Other dependencies
If you have other dependencies, you have to find the files yourself.
They can often be found in the repo containing the source of your dependencies or if you have them installed, you might find them under `/usr/share/gir-1.0/` again.

## Example
We want to generate the wrapper for pango.
It is related to GTK, so in order to get its .gir files, we use the recommended way.
While being in the project folder `git-tutorial`, we add the [gir-files repo](https://github.com/gtk-rs/gir-files) as a submodule and set the branch of the submodule to master.

```sh
git submodule add https://github.com/gtk-rs/gir-files
git config -f .gitmodules submodule.gir-files.update none
git submodule set-branch --branch master -- ./gir-files
```
We also change the setting so that the submodule is not automatically checked out, otherwise anyone using your library from git will have the useless submodule checked out.
Run `git submodule update --checkout` if you want to update the submodule.
If you look into `gir-files`, you'll see a file named `Pango-1.0.gir`.
That's the one for pango.
Because we already added the gir-files repo, we also have all the other .gir files of the dependencies that we need.
Now we can create the unsafe bindings.