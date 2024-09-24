use std::{
    io::{prelude::*, Result},
    path::Path,
};

use super::{function, general::allow_deprecated, trait_impls};
use crate::nameutil::flag_name;
use crate::{
    analysis::flags::Info,
    codegen::{
        general::{
            self, cfg_condition, cfg_condition_doc, cfg_condition_no_doc, cfg_condition_string,
            cfg_deprecated, derives, doc_alias, version_condition, version_condition_doc,
            version_condition_no_doc, version_condition_string,
        },
        generate_default_impl,
    },
    config::gobjects::GObject,
    env::Env,
    file_saver,
    library::*,
    nameutil::{bitfield_member_name, use_glib_type},
    traits::*,
};

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    if !env
        .analysis
        .flags
        .iter()
        .any(|f| env.config.objects[&f.full_name].status.need_generate())
    {
        return;
    }

    let path = root_path.join("flags.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::uses(w, env, &env.analysis.flags_imports, None)?;
        writeln!(w)?;

        mod_rs.push("\nmod flags;".into());
        for flags_analysis in &env.analysis.flags {
            let config = &env.config.objects[&flags_analysis.full_name];
            if !config.status.need_generate() {
                continue;
            }
            let flags = flags_analysis.type_(&env.library);

            if let Some(cfg) = version_condition_string(env, None, flags.version, false, 0) {
                mod_rs.push(cfg);
            }
            if let Some(cfg) = cfg_condition_string(config.cfg_condition.as_ref(), false, 0) {
                mod_rs.push(cfg);
            }
            mod_rs.push(format!(
                "{}{} use self::flags::{};",
                flags
                    .deprecated_version
                    .map(|_| "#[allow(deprecated)]\n")
                    .unwrap_or(""),
                flags_analysis.visibility.export_visibility(),
                flag_name(&flags.name)
            ));
            generate_flags(env, w, flags, config, flags_analysis)?;
        }

        Ok(())
    });
}

fn generate_flags(
    env: &Env,
    w: &mut dyn Write,
    flags: &Bitfield,
    config: &GObject,
    analysis: &Info,
) -> Result<()> {
    let sys_crate_name = env.sys_crate_import(analysis.type_id);
    cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
    version_condition_no_doc(w, env, None, flags.version, false, 0)?;
    writeln!(w, "bitflags! {{")?;
    cfg_condition_doc(w, config.cfg_condition.as_ref(), false, 1)?;
    version_condition_doc(w, env, flags.version, false, 1)?;
    cfg_deprecated(
        w,
        env,
        Some(analysis.type_id),
        flags.deprecated_version,
        false,
        1,
    )?;
    if config.must_use {
        writeln!(w, "    #[must_use]")?;
    }

    if let Some(ref d) = config.derives {
        derives(w, d, 1)?;
    }
    writeln!(w, "    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]")?;

    doc_alias(w, &flags.c_type, "", 1)?;
    writeln!(
        w,
        "    {} struct {}: u32 {{",
        analysis.visibility,
        flag_name(&flags.name)
    )?;
    for member in &flags.members {
        let member_config = config.members.matched(&member.name);
        if member.status.ignored() {
            continue;
        }

        let name = bitfield_member_name(&member.name);
        let deprecated_version = member_config
            .iter()
            .find_map(|m| m.deprecated_version)
            .or(member.deprecated_version);
        let version = member_config
            .iter()
            .find_map(|m| m.version)
            .or(member.version);
        let cfg_cond = member_config.iter().find_map(|m| m.cfg_condition.as_ref());
        cfg_deprecated(w, env, Some(analysis.type_id), deprecated_version, false, 2)?;
        version_condition(w, env, None, version, false, 2)?;
        cfg_condition(w, cfg_cond, false, 2)?;
        if member.c_identifier != member.name {
            doc_alias(w, &member.c_identifier, "", 2)?;
        }
        writeln!(
            w,
            "\t\tconst {} = {}::{} as _;",
            name, sys_crate_name, member.c_identifier,
        )?;
    }

    writeln!(
        w,
        "    }}
}}"
    )?;

    let functions = analysis
        .functions
        .iter()
        .filter(|f| f.status.need_generate())
        .collect::<Vec<_>>();

    if !functions.is_empty() {
        writeln!(w)?;
        version_condition(w, env, None, flags.version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        write!(w, "impl {} {{", flag_name(&analysis.name))?;
        for func_analysis in functions {
            function::generate(
                w,
                env,
                Some(analysis.type_id),
                func_analysis,
                Some(&analysis.specials),
                flags.version,
                false,
                false,
                1,
            )?;
        }
        writeln!(w, "}}")?;
    }

    trait_impls::generate(
        w,
        env,
        &flag_name(&analysis.name),
        &analysis.functions,
        &analysis.specials,
        None,
        None,
        config.cfg_condition.as_deref(),
    )?;

    writeln!(w)?;

    generate_default_impl(
        w,
        env,
        config,
        &flag_name(&flags.name),
        flags.version,
        flags.members.iter(),
        |member| {
            let member_config = config.members.matched(&member.name);
            if member.status.ignored() {
                return None;
            }
            let version = member_config
                .iter()
                .find_map(|m| m.version)
                .or(member.version);
            let cfg_cond = member_config.iter().find_map(|m| m.cfg_condition.as_ref());
            Some((version, cfg_cond, bitfield_member_name(&member.name)))
        },
    )?;

    version_condition(w, env, None, flags.version, false, 0)?;
    cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
    allow_deprecated(w, flags.deprecated_version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl IntoGlib for {name} {{
    type GlibType = {sys_crate_name}::{ffi_name};

    #[inline]
    fn into_glib(self) -> {sys_crate_name}::{ffi_name} {{
        self.bits()
    }}
}}
",
        sys_crate_name = sys_crate_name,
        name = flag_name(&flags.name),
        ffi_name = flags.c_type
    )?;

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    version_condition(w, env, None, flags.version, false, 0)?;
    cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
    allow_deprecated(w, flags.deprecated_version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl FromGlib<{sys_crate_name}::{ffi_name}> for {name} {{
    #[inline]
    unsafe fn from_glib(value: {sys_crate_name}::{ffi_name}) -> Self {{
        {assert}Self::from_bits_truncate(value)
    }}
}}
",
        sys_crate_name = sys_crate_name,
        name = flag_name(&flags.name),
        ffi_name = flags.c_type,
        assert = assert
    )?;

    if let Some(ref get_type) = flags.glib_get_type {
        let configured_functions = config.functions.matched("get_type");
        let version = std::iter::once(flags.version)
            .chain(configured_functions.iter().map(|f| f.version))
            .max()
            .flatten();

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl StaticType for {name} {{
                #[inline]",
            name = flag_name(&flags.name),
        )?;
        doc_alias(w, get_type, "", 1)?;
        writeln!(
            w,
            "   fn static_type() -> {glib_type} {{
                    unsafe {{ from_glib({sys_crate_name}::{get_type}()) }}
                }}
            }}",
            sys_crate_name = sys_crate_name,
            get_type = get_type,
            glib_type = use_glib_type(env, "Type")
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl {has_param_spec} for {name} {{
                type ParamSpec = {param_spec_flags};
                type SetValue = Self;
                type BuilderFn = fn(&str) -> {param_spec_builder}<Self>;
    
                fn param_spec_builder() -> Self::BuilderFn {{
                    Self::ParamSpec::builder
                }}
}}",
            name = flag_name(&flags.name),
            has_param_spec = use_glib_type(env, "HasParamSpec"),
            param_spec_flags = use_glib_type(env, "ParamSpecFlags"),
            param_spec_builder = use_glib_type(env, "ParamSpecFlagsBuilder"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl {valuetype} for {name} {{
    type Type = Self;
}}",
            name = flag_name(&flags.name),
            valuetype = use_glib_type(env, "value::ValueType"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        writeln!(
            w,
            "unsafe impl<'a> {from_value_type}<'a> for {name} {{
    type Checker = {genericwrongvaluetypechecker}<Self>;

    #[inline]
    unsafe fn from_value(value: &'a {gvalue}) -> Self {{
        {assert}from_glib({glib}(value.to_glib_none().0))
    }}
}}",
            name = flag_name(&flags.name),
            glib = use_glib_type(env, "gobject_ffi::g_value_get_flags"),
            gvalue = use_glib_type(env, "Value"),
            genericwrongvaluetypechecker = use_glib_type(env, "value::GenericValueTypeChecker"),
            assert = assert,
            from_value_type = use_glib_type(env, "value::FromValue"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl ToValue for {name} {{
    #[inline]
    fn to_value(&self) -> {gvalue} {{
        let mut value = {gvalue}::for_value_type::<Self>();
        unsafe {{
            {glib}(value.to_glib_none_mut().0, self.into_glib());
        }}
        value
    }}

    #[inline]
    fn value_type(&self) -> {gtype} {{
        Self::static_type()
    }}
}}",
            name = flag_name(&flags.name),
            glib = use_glib_type(env, "gobject_ffi::g_value_set_flags"),
            gvalue = use_glib_type(env, "Value"),
            gtype = use_glib_type(env, "Type"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, flags.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl From<{name}> for {gvalue} {{
    #[inline]
    fn from(v: {name}) -> Self {{
        {assert}ToValue::to_value(&v)
    }}
}}",
            name = flag_name(&flags.name),
            gvalue = use_glib_type(env, "Value"),
            assert = assert,
        )?;
        writeln!(w)?;
    }

    Ok(())
}
