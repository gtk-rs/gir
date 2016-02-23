use std::io::{Result, Write};

use analysis;

pub fn generate(w: &mut Write, analysis: &analysis::trampolines::Trampoline,
                in_trait: bool, object_name: &str) -> Result<()> {
    try!(writeln!(w, ""));
    let (bounds, end) = if in_trait {
        ("<T>", "")
    } else {
        ("", " {")
    };

    //TODO: version, cfg_condition
    try!(writeln!(w, "unsafe extern \"C\" fn {}{}(/*TODO: params*/){}", analysis.name, bounds, end));
    if in_trait {
        try!(writeln!(w, "where T: IsA<{}> {{", object_name));
    }
    try!(writeln!(w, "\t//TODO: body"));
    try!(writeln!(w, "}}"));

    Ok(())
}
