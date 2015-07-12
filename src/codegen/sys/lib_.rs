use std::path::*;
use std::io::{Result, Write};
use case::CaseExt;

use env::Env;
use file_saver::*;
use library::{self, MaybeRef};
use nameutil::*;
use super::functions;
use super::statics;
use super::super::general::{self, tabs};

pub fn generate(env: &Env) {
    println!("generating sys for {}", env.config.library_name);

    let path =  PathBuf::from(&env.config.target_path)
        .join(file_name_sys(&env.config.library_name, "lib"));
    println!("Generating file {:?}", path);

    save_to_file(path, &mut |w| generate_lib(w, env));
}

fn generate_lib<W: Write>(w: &mut W, env: &Env) -> Result<()>{
    try!(general::start_comments(w, &env.config));
    try!(statics::begin(w));

    let ns = env.library.namespace(library::MAIN_NAMESPACE);
    let classes = prepare(ns);
    let interfaces = prepare(ns);

    try!(generate_enums(w, &ns.name, &prepare(ns)));
    try!(generate_bitfields(w, &ns.name, &prepare(ns)));
    try!(functions::generate_callbacks(w, env, &prepare(ns)));
    try!(generate_classes_structs(w, &classes));
    try!(generate_interfaces_structs(w, &interfaces));

    try!(statics::before_func(w));

    try!(writeln!(w, ""));
    try!(writeln!(w, "extern \"C\" {{"));
    try!(functions::generate_classes_funcs(w, env, &classes));
    try!(functions::generate_interfaces_funcs(w, env, &interfaces));

    //TODO: other functions
    try!(writeln!(w, "\n}}"));

    Ok(())
}

fn prepare<T: Ord>(ns: &library::Namespace) -> Vec<&T>
where library::Type: MaybeRef<T> {
    let mut vec: Vec<&T> = Vec::with_capacity(ns.types.len());
    for typ in ns.types.iter().filter_map(|t| t.as_ref()) {
        if let Some(ref x) = typ.maybe_ref() {
            vec.push(x);
        }
    }
    vec.sort();
    vec
}

fn generate_bitfields<W: Write>(w: &mut W, ns_name: &str, items: &[&library::Bitfield])
        -> Result<()> {
    try!(writeln!(w, ""));
    for item in items {
        try!(writeln!(w, "bitflags! {{\n{}#[repr(C)]\n{0}flags {}: i32 {{", tabs(1), item.name));
        for member in &item.members {
            try!(writeln!(w, "{}const {} = {},",
                          tabs(2), strip_prefix(ns_name, &member.c_identifier), member.value));
        }
        try!(writeln!(w, "{}}}\n}}", tabs(1)));
        try!(writeln!(w, "pub type {} = {};", item.glib_type_name, item.name));
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_enums<W: Write>(w: &mut W, ns_name: &str, items: &[&library::Enumeration])
        -> Result<()> {
    try!(writeln!(w, ""));
    for item in items {
        try!(writeln!(w, "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n#[repr(C)]"));
        try!(writeln!(w, "pub enum {} {{", item.name));
        for member in &item.members {
            try!(writeln!(w, "{}{} = {},",
                          tabs(1), member.name.to_camel(), member.value));
        }
        try!(writeln!(w, "}}"));
        for member in &item.members {
            try!(writeln!(w, "pub const {}: {} = {1}::{};",
                          strip_prefix(ns_name, &member.c_identifier),
                          item.name, member.name.to_camel()));
        }
        try!(writeln!(w, "pub type {} = {};", item.glib_type_name, item.name));
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_classes_structs<W: Write>(w: &mut W, classes: &[&library::Class]) -> Result<()> {
    try!(writeln!(w, ""));
    for klass in classes {
        try!(writeln!(w, "#[repr(C)]\npub struct {};", klass.glib_type_name));
    }

    Ok(())
}

fn generate_interfaces_structs<W: Write>(w: &mut W, interfaces: &[&library::Interface]) -> Result<()> {
    try!(writeln!(w, ""));
    for interface in interfaces {
        try!(writeln!(w, "#[repr(C)]\npub struct {};", interface.glib_type_name));
    }

    Ok(())
}
