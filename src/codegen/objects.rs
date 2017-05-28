use std::path::Path;

use env::Env;
use file_saver::*;
use nameutil::*;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>,
                traits: &mut Vec<String>) {
    info!("Generate objects");
    for class_analysis in env.analysis.objects.values() {
        let obj = &env.config.objects[&class_analysis.full_name];
        if !obj.status.need_generate() {
            continue;
        }

        let mod_name = obj.module_name.clone().unwrap_or_else(|| {
            module_name(split_namespace_name(&class_analysis.full_name).1)
        });

        let mut path = root_path.join(&mod_name);
        path.set_extension("rs");
        info!("Generating file {:?}", path);

        save_to_file(path, env.config.make_backup,
            |ref mut w| super::object::generate(w, env, class_analysis));

        super::object::generate_reexports(env, class_analysis, &mod_name, mod_rs, traits);
    }
}
