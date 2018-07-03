use analysis::imports::Imports;
use analysis::namespaces;
use case::CaseExt;
use codegen::general::{self, cfg_deprecated, derives, version_condition, version_condition_string};
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
    let configs: Vec<&GObject> = env.config
        .objects
        .values()
        .filter(|c| {
            c.status.need_generate() && c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
        })
        .collect();
    let mut has_get_quark = false;
    let mut has_any = false;
    let mut has_get_type = false;
    for config in &configs {
        if let Type::Enumeration(ref enum_) = *env.library.type_(config.type_id.unwrap()) {
            has_any = true;
            if get_error_quark_name(enum_).is_some() {
                has_get_quark = true;
            }
            if enum_.glib_get_type.is_some() {
                has_get_type = true;
            }

            if has_get_type && has_get_quark {
                break;
            }
        }
    }

    let mut imports = Imports::new(&env.library);
    imports.add("ffi", None);
    if has_get_quark {
        imports.add("glib_ffi", None);
        imports.add("glib::error::ErrorDomain", None);
    }
    if has_get_type {
        imports.add("glib::Type", None);
        imports.add("glib::StaticType", None);
        imports.add("glib::value::Value", None);
        imports.add("glib::value::SetValue", None);
        imports.add("glib::value::FromValue", None);
        imports.add("glib::value::FromValueOptional", None);
        imports.add("gobject_ffi", None);
    }
    imports.add("glib::translate::*", None);

    if !has_any {
        return
    }

    let path = root_path.join("enums.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(general::uses(w, env, &imports));
        try!(writeln!(w));

        if has_any {
            mod_rs.push("\nmod enums;".into());
        }
        for config in &configs {
            if let Type::Enumeration(ref enum_) = *env.library.type_(config.type_id.unwrap()) {
                if let Some(cfg) = version_condition_string(env, enum_.version, false, 0) {
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
        value: String,
        version: Option<Version>,
        deprecated_version: Option<Version>,
    }

    let mut members: Vec<Member> = Vec::new();
    let mut vals: HashSet<String> = HashSet::new();

    for member in &enum_.members {
        let member_config = config.members.matched(&member.name);
        let is_alias = member_config.iter().any(|m| m.alias);
        let ignore = member_config.iter().any(|m| m.ignore);
        if is_alias || ignore || vals.contains(&member.value) {
            continue;
        }
        vals.insert(member.value.clone());
        let deprecated_version = member_config.iter().filter_map(|m| m.deprecated_version).next();
        let version = member_config.iter().filter_map(|m| m.version).next();
        members.push(Member {
            name: member.name.to_camel(),
            c_name: member.c_identifier.clone(),
            value: member.value.clone(),
            version,
            deprecated_version,
        });
    }

    try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
    try!(version_condition(w, env, enum_.version, false, 0));
    try!(writeln!(
        w,
        "#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]"
    ));
    if config.must_use {
        try!(writeln!(
            w,
            "#[must_use]"
        ));
    }
    try!(derives(w, &config.derives, 0));
    try!(writeln!(w, "pub enum {} {{", enum_.name));
    for member in &members {
        try!(cfg_deprecated(w, env, member.deprecated_version, false, 1));
        try!(version_condition(w, env, member.version, false, 1));
        try!(writeln!(w, "\t{},", member.name));
    }
    try!(writeln!(
        w,
        "{}",
        "    #[doc(hidden)]
    __Unknown(i32),
}
"
    ));

    try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
    try!(version_condition(w, env, enum_.version, false, 0));
    try!(writeln!(
        w,
        "#[doc(hidden)]
impl ToGlib for {name} {{
    type GlibType = ffi::{ffi_name};

    fn to_glib(&self) -> ffi::{ffi_name} {{
        match *self {{",
        name = enum_.name,
        ffi_name = enum_.c_type
    ));
    for member in &members {
        try!(cfg_deprecated(w, env, member.deprecated_version, false, 3));
        try!(version_condition(w, env, member.version, false, 3));
        try!(writeln!(
            w,
            "\t\t\t{}::{} => ffi::{},",
            enum_.name,
            member.name,
            member.c_name
        ));
    }
    try!(writeln!(
        w,
        "\t\t\t{}::__Unknown(value) => value",
        enum_.name
    ));
    try!(writeln!(
        w,
        "{}",
        "        }
    }
}
"
    ));

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
    try!(version_condition(w, env, enum_.version, false, 0));
    try!(writeln!(
        w,
        "#[doc(hidden)]
impl FromGlib<ffi::{ffi_name}> for {name} {{
    fn from_glib(value: ffi::{ffi_name}) -> Self {{
        {assert}match value {{",
        name = enum_.name,
        ffi_name = enum_.c_type,
        assert = assert
    ));
    for member in &members {
        try!(cfg_deprecated(w, env, member.deprecated_version, false, 3));
        try!(version_condition(w, env, member.version, false, 3));
        try!(writeln!(
            w,
            "\t\t\t{} => {}::{},",
            member.value,
            enum_.name,
            member.name
        ));
    }
    try!(writeln!(
        w,
        "\t\t\tvalue => {}::__Unknown(value),",
        enum_.name
    ));
    try!(writeln!(
        w,
        "{}",
        "        }
    }
}
"
    ));
    if let Some(ref get_quark) = get_error_quark_name(enum_) {
        let get_quark = get_quark.replace("-", "_");
        let has_failed_member = members.iter().any(|m| m.name == "Failed");

        try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
        try!(version_condition(w, env, enum_.version, false, 0));
        try!(writeln!(
            w,
            "impl ErrorDomain for {name} {{
    fn domain() -> glib_ffi::GQuark {{
        {assert}unsafe {{ ffi::{get_quark}() }}
    }}

    fn code(self) -> i32 {{
        self.to_glib()
    }}

    fn from(code: i32) -> Option<Self> {{
        {assert}match code {{",
            name = enum_.name,
            get_quark = get_quark,
            assert = assert
        ));

        for member in &members {
            try!(cfg_deprecated(w, env, member.deprecated_version, false, 3));
            try!(version_condition(w, env, member.version, false, 3));
            try!(writeln!(
                w,
                "\t\t\t{} => Some({}::{}),",
                member.value,
                enum_.name,
                member.name
            ));
        }
        if has_failed_member {
            try!(writeln!(w, "\t\t\t_ => Some({}::Failed),", enum_.name));
        } else {
            try!(writeln!(
                w,
                "\t\t\tvalue => Some({}::__Unknown(value)),",
                enum_.name
            ));
        }

        try!(writeln!(
            w,
            "{}",
            "        }
    }
}
"
        ));
    }

    if let Some(ref get_type) = enum_.glib_get_type {
        try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
        try!(version_condition(w, env, enum_.version, false, 0));
        try!(writeln!(
            w,
            "impl StaticType for {name} {{
    fn static_type() -> Type {{
        unsafe {{ from_glib(ffi::{get_type}()) }}
    }}
}}",
            name = enum_.name,
            get_type = get_type
        ));
        try!(writeln!(w));

        try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
        try!(version_condition(w, env, enum_.version, false, 0));
        try!(writeln!(
            w,
            "impl<'a> FromValueOptional<'a> for {name} {{
    unsafe fn from_value_optional(value: &Value) -> Option<Self> {{
        Some(FromValue::from_value(value))
    }}
}}",
            name = enum_.name,
        ));
        try!(writeln!(w));

        try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
        try!(version_condition(w, env, enum_.version, false, 0));
        try!(writeln!(
            w,
            "impl<'a> FromValue<'a> for {name} {{
    unsafe fn from_value(value: &Value) -> Self {{
        from_glib(gobject_ffi::g_value_get_enum(value.to_glib_none().0))
    }}
}}",
            name = enum_.name,
        ));
        try!(writeln!(w));

        try!(cfg_deprecated(w, env, enum_.deprecated_version, false, 0));
        try!(version_condition(w, env, enum_.version, false, 0));
        try!(writeln!(
            w,
            "impl SetValue for {name} {{
    unsafe fn set_value(value: &mut Value, this: &Self) {{
        gobject_ffi::g_value_set_enum(value.to_glib_none_mut().0, this.to_glib())
    }}
}}",
            name = enum_.name,
        ));
        try!(writeln!(w));
    }

    Ok(())
}

fn get_error_quark_name(enum_: &Enumeration) -> Option<String> {
    enum_
        .functions
        .iter()
        .find(|f| f.name == "quark")
        .and_then(|f| f.c_identifier.clone())
        .or_else(|| enum_.error_domain.clone())
}
