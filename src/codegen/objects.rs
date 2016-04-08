use std::path::Path;

use analysis;
use env::Env;
use file_saver::*;
use nameutil::*;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>, traits: &mut Vec<String>) {
    info!("Generate objects");
    for obj in env.config.objects.values() {
        if !obj.status.need_generate() {
            continue;
        }

        info!("Analyzing {:?}", obj.name);
        let info = analysis::object::class(env, obj)
            .or_else(|| analysis::object::interface(env, obj));
        let class_analysis = match info {
            Some(info) => info,
            None => continue,
        };

        let mod_name = obj.module_name.clone().unwrap_or_else(|| {
            module_name(split_namespace_name(&class_analysis.full_name).1)
        });

        let mut path = root_path.join(&mod_name);
        path.set_extension("rs");
        info!("Generating file {:?}", path);

        save_to_file(path, env.config.make_backup,
            |w| super::object::generate(w, env, &class_analysis));

        super::object::generate_reexports(env, &class_analysis, &mod_name, mod_rs, traits);
    }
}
