use env::Env;
use library::Type;

pub mod bounds;
pub mod c_type;
pub mod class_hierarchy;
pub mod conversion_type;
pub mod ffi_type;
pub mod functions;
pub mod general;
pub mod imports;
pub mod info_base;
pub mod namespaces;
pub mod object;
pub mod out_parameters;
pub mod parameter;
pub mod record;
pub mod record_type;
pub mod ref_mode;
pub mod return_value;
pub mod rust_type;
pub mod safety_assertion_mode;
pub mod signals;
pub mod special_functions;
pub mod supertypes;
pub mod symbols;
pub mod trampolines;

#[derive(Default)]
pub struct Analysis {
    pub objects: Vec<object::Info>,
    pub records: Vec<record::Info>,
}

pub fn run(env: &mut Env) {
    for obj in env.config.objects.values() {
        if obj.status.ignored() {
            continue;
        }
        let tid = match env.library.find_type(0, &obj.name) {
            Some(x) => x,
            None => continue,
        };
        match *env.library.type_(tid) {
            Type::Class(_) => {
                if let Some(info) = object::class(env, obj) {
                    env.analysis.objects.push(info);
                }
            }
            Type::Interface(_) => {
                if let Some(info) = object::interface(env, obj) {
                    env.analysis.objects.push(info);
                }
            }
            Type::Record(_) => {
                if let Some(info) = record::new(env, obj) {
                    env.analysis.records.push(info);
                }
            }
            _ => {}
        }
    }
}
