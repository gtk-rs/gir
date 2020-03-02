use crate::{
    analysis::{imports::Imports, namespaces},
    codegen::general::{
        self, cfg_deprecated, derives, version_condition, version_condition_string,
    },
    config::gobjects::GObject,
    env::Env,
    file_saver,
    library::*,
    nameutil::bitfield_member_name,
    traits::*,
};
use std::{
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
    let has_any = configs.iter().any(|c| {
        if let Type::Bitfield(_) = *env.library.type_(c.type_id.unwrap()) {
            true
        } else {
            false
        }
    });

    if !has_any {
        return;
    }
    let path = root_path.join("flags.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        let mut imports = Imports::new(&env.library);
        imports.add(env.main_sys_crate_name());
        imports.add("glib::translate::*");

        for config in &configs {
            if let Type::Bitfield(ref flags) = *env.library.type_(config.type_id.unwrap()) {
                if flags.glib_get_type.is_some() {
                    imports.add("glib::Type");
                    imports.add("glib::StaticType");
                    imports.add("glib::value::Value");
                    imports.add("glib::value::SetValue");
                    imports.add("glib::value::FromValue");
                    imports.add("glib::value::FromValueOptional");
                    imports.add("gobject_sys");
                    break;
                }
            }
        }

        general::start_comments(w, &env.config)?;
        general::uses(w, env, &imports)?;
        writeln!(w)?;

        mod_rs.push("\nmod flags;".into());
        for config in &configs {
            if let Type::Bitfield(ref flags) = *env.library.type_(config.type_id.unwrap()) {
                if let Some(cfg) = version_condition_string(env, flags.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!("pub use self::flags::{};", flags.name));
                generate_flags(env, w, flags, config)?;
            }
        }

        Ok(())
    });
}

fn generate_flags(env: &Env, w: &mut dyn Write, flags: &Bitfield, config: &GObject) -> Result<()> {
    let sys_crate_name = env.main_sys_crate_name();
    cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
    version_condition(w, env, flags.version, false, 0)?;
    writeln!(w, "bitflags! {{")?;
    if config.must_use {
        writeln!(w, "    #[must_use]")?;
    }

    if let Some(ref d) = config.derives {
        derives(w, &d, 1)?;
    }

    writeln!(w, "    pub struct {}: u32 {{", flags.name)?;
    for member in &flags.members {
        let member_config = config.members.matched(&member.name);
        let ignore = member_config.iter().any(|m| m.ignore);
        if ignore {
            continue;
        }

        let name = bitfield_member_name(&member.name);
        let val: i64 = member.value.parse().unwrap();
        let deprecated_version = member_config
            .iter()
            .filter_map(|m| m.deprecated_version)
            .next();
        let version = member_config.iter().filter_map(|m| m.version).next();
        cfg_deprecated(w, env, deprecated_version, false, 2)?;
        version_condition(w, env, version, false, 2)?;
        writeln!(w, "\t\tconst {} = {};", name, val as u32)?;
    }

    writeln!(
        w,
        "{}",
        "    }
}
"
    )?;

    cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
    version_condition(w, env, flags.version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl ToGlib for {name} {{
    type GlibType = {sys_crate_name}::{ffi_name};

    fn to_glib(&self) -> {sys_crate_name}::{ffi_name} {{
        self.bits()
    }}
}}
",
        sys_crate_name = sys_crate_name,
        name = flags.name,
        ffi_name = flags.c_type
    )?;

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
    version_condition(w, env, flags.version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl FromGlib<{sys_crate_name}::{ffi_name}> for {name} {{
    fn from_glib(value: {sys_crate_name}::{ffi_name}) -> {name} {{
        {assert}{name}::from_bits_truncate(value)
    }}
}}
",
        sys_crate_name = sys_crate_name,
        name = flags.name,
        ffi_name = flags.c_type,
        assert = assert
    )?;

    if let Some(ref get_type) = flags.glib_get_type {
        cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
        version_condition(w, env, flags.version, false, 0)?;
        writeln!(
            w,
            "impl StaticType for {name} {{
    fn static_type() -> Type {{
        unsafe {{ from_glib({sys_crate_name}::{get_type}()) }}
    }}
}}",
            sys_crate_name = sys_crate_name,
            name = flags.name,
            get_type = get_type
        )?;
        writeln!(w)?;

        cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
        version_condition(w, env, flags.version, false, 0)?;
        writeln!(
            w,
            "impl<'a> FromValueOptional<'a> for {name} {{
    unsafe fn from_value_optional(value: &Value) -> Option<Self> {{
        Some(FromValue::from_value(value))
    }}
}}",
            name = flags.name,
        )?;
        writeln!(w)?;

        cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
        version_condition(w, env, flags.version, false, 0)?;
        writeln!(
            w,
            "impl<'a> FromValue<'a> for {name} {{
    unsafe fn from_value(value: &Value) -> Self {{
        from_glib(gobject_sys::g_value_get_flags(value.to_glib_none().0))
    }}
}}",
            name = flags.name,
        )?;
        writeln!(w)?;

        cfg_deprecated(w, env, flags.deprecated_version, false, 0)?;
        version_condition(w, env, flags.version, false, 0)?;
        writeln!(
            w,
            "impl SetValue for {name} {{
    unsafe fn set_value(value: &mut Value, this: &Self) {{
        gobject_sys::g_value_set_flags(value.to_glib_none_mut().0, this.to_glib())
    }}
}}",
            name = flags.name,
        )?;

        writeln!(w)?;
    }

    Ok(())
}
