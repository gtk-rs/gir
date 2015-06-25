use std::io::{Result, Write};

pub fn start_comments<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
        "// Copyright 2013-2015, The Rust-GNOME Project Developers.",
        "// See the COPYRIGHT file at the top-level directory of this distribution.",
        "// Licensed under the MIT license, see the LICENSE file or <http://opensource.org/licenses/MIT>"
    ];
    for s in v {
        try!(writeln!(w, "{}", s));
    }

    Ok(())
}

pub fn objects_child_type<W: Write>(w: &mut W, type_name: &str, glib_name: &str) -> Result<()>{
    try!(writeln!(w, ""));
    try!(writeln!(w, "pub type {} = Object<ffi::{}>;", type_name, glib_name));

    Ok(())
}
