use crate::{
    analysis::{imports::Imports, namespaces},
    codegen::general::{
        self, cfg_deprecated, derives, version_condition, version_condition_string,
    },
    config::gobjects::GObject,
    env::Env,
    file_saver,
    library::*,
    nameutil::enum_member_name,
    traits::*,
    version::Version,
};
use std::{
    collections::HashSet,
    io::{prelude::*, Result},
    path::Path,
};

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let configs: Vec<&GObject> = env
        .config
        .objects
        .values()
        .filter(|c| {
            c.status.need_generate() && c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
        })
        .collect();
    let mut has_get_quark = false;
    let mut has_any = false;
    let mut has_get_type = false;
    let mut generate_display_trait = false;
    for config in &configs {
        if let Type::Enumeration(ref enum_) = *env.library.type_(config.type_id.unwrap()) {
            has_any = true;
            if get_error_quark_name(enum_).is_some() {
                has_get_quark = true;
            }
            if enum_.glib_get_type.is_some() {
                has_get_type = true;
            }
            generate_display_trait |= config.generate_display_trait;

            if has_get_type && has_get_quark {
                break;
            }
        }
    }

    if !has_any {
        return;
    }

    let mut imports = Imports::new(&env.library);
    imports.add(env.main_sys_crate_name());
    if has_get_quark {
        imports.add("glib::Quark");
        imports.add("glib::error::ErrorDomain");
    }
    if has_get_type {
        imports.add("glib::Type");
        imports.add("glib::StaticType");
        imports.add("glib::value::Value");
        imports.add("glib::value::SetValue");
        imports.add("glib::value::FromValue");
        imports.add("glib::value::FromValueOptional");
        imports.add("gobject_sys");
    }
    imports.add("glib::translate::*");

    if generate_display_trait {
        imports.add("std::fmt");
    }

    let path = root_path.join("enums.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::uses(w, env, &imports)?;
        writeln!(w)?;

        mod_rs.push("\nmod enums;".into());
        for config in &configs {
            if let Type::Enumeration(ref enum_) = *env.library.type_(config.type_id.unwrap()) {
                if let Some(cfg) = version_condition_string(env, enum_.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!("pub use self::enums::{};", enum_.name));
                generate_enum(env, w, enum_, config)?;
            }
        }

        Ok(())
    });
}

fn generate_enum(
    env: &Env,
    w: &mut dyn Write,
    enum_: &Enumeration,
    config: &GObject,
) -> Result<()> {
    struct Member {
        name: String,
        c_name: String,
        value: String,
        version: Option<Version>,
        deprecated_version: Option<Version>,
    }

    let mut members: Vec<Member> = Vec::new();
    let mut vals: HashSet<String> = HashSet::new();
    let sys_crate_name = env.main_sys_crate_name();

    for member in &enum_.members {
        let member_config = config.members.matched(&member.name);
        let is_alias = member_config.iter().any(|m| m.alias);
        let ignore = member_config.iter().any(|m| m.ignore);
        if is_alias || ignore || vals.contains(&member.value) {
            continue;
        }
        vals.insert(member.value.clone());
        let deprecated_version = member_config
            .iter()
            .filter_map(|m| m.deprecated_version)
            .next();
        let version = member_config.iter().filter_map(|m| m.version).next();
        members.push(Member {
            name: enum_member_name(&member.name),
            c_name: member.c_identifier.clone(),
            value: member.value.clone(),
            version,
            deprecated_version,
        });
    }

    cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
    version_condition(w, env, enum_.version, false, 0)?;
    if config.must_use {
        writeln!(w, "#[must_use]")?;
    }

    if let Some(ref d) = config.derives {
        derives(w, &d, 1)?;
    } else {
        writeln!(w, "#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]")?;
    }
    writeln!(w, "#[derive(Clone, Copy)]")?;

    writeln!(w, "pub enum {} {{", enum_.name)?;
    for member in &members {
        cfg_deprecated(w, env, member.deprecated_version, false, 1)?;
        version_condition(w, env, member.version, false, 1)?;
        writeln!(w, "\t{},", member.name)?;
    }
    writeln!(
        w,
        "{}",
        "    #[doc(hidden)]
    __Unknown(i32),
}
"
    )?;

    if config.generate_display_trait {
        // Generate Display trait implementation.
        cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
        version_condition(w, env, enum_.version, false, 0)?;
        writeln!(
            w,
            "impl fmt::Display for {0} {{\n\
             \tfn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{\n\
             \t\twrite!(f, \"{0}::{{}}\", match *self {{",
            enum_.name
        )?;
        for member in &members {
            cfg_deprecated(w, env, member.deprecated_version, false, 3)?;
            version_condition(w, env, member.version, false, 3)?;
            writeln!(w, "\t\t\t{0}::{1} => \"{1}\",", enum_.name, member.name)?;
        }
        writeln!(
            w,
            "\t\t\t_ => \"Unknown\",\n\
             \t\t}})\n\
             \t}}\n\
             }}\n"
        )?;
    }

    // Generate ToGlib trait implementation.
    cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
    version_condition(w, env, enum_.version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl ToGlib for {name} {{
    type GlibType = {sys_crate_name}::{ffi_name};

    fn to_glib(&self) -> {sys_crate_name}::{ffi_name} {{
        match *self {{",
        sys_crate_name = sys_crate_name,
        name = enum_.name,
        ffi_name = enum_.c_type
    )?;
    for member in &members {
        cfg_deprecated(w, env, member.deprecated_version, false, 3)?;
        version_condition(w, env, member.version, false, 3)?;
        writeln!(
            w,
            "\t\t\t{}::{} => {}::{},",
            enum_.name, member.name, sys_crate_name, member.c_name
        )?;
    }
    writeln!(w, "\t\t\t{}::__Unknown(value) => value", enum_.name)?;
    writeln!(
        w,
        "{}",
        "        }
    }
}
"
    )?;

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    // Generate FromGlib trait implementation.
    cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
    version_condition(w, env, enum_.version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl FromGlib<{sys_crate_name}::{ffi_name}> for {name} {{
    fn from_glib(value: {sys_crate_name}::{ffi_name}) -> Self {{
        {assert}match value {{",
        sys_crate_name = sys_crate_name,
        name = enum_.name,
        ffi_name = enum_.c_type,
        assert = assert
    )?;
    for member in &members {
        cfg_deprecated(w, env, member.deprecated_version, false, 3)?;
        version_condition(w, env, member.version, false, 3)?;
        writeln!(
            w,
            "\t\t\t{} => {}::{},",
            member.value, enum_.name, member.name
        )?;
    }
    writeln!(w, "\t\t\tvalue => {}::__Unknown(value),", enum_.name)?;
    writeln!(
        w,
        "{}",
        "        }
    }
}
"
    )?;

    // Generate ErrorDomain trait implementation.
    if let Some(ref get_quark) = get_error_quark_name(enum_) {
        let get_quark = get_quark.replace("-", "_");
        let has_failed_member = members.iter().any(|m| m.name == "Failed");

        cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
        version_condition(w, env, enum_.version, false, 0)?;
        writeln!(
            w,
            "impl ErrorDomain for {name} {{
    fn domain() -> Quark {{
        {assert}unsafe {{ from_glib({sys_crate_name}::{get_quark}()) }}
    }}

    fn code(self) -> i32 {{
        self.to_glib()
    }}

    fn from(code: i32) -> Option<Self> {{
        {assert}match code {{",
            sys_crate_name = sys_crate_name,
            name = enum_.name,
            get_quark = get_quark,
            assert = assert
        )?;

        for member in &members {
            cfg_deprecated(w, env, member.deprecated_version, false, 3)?;
            version_condition(w, env, member.version, false, 3)?;
            writeln!(
                w,
                "\t\t\t{} => Some({}::{}),",
                member.value, enum_.name, member.name
            )?;
        }
        if has_failed_member {
            writeln!(w, "\t\t\t_ => Some({}::Failed),", enum_.name)?;
        } else {
            writeln!(w, "\t\t\tvalue => Some({}::__Unknown(value)),", enum_.name)?;
        }

        writeln!(
            w,
            "{}",
            "        }
    }
}
"
        )?;
    }

    // Generate StaticType trait implementation.
    if let Some(ref get_type) = enum_.glib_get_type {
        cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
        version_condition(w, env, enum_.version, false, 0)?;
        writeln!(
            w,
            "impl StaticType for {name} {{
    fn static_type() -> Type {{
        unsafe {{ from_glib({sys_crate_name}::{get_type}()) }}
    }}
}}",
            sys_crate_name = sys_crate_name,
            name = enum_.name,
            get_type = get_type
        )?;
        writeln!(w)?;

        cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
        version_condition(w, env, enum_.version, false, 0)?;
        writeln!(
            w,
            "impl<'a> FromValueOptional<'a> for {name} {{
    unsafe fn from_value_optional(value: &Value) -> Option<Self> {{
        Some(FromValue::from_value(value))
    }}
}}",
            name = enum_.name,
        )?;
        writeln!(w)?;

        cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
        version_condition(w, env, enum_.version, false, 0)?;
        writeln!(
            w,
            "impl<'a> FromValue<'a> for {name} {{
    unsafe fn from_value(value: &Value) -> Self {{
        from_glib(gobject_sys::g_value_get_enum(value.to_glib_none().0))
    }}
}}",
            name = enum_.name,
        )?;
        writeln!(w)?;

        cfg_deprecated(w, env, enum_.deprecated_version, false, 0)?;
        version_condition(w, env, enum_.version, false, 0)?;
        writeln!(
            w,
            "impl SetValue for {name} {{
    unsafe fn set_value(value: &mut Value, this: &Self) {{
        gobject_sys::g_value_set_enum(value.to_glib_none_mut().0, this.to_glib())
    }}
}}",
            name = enum_.name,
        )?;
        writeln!(w)?;
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
