use std::path::Path;
use std::io::{Result, Write};

use env::Env;
use file_saver::*;
use nameutil::*;

use analysis;
use library::*;
use traits::*;
use codegen::subclass::class_impl;

pub struct SubclassInfo{

}

impl SubclassInfo{
    pub fn new(env: &Env, class: &Class, analysis: &analysis::object::Info) -> Self{
        Self{}
    }
}



pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>, traits: &mut Vec<String>) {
    info!("Generate class traits");

    let ns = env.library.namespace(MAIN_NAMESPACE);
    let classes = prepare(ns);


    for object_analysis in env.analysis.objects.values() {
        let obj = &env.config.objects[&object_analysis.full_name];
        if !obj.status.need_generate() {
            continue;
        }

        let mod_name = obj.module_name.clone().unwrap_or_else(|| {
            module_name(split_namespace_name(&object_analysis.full_name).1)
        });

        let mut path = root_path.join(&mod_name);
        path.set_extension("rs");
        info!("Generating file {:?}", mod_name);

        let class_idx = classes.binary_search_by(|c: &&Class|  {
            (*c).c_type.cmp(&object_analysis.c_type)
        });

        match class_idx{
            Ok(idx) => {
                let class = classes[class_idx.unwrap()];

                save_to_file(path, env.config.make_backup, |ref mut w| {
                    class_impl::generate(w, env, class, object_analysis)
                });
            },
            Err(_) => {
                warn!("No class definition found for {:?}. Skipping...", object_analysis.full_name)
            }
        }

        // super::object::generate_reexports(env, class_analysis, &mod_name, mod_rs, traits);
    }
}


// TODO: remove duplicate with sys generator
fn prepare<T: Ord>(ns: &Namespace) -> Vec<&T>
where
    Type: MaybeRef<T>,
{
    let mut vec: Vec<&T> = Vec::with_capacity(ns.types.len());
    for typ in ns.types.iter().filter_map(|t| t.as_ref()) {
        if let Some(x) = typ.maybe_ref() {
            vec.push(x);
        }
    }
    vec.sort();
    vec
}
