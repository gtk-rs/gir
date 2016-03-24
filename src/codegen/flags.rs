use analysis::namespaces;
use codegen::general::{self, version_condition, version_condition_string};
use config::gobjects::GObject;
use env::Env;
use file_saver;
use library::*;
use nameutil::strip_prefix_uppercase;
use std::cmp;
use std::io::prelude::*;
use std::io::Result;
use std::path::Path;
use traits::*;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let path = root_path.join("flags.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(writeln!(w, "{}", "
use ffi;
use glib::translate::*;
"));

        let configs = env.config.objects.values()
            .filter(|c| { 
                c.status.need_generate() &&
                    c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
            });
        let mut first = true;
        for config in configs {
            if let Type::Bitfield(ref flags) = *env.library.type_(config.type_id.unwrap()) {
                if first {
                    mod_rs.push("\nmod flags;".into());
                    first = false;
                }
                if let Some (cfg) = version_condition_string(env, flags.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!("pub use self::flags::{};", flags.name));
                try!(generate_flags(env, w, mod_rs, flags, config));
            }
        }

        Ok(())
    });
}

fn generate_flags(env: &Env, w: &mut Write, mod_rs: &mut Vec<String>, flags: &Bitfield,
                  config: &GObject) -> Result<()> {
    try!(version_condition(w, env, flags.version, false, 0));
    try!(writeln!(w, "bitflags! {{"));
    try!(writeln!(w, "    pub flags {}: u32 {{", flags.name));
    for member in &flags.members {
        let name = strip_prefix_uppercase(&env.library.namespace(namespaces::MAIN).symbol_prefixes,
            &member.c_identifier);
        let val: i64 = member.value.parse().unwrap();
        let member_config = config.members.matched(&member.name);
        let version = member_config.iter().filter_map(|m| m.version).next();
        try!(version_condition(w, env, version, false, 2));
        try!(writeln!(w, "\t\tconst {} = {},", name, val as u32));
        if let Some(cfg) = version_condition_string(env,
                cmp::max(flags.version, version), false, 0) {
            mod_rs.push(cfg);
        }
        mod_rs.push(format!("pub use self::flags::{};", name));
    }

    try!(writeln!(w, "{}",
"    }
}
"));

    try!(version_condition(w, env, flags.version, false, 0));
    try!(writeln!(w, "#[doc(hidden)]
impl ToGlib for {name} {{
    type GlibType = ffi::{ffi_name};

    fn to_glib(&self) -> ffi::{ffi_name} {{
        ffi::{ffi_name}::from_bits_truncate(self.bits())
    }}
}}
", name = flags.name, ffi_name = flags.c_type));

    try!(version_condition(w, env, flags.version, false, 0));
    try!(writeln!(w, "#[doc(hidden)]
impl FromGlib<ffi::{ffi_name}> for {name} {{
    fn from_glib(value: ffi::{ffi_name}) -> {name} {{
        {name}::from_bits_truncate(value.bits())
    }}
}}
", name = flags.name, ffi_name = flags.c_type));

    Ok(())
}
