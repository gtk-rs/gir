use std::path::*;
use std::io::{Result, Write};

use env::Env;
use file_saver::*;
use library;
use nameutil::*;
use super::statics;
use super::super::general;

pub fn generate(env: &Env) {
    println!("generating sys for {}", env.config.library_name);

    let path =  PathBuf::from(&env.config.target_path)
        .join(file_name_sys(&env.config.library_name, "lib"));
    println!("Generating file {:?}", path);

    save_to_file(path, &mut |w| generate_funcs(w, env));
}

fn generate_funcs<W: Write>(w: &mut W, env: &Env) -> Result<()>{
    try!(general::start_comments(w, &env.config));

    try!(statics::generate(w));

    let ns_id = library::MAIN_NAMESPACE;
    let ns = env.library.namespace(ns_id);

    try!(writeln!(w, ""));
    try!(writeln!(w, "extern \"C\" {{"));
    try!(generate_classes_funcs(w, env, ns_id, ns));

    //TODO: other functions
    try!(writeln!(w, "\n}}"));

    Ok(())
}
fn generate_classes_funcs<W: Write>(w: &mut W, env: &Env, ns_id: u16, ns:&library::Namespace) -> Result<()> {
    let mut vec: Vec<(library::TypeId, &library::Class)> = Vec::with_capacity(ns.types.len());
    for id in 0..ns.types.len() {
        let tid = library::TypeId { ns_id: ns_id, id: id as u32 };
        if let &library::Type::Class(ref klass) = env.library.type_(tid) {
            vec.push((tid, klass));
        }
    }
    vec.sort_by(|&(_, klass1), &(_, klass2)| klass1.glib_type_name.cmp(&klass2.glib_type_name));

    for (_, ref klass) in vec {
        try!(generate_class_funcs(w, klass));
    }

    Ok(())
}

fn generate_class_funcs<W: Write>(w: &mut W, klass: &library::Class) -> Result<()> {
    try!(writeln!(w, ""));
    try!(writeln!(w, "    //========================================================================="));
    try!(writeln!(w, "    // {}", klass.glib_type_name));
    try!(writeln!(w, "    //========================================================================="));
    try!(writeln!(w, "    pub fn {:<36}() -> GType;", klass.glib_get_type));

    Ok(())
}
