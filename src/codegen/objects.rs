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
        let info = analysis::object::new(env, obj)
            .or_else(|| analysis::object::interface(env, obj));
        let class_analysis = match info {
            Some(info) => info,
            None => {
                warn!("Class or interface {} not found.", obj.name);
                continue;
            }
        };

        let path = root_path.join(file_name(&class_analysis.full_name));
        info!("Generating file {:?}", path);

        save_to_file(path, env.config.make_backup,
            |w| super::object::generate(w, env, &class_analysis));

        let mod_name = module_name(split_namespace_name(&class_analysis.full_name).1);
        super::object::generate_reexports(env, &class_analysis, &mod_name, mod_rs, traits);
    }
}
