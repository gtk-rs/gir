# Tutorial
In this tutorial, we go through all the steps needed to generate a safe wrapper for a C library. We start with finding the .gir files needed and end with generating the documentation. Afterwards, we look at a few common errors and how to fix them.

The first paragraphs of each chapter explain the general process. Because this can be a bit abstract sometimes, it is followed by an example. The example continues through all the chapters. If you follow along until the end, you will have generated a safe wrapper including the documentation. In case you are stuck at any point or there are other errors and you can't figure out what's going on, don't hesitate to reach us so we can give you a hand!

As an example, we'll generate the `sourceview` library bindings. So first, let's see where we can get the .gir files for it!

[gir]: https://github.com/gtk-rs/gir
