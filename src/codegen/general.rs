use std::io::{Result, Write};

use analysis::general::StatusedTypeId;

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

pub fn impl_parents<W: Write>(w: &mut W, type_name: &str, parents: &Vec<StatusedTypeId>) -> Result<()>{
    try!(writeln!(w, ""));
    for stid in parents {
        //TODO: don't generate for parents without traits
        try!(writeln!(w, "unsafe impl Upcast<{}> for {} {{ }}", stid.name, type_name));
    }

    Ok(())
}

pub fn impl_static_type<W: Write>(w: &mut W, type_name: &str, glib_func_name: &str) -> Result<()>{
    try!(writeln!(w, ""));
    try!(writeln!(w, "impl types::StaticType for {} {{", type_name));
    try!(writeln!(w, "    #[inline]"));
    try!(writeln!(w, "    fn static_type() -> types::Type {{"));
    try!(writeln!(w, "        unsafe {{ from_glib(ffi::{}()) }}", glib_func_name));
    try!(writeln!(w, "    }}"));
    try!(writeln!(w, "}}"));

    Ok(())
}
