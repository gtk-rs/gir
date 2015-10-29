use std::cmp::Ord;
use std::io::{Result, Write};

use analysis::foreign::{Def, DefId, DefKind};
use analysis::namespaces;
use env::Env;
use file_saver;
use nameutil;

pub fn generate(env: &Env) {
    println!("Generating ABI tests for {}", env.config.library_name);

    let mut def_ids: Vec<DefId> = env.foreign.defs.ids_by_ns(namespaces::MAIN)
        .filter(|&def_id| {
            match env.foreign.defs[def_id] {
                Def { public: true, kind: DefKind::Bitfield, .. } => true,
                Def { public: true, kind: DefKind::Enumeration, .. } => true,
                Def { public: true, kind: DefKind::Record { ref fields, fake, .. }, .. }
                    if !fields.is_empty() && !fake => true,
                _ => false,
            }
        })
        .collect();
    def_ids.sort_by(|&a, &b| env.foreign.defs[a].name.cmp(&env.foreign.defs[b].name));

    let mut path = env.config.target_path.join(nameutil::file_name_sys("abi_tests"));

    println!("Generating file {:?}", path);
    file_saver::save_to_file(&path, env.config.make_backup,
        |w| generate_rust_side(w, env, &def_ids));

    path.set_extension("c");
    println!("Generating file {:?}", path);
    file_saver::save_to_file(&path, env.config.make_backup,
        |w| generate_c_side(w, env, &def_ids));
}

fn generate_rust_side(w: &mut Write, env: &Env, def_ids: &[DefId]) -> Result<()> {
    try!(writeln!(w, "{}",
r#"#![allow(non_snake_case)]

use std::mem::{align_of, size_of};
use libc::size_t;
use super::*;
"#));

    for &def_id in def_ids {
        try!(writeln!(w, "\
#[test]
fn {name}_alignment() {{
    extern {{ fn {name}_alignment() -> size_t; }}
    unsafe {{ assert_eq!(align_of::<{name}>(), {name}_alignment() as usize); }}
}}

#[test]
fn {name}_size() {{
    extern {{ fn {name}_size() -> size_t; }}
    unsafe {{ assert_eq!(size_of::<{name}>(), {name}_size() as usize); }}
}}
", name = env.foreign.defs[def_id].name));
    }

    Ok(())
}

fn generate_c_side(w: &mut Write, env: &Env, def_ids: &[DefId]) -> Result<()> {
    try!(writeln!(w, "{}",
r#"#include <stdalign.h>
#include <glib.h>
"#));

    for &def_id in def_ids {
        try!(writeln!(w, "\
size_t {name}_alignment(void) {{
	return alignof({name});
}}

size_t {name}_size(void) {{
	return sizeof({name});
}}
", name = env.foreign.defs[def_id].name));
    }

    Ok(())
}
