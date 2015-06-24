use std::io::{Result, Write};
use std::path::PathBuf;

use env::Env;
use file_saver::*;
use gobjects::*;
//use library::*;
use nameutil::*;

pub fn generate(env: &Env) {
    let root_path = PathBuf::from(&env.config.target_path).join("src/widgets");
    let objs = &env.config.objects;
    for obj in objs.values() {
        if obj.status != GStatus::Generate || obj.gtype != GType::Widget {
            continue;
        }
        let path = root_path.join(file_name(&obj.name));
        println!("Generating file {:?}", path);

        save_to_file(path, &mut |w| inner(w, env, obj));
    }
}

fn inner<W: Write>(w: &mut W, env: &Env, obj: &GObject) -> Result<()>{
    //TODO: do normal generation
    let v = vec![
        "// Copyright 2013-2015, The Rust-GNOME Project Developers.",
        "// See the COPYRIGHT file at the top-level directory of this distribution.",
        "// Licensed under the MIT license, see the LICENSE file or <http://opensource.org/licenses/MIT>"
    ];
    for s in v {
        try!(writeln!(w, "{}", s));
    }

    println!("{:?}", obj);
    let class_id = env.library.find_type(0, &obj.name)
        .unwrap_or_else(|| panic!("Class {} not found.", obj.name));
    let class_info = env.library.type_(class_id).as_class()
        .unwrap_or_else(|| panic!("{} is not a class.", obj.name));
    println!("Class name: {:?}", class_info.glib_name);

    Ok(())
}
