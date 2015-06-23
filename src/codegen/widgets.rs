use std::path::*;

use chunk::*;
use env::Env;
use file_saver::*;
use gobjects::*;
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

        //TODO: do normal generation
        let v = vec![
            "// Copyright 2013-2015, The Rust-GNOME Project Developers.",
            "// See the COPYRIGHT file at the top-level directory of this distribution.",
            "// Licensed under the MIT license, see the LICENSE file or <http://opensource.org/licenses/MIT>"
        ];
        let _ =  v.into_chunks().into_iter().save_to_file(path);
    }
}
