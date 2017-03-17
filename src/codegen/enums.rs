use analysis::namespaces;
use case::CaseExt;
use codegen::general::{self, version_condition, version_condition_string};
use config::gobjects::GObject;
use env::Env;
use file_saver;
use library::*;
use std::collections::HashSet;
use std::io::prelude::*;
use std::io::Result;
use std::path::Path;
use traits::*;
use version::Version;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let configs: Vec<&GObject> = env.config.objects.values()
        .filter(|c| {
            c.status.need_generate() &&
                c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
        })
        .collect();
    let mut has_get_quark = false;
    let mut has_any = false;
    for config in &configs {
        if let Type::Enumeration(ref enum_) = *env.library.type_(config.type_id.unwrap()) {
            has_any = true;
            if get_error_quark_name(enum_).is_some(){
                has_get_quark = true;
                break;
            }
        }
    }

    let path = root_path.join("enums.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(writeln!(w, ""));
        try!(writeln!(w, "use ffi;"));
        if env.namespaces.glib_ns_id == namespaces::MAIN {
            if has_get_quark {
                try!(writeln!(w, "use ffi as glib_ffi;"));
                try!(writeln!(w, "use error::ErrorDomain;"));
            }
            try!(writeln!(w, "use translate::*;"));
        } else {
            if has_get_quark {
                try!(writeln!(w, "use glib_ffi;"));
                try!(writeln!(w, "use glib::error::ErrorDomain;"));
            }
            try!(writeln!(w, "use glib::translate::*;"));
        }
        try!(writeln!(w, ""));

        if has_any {
            mod_rs.push("\nmod enums;".into());
        }
        for config in &configs {
            if let Type::Enumeration(ref enum_) = *env.library.type_(config.type_id.unwrap()) {
                if let Some (cfg) = version_condition_string(env, enum_.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!("pub use self::enums::{};", enum_.name));
                try!(generate_enum(env, w, enum_, config));
            }
        }

        Ok(())
    });
}

fn generate_enum(env: &Env, w: &mut Write, enum_: &Enumeration, config: &GObject) -> Result<()> {
    struct Member {
        name: String,
        c_name: String,
        version: Option<Version>,
    }

    let mut members: Vec<Member> = Vec::new();
    let mut vals: HashSet<String> = HashSet::new();

    for member in &enum_.members {
        let member_config = config.members.matched(&member.name);
        let is_alias = member_config.iter().any(|m| m.alias);
        if is_alias || vals.contains(&member.value) {
            continue
        }
        vals.insert(member.value.clone());
        let version = member_config.iter().filter_map(|m| m.version).next();
        members.push(Member {
            name: member.name.to_camel(),
            c_name: member.c_identifier.clone(),
            version: version,
        });
    }

    try!(version_condition(w, env, enum_.version, false, 0));
    try!(writeln!(w, "#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]"));
    try!(writeln!(w, "pub enum {} {{", enum_.name));
    for member in &members {
        try!(version_condition(w, env, member.version, false, 1));
        try!(writeln!(w, "\t{},", member.name));
    }
    try!(writeln!(w, "{}", "    #[doc(hidden)]
    __Nonexhaustive(()),
}
"));

    try!(version_condition(w, env, enum_.version, false, 0));
    try!(writeln!(w, "#[doc(hidden)]
impl ToGlib for {name} {{
    type GlibType = ffi::{ffi_name};

    fn to_glib(&self) -> ffi::{ffi_name} {{
        match *self {{", name = enum_.name, ffi_name = enum_.c_type));
    for member in &members {
        try!(version_condition(w, env, member.version, false, 3));
        try!(writeln!(w, "\t\t\t{}::{} => ffi::{},", enum_.name, member.name, member.c_name));
    }
    try!(writeln!(w, "\t\t\t{}::__Nonexhaustive(_) => panic!(),", enum_.name));
    try!(writeln!(w, "{}",
"        }
    }
}
"));

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    try!(version_condition(w, env, enum_.version, false, 0));
    try!(writeln!(w, "#[doc(hidden)]
impl FromGlib<ffi::{ffi_name}> for {name} {{
    fn from_glib(value: ffi::{ffi_name}) -> Self {{
        {assert}match value {{", name = enum_.name, ffi_name = enum_.c_type, assert = assert));
    for member in &members {
        try!(version_condition(w, env, member.version, false, 3));
        try!(writeln!(w, "\t\t\tffi::{} => {}::{},", member.c_name, enum_.name, member.name));
    }
    if members.len() == 1 {
        try!(writeln!(w, "\t\t\t_ => {}::__Nonexhaustive(()),", enum_.name));
    }
    try!(writeln!(w, "{}",
"        }
    }
}
"));
    if let Some(ref get_quark) = get_error_quark_name(enum_) {
        let get_quark = get_quark.replace("-", "_");
        let has_failed_member = members.iter().any(|m| m.name == "Failed");

        try!(version_condition(w, env, enum_.version, false, 0));
        try!(writeln!(w, "impl ErrorDomain for {name} {{
    fn domain() -> glib_ffi::GQuark {{
        {assert}unsafe {{ ffi::{get_quark}() }}
    }}

    fn code(self) -> i32 {{
        self.to_glib() as i32
    }}

    fn from(code: i32) -> Option<Self> {{
        {assert}match code {{", name = enum_.name, get_quark = get_quark, assert = assert));

        for member in &members {
            try!(version_condition(w, env, member.version, false, 3));
            try!(writeln!(w, "\t\t\tx if x == ffi::{} as i32 => Some({}::{}),", member.c_name,
                enum_.name, member.name));
        }
        if has_failed_member {
            try!(writeln!(w, "\t\t\t_ => Some({}::Failed),", enum_.name));
        } else {
            try!(writeln!(w, "\t\t\t_ => Some({}::__Nonexhaustive(())),", enum_.name));
        }

        try!(writeln!(w, "{}",
"        }
    }
}
"));
    }

    Ok(())
}

fn get_error_quark_name(enum_: &Enumeration) -> Option<String> {
    enum_.functions.iter()
        .find(|f| f.name == "quark")
        .and_then(|f| f.c_identifier.clone())
        .or_else(|| enum_.error_domain.clone())
}
