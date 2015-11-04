use std::path::Path;

use analysis;
use env::Env;
use file_saver::*;
use nameutil::*;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    info!("Generate records");
    for obj in env.config.objects.values() {
        if !obj.status.need_generate() {
            continue;
        }

        info!("Analyzing {:?}", obj.name);
        let record_analysis = match analysis::record::new(env, obj) {
            Some(info) => info,
            None => continue,
        };

        let path = root_path.join(file_name(&record_analysis.full_name));
        info!("Generating file {:?}", path);

        save_to_file(path, env.config.make_backup,
            |w| super::record::generate(w, env, &record_analysis));

        let mod_name = module_name(split_namespace_name(&record_analysis.full_name).1);
        super::record::generate_reexports(env, &record_analysis, &mod_name, mod_rs);
    }
}
