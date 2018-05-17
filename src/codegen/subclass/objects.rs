use std::path::Path;
use std::io::{Result, Write};

use env::Env;
use file_saver::*;
use nameutil::*;

use library::*;
use traits::*;


pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>, traits: &mut Vec<String>) {
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
        info!("Generating file {:?}", mod_name);


        let ns = env.library.namespace(MAIN_NAMESPACE);
        let classes = prepare(ns);

        save_to_file(path, env.config.make_backup, |ref mut w| {
            super::object::generate(w, env, class_analysis);
            generate_classes_traits(w, env, &classes)

        });

        // super::object::generate_reexports(env, class_analysis, &mod_name, mod_rs, traits);
    }
}



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


fn generate_classes_traits(w: &mut Write, env: &Env, classes: &[&Class]) -> Result<()> {
    if !classes.is_empty() {
        try!(writeln!(w, "// Classes"));
    }
    for class in classes {
        let full_name = format!("{}.{}", env.namespaces.main().name, class.name);
        info!("{:?}", full_name);
        // if !env.type_status_sys(&full_name).need_generate() {
        //     continue;
        // }
        // let fields = fields::from_class(env, class);
        // try!(generate_from_fields(w, &fields));
    }
    Ok(())
}
