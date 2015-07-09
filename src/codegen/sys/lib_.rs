use std::path::*;
use std::io::{Result, Write};

use env::Env;
use file_saver::*;
use library;
use nameutil::*;
use super::functions;
use super::statics;
use super::super::general;

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

    let ns_id = library::MAIN_NAMESPACE;
    let classes = prepare_classes(env, ns_id);
    let interfaces = prepare_interfaces(env, ns_id);

    try!(generate_classes_structs(w, &classes));
    try!(generate_interfaces_structs(w, &interfaces));

    try!(statics::before_func(w));

    try!(writeln!(w, ""));
    try!(writeln!(w, "extern \"C\" {{"));
    try!(functions::generate_classes_funcs(w, env, &classes));

    //TODO: other functions
    try!(writeln!(w, "\n}}"));

    Ok(())
}

fn prepare_classes(env: &Env, ns_id: u16) -> Vec<&library::Class> {
    let ns = env.library.namespace(ns_id);
    let mut vec: Vec<&library::Class> = Vec::with_capacity(ns.types.len());
    for id in 0..ns.types.len() {
        let tid = library::TypeId { ns_id: ns_id, id: id as u32 };
        if let &library::Type::Class(ref klass) = env.library.type_(tid) {
            vec.push(klass);
        }
    }
    vec.sort_by(|ref klass1, ref klass2| klass1.glib_type_name.cmp(&klass2.glib_type_name));
    vec
}

fn generate_classes_structs<W: Write>(w: &mut W, classes: &Vec<&library::Class>) -> Result<()> {
    try!(writeln!(w, ""));
    for klass in classes {
        try!(writeln!(w, "#[repr(C)]\npub struct {};", klass.glib_type_name));
    }

    Ok(())
}

fn prepare_interfaces(env: &Env, ns_id: u16) -> Vec<&library::Interface> {
    let ns = env.library.namespace(ns_id);
    let mut vec: Vec<&library::Interface> = Vec::with_capacity(ns.types.len());
    for id in 0..ns.types.len() {
        let tid = library::TypeId { ns_id: ns_id, id: id as u32 };
        if let &library::Type::Interface(ref interface) = env.library.type_(tid) {
            vec.push(interface);
        }
    }
    vec.sort_by(|ref interface1, ref interface2| interface1.glib_type_name.cmp(&interface2.glib_type_name));
    vec
}

fn generate_interfaces_structs<W: Write>(w: &mut W, interfaces: &Vec<&library::Interface>) -> Result<()> {
    try!(writeln!(w, ""));
    for interface in interfaces {
        try!(writeln!(w, "#[repr(C)]\npub struct {};", interface.glib_type_name));
    }

    Ok(())
}
