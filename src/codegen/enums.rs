use std::{
    collections::HashSet,
    io::{prelude::*, Result},
    path::Path,
};

use super::{function, trait_impls};
use crate::{
    analysis::{enums::Info, special_functions::Type},
    codegen::{
        general::{
            self, allow_deprecated, cfg_condition, cfg_condition_no_doc, cfg_condition_string,
            cfg_deprecated, derives, doc_alias, version_condition, version_condition_no_doc,
            version_condition_string,
        },
        generate_default_impl,
    },
    config::gobjects::GObject,
    env::Env,
    file_saver,
    library::*,
    nameutil::{enum_member_name, use_glib_if_needed, use_glib_type},
    traits::*,
    version::Version,
};

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    if !env
        .analysis
        .enumerations
        .iter()
        .any(|e| env.config.objects[&e.full_name].status.need_generate())
    {
        return;
    }

    let path = root_path.join("enums.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::uses(w, env, &env.analysis.enum_imports, None)?;
        writeln!(w)?;

        mod_rs.push("\nmod enums;".into());
        for enum_analysis in &env.analysis.enumerations {
            let config = &env.config.objects[&enum_analysis.full_name];
            if !config.status.need_generate() {
                continue;
            }

            let enum_ = enum_analysis.type_(&env.library);

            if let Some(cfg) = version_condition_string(env, None, enum_.version, false, 0) {
                mod_rs.push(cfg);
            }
            if let Some(cfg) = cfg_condition_string(config.cfg_condition.as_ref(), false, 0) {
                mod_rs.push(cfg);
            }
            mod_rs.push(format!(
                "{}{} use self::enums::{};",
                enum_
                    .deprecated_version
                    .map(|_| "#[allow(deprecated)]\n")
                    .unwrap_or(""),
                enum_analysis.visibility.export_visibility(),
                enum_.name
            ));

            generate_enum(env, w, enum_, config, enum_analysis)?;
        }

        Ok(())
    });
}

fn generate_enum(
    env: &Env,
    w: &mut dyn Write,
    enum_: &Enumeration,
    config: &GObject,
    analysis: &Info,
) -> Result<()> {
    struct Member<'a> {
        name: String,
        c_name: String,
        version: Option<Version>,
        deprecated_version: Option<Version>,
        cfg_condition: Option<&'a String>,
    }

    let mut members: Vec<Member<'_>> = Vec::new();
    let mut vals: HashSet<String> = HashSet::new();
    let sys_crate_name = env.main_sys_crate_name();

    for member in &enum_.members {
        let member_config = config.members.matched(&member.name);
        let is_alias = member_config.iter().any(|m| m.alias);
        if is_alias || member.status.ignored() || vals.contains(&member.value) {
            continue;
        }
        vals.insert(member.value.clone());
        let deprecated_version = member_config
            .iter()
            .find_map(|m| m.deprecated_version)
            .or(member.deprecated_version);
        let version = member_config
            .iter()
            .find_map(|m| m.version)
            .or(member.version);
        let cfg_condition = member_config.iter().find_map(|m| m.cfg_condition.as_ref());
        members.push(Member {
            name: enum_member_name(&member.name),
            c_name: member.c_identifier.clone(),
            version,
            deprecated_version,
            cfg_condition,
        });
    }

    cfg_deprecated(
        w,
        env,
        Some(analysis.type_id),
        enum_.deprecated_version,
        false,
        0,
    )?;
    version_condition(w, env, None, enum_.version, false, 0)?;
    cfg_condition(w, config.cfg_condition.as_ref(), false, 0)?;
    if config.must_use {
        writeln!(w, "#[must_use]")?;
    }

    if let Some(ref d) = config.derives {
        derives(w, d, 1)?;
    } else {
        writeln!(w, "#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]")?;
    }
    writeln!(w, "#[derive(Clone, Copy)]")?;
    if config.exhaustive {
        writeln!(w, "#[repr(i32)]")?;
    } else {
        writeln!(w, "#[non_exhaustive]")?;
    }
    doc_alias(w, &enum_.c_type, "", 0)?;

    writeln!(w, "{} enum {} {{", analysis.visibility, enum_.name)?;
    for member in &members {
        cfg_deprecated(
            w,
            env,
            Some(analysis.type_id),
            member.deprecated_version,
            false,
            1,
        )?;
        version_condition(w, env, None, member.version, false, 1)?;
        cfg_condition(w, member.cfg_condition.as_ref(), false, 1)?;
        // Don't generate a doc_alias if the C name is the same as the Rust one
        if member.c_name != member.name {
            doc_alias(w, &member.c_name, "", 1)?;
        }
        if config.exhaustive {
            writeln!(
                w,
                "\t{} = {}::{},",
                member.name, sys_crate_name, member.c_name
            )?;
        } else {
            writeln!(w, "\t{},", member.name)?;
        }
    }

    if !config.exhaustive {
        writeln!(
            w,
            "\
    #[doc(hidden)]
    __Unknown(i32),",
        )?;
    }

    writeln!(w, "}}")?;

    let any_deprecated_version = enum_
        .deprecated_version
        .or_else(|| members.iter().find_map(|m| m.deprecated_version));

    let functions = analysis
        .functions
        .iter()
        .filter(|f| f.status.need_generate())
        .collect::<Vec<_>>();

    if !functions.is_empty() {
        writeln!(w)?;
        version_condition(w, env, None, enum_.version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
        write!(w, "impl {} {{", analysis.name)?;
        for func_analysis in functions {
            function::generate(
                w,
                env,
                Some(analysis.type_id),
                func_analysis,
                Some(&analysis.specials),
                enum_.version,
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
        &analysis.name,
        &analysis.functions,
        &analysis.specials,
        None,
        None,
        config.cfg_condition.as_deref(),
    )?;

    writeln!(w)?;

    if config.generate_display_trait && !analysis.specials.has_trait(Type::Display) {
        // Generate Display trait implementation.
        version_condition(w, env, None, enum_.version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, any_deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl fmt::Display for {0} {{\n\
             \tfn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{\n\
             \t\twrite!(f, \"{0}::{{}}\", match *self {{",
            enum_.name
        )?;
        for member in &members {
            version_condition_no_doc(w, env, None, member.version, false, 3)?;
            cfg_condition_no_doc(w, member.cfg_condition.as_ref(), false, 3)?;
            writeln!(w, "\t\t\tSelf::{0} => \"{0}\",", member.name)?;
        }

        if !config.exhaustive {
            writeln!(
                w,
                "\t\t\t_ => \"Unknown\",\n\
                 \t\t}})\n\
                 \t}}\n\
                 }}\n"
            )?;
        }
    }

    // Only inline from_glib / into_glib implementations if there are not many enums members
    let maybe_inline = if members.len() <= 12 || config.exhaustive {
        "#[inline]\n"
    } else {
        ""
    };

    // Generate IntoGlib trait implementation.
    version_condition(w, env, None, enum_.version, false, 0)?;
    cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
    allow_deprecated(w, any_deprecated_version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl IntoGlib for {name} {{
    type GlibType = {sys_crate_name}::{ffi_name};

    {maybe_inline}fn into_glib(self) -> {sys_crate_name}::{ffi_name} {{",
        sys_crate_name = sys_crate_name,
        name = enum_.name,
        ffi_name = enum_.c_type,
        maybe_inline = maybe_inline
    )?;

    if config.exhaustive {
        writeln!(
            w,
            "self as {sys_crate_name}::{ffi_name}",
            sys_crate_name = sys_crate_name,
            ffi_name = enum_.c_type,
        )?;
    } else {
        writeln!(w, "match self {{",)?;
        for member in &members {
            version_condition_no_doc(w, env, None, member.version, false, 3)?;
            cfg_condition_no_doc(w, member.cfg_condition.as_ref(), false, 3)?;
            writeln!(
                w,
                "\t\t\tSelf::{} => {}::{},",
                member.name, sys_crate_name, member.c_name
            )?;
        }
        writeln!(w, "\t\t\tSelf::__Unknown(value) => value,")?;
        writeln!(
            w,
            "\
        }}"
        )?;
    }

    writeln!(
        w,
        "\
    }}
}}
"
    )?;

    let assert = if env.config.generate_safety_asserts {
        "skip_assert_initialized!();\n\t\t"
    } else {
        ""
    };

    // Generate FromGlib trait implementation.
    version_condition(w, env, None, enum_.version, false, 0)?;
    cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
    allow_deprecated(w, any_deprecated_version, false, 0)?;
    writeln!(
        w,
        "#[doc(hidden)]
impl FromGlib<{sys_crate_name}::{ffi_name}> for {name} {{
    {maybe_inline}unsafe fn from_glib(value: {sys_crate_name}::{ffi_name}) -> Self {{
        {assert}",
        sys_crate_name = sys_crate_name,
        name = enum_.name,
        ffi_name = enum_.c_type,
        assert = assert,
        maybe_inline = maybe_inline
    )?;
    if config.exhaustive {
        let all_members = members
            .iter()
            .map(|m| format!("{}::{}", sys_crate_name, m.c_name))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(w, "debug_assert!([{all_members}].contains(&value));")?;
        writeln!(w, "std::mem::transmute(value)",)?;
    } else {
        writeln!(w, "match value {{")?;
        for member in &members {
            version_condition_no_doc(w, env, None, member.version, false, 3)?;
            cfg_condition_no_doc(w, member.cfg_condition.as_ref(), false, 3)?;
            writeln!(
                w,
                "\t\t\t{}::{} => Self::{},",
                sys_crate_name, member.c_name, member.name
            )?;
        }
        writeln!(w, "\t\t\tvalue => Self::__Unknown(value),")?;
        writeln!(
            w,
            "\
        }}"
        )?;
    }

    writeln!(
        w,
        "\
    }}
}}
"
    )?;

    // Generate ErrorDomain trait implementation.
    if let Some(ref domain) = enum_.error_domain {
        let has_failed_member = members.iter().any(|m| m.name == "Failed");

        version_condition(w, env, None, enum_.version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, any_deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl ErrorDomain for {name} {{
    #[inline]
    fn domain() -> Quark {{
        {assert}",
            name = enum_.name,
            assert = assert
        )?;

        match domain {
            ErrorDomain::Quark(quark) => {
                writeln!(
                    w,
                    "        static QUARK: once_cell::sync::Lazy<{0}ffi::GQuark> = once_cell::sync::Lazy::new(|| unsafe {{
            {0}ffi::g_quark_from_static_string(b\"{1}\\0\".as_ptr() as *const _)
        }});
        unsafe {{ from_glib(*QUARK) }}",
                    use_glib_if_needed(env, ""),
                    quark,
                )?;
            }
            ErrorDomain::Function(f) => {
                writeln!(w, "        unsafe {{ from_glib({sys_crate_name}::{f}()) }}")?;
            }
        }

        writeln!(
            w,
            "    }}

    #[inline]
    fn code(self) -> i32 {{
        self.into_glib()
    }}

    #[inline]
    #[allow(clippy::match_single_binding)]
    fn from(code: i32) -> Option<Self> {{
        {assert}match unsafe {{ from_glib(code) }} {{"
        )?;

        if has_failed_member && !config.exhaustive {
            writeln!(w, "\t\t\tSelf::__Unknown(_) => Some(Self::Failed),")?;
        }
        writeln!(w, "\t\t\tvalue => Some(value),")?;

        writeln!(
            w,
            "\
        }}
    }}
}}
"
        )?;
    }

    // Generate StaticType trait implementation.
    if let Some(ref get_type) = enum_.glib_get_type {
        let configured_functions = config.functions.matched("get_type");
        let version = std::iter::once(enum_.version)
            .chain(configured_functions.iter().map(|f| f.version))
            .max()
            .flatten();

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl StaticType for {name} {{
    #[inline]
    fn static_type() -> Type {{
        unsafe {{ from_glib({sys_crate_name}::{get_type}()) }}
    }}
}}",
            sys_crate_name = sys_crate_name,
            name = enum_.name,
            get_type = get_type
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl {has_param_spec} for {name} {{
                type ParamSpec = {param_spec_enum};
                type SetValue = Self;
                type BuilderFn = fn(&str, Self) -> {param_spec_builder}<Self>;
    
                fn param_spec_builder() -> Self::BuilderFn {{
                    |name, default_value| Self::ParamSpec::builder_with_default(name, default_value)
                }}
}}",
            name = enum_.name,
            has_param_spec = use_glib_type(env, "HasParamSpec"),
            param_spec_enum = use_glib_type(env, "ParamSpecEnum"),
            param_spec_builder = use_glib_type(env, "ParamSpecEnumBuilder"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl {valuetype} for {name} {{
    type Type = Self;
}}",
            name = enum_.name,
            valuetype = use_glib_type(env, "value::ValueType"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
        writeln!(
            w,
            "unsafe impl<'a> FromValue<'a> for {name} {{
    type Checker = {genericwrongvaluetypechecker}<Self>;

    #[inline]
    unsafe fn from_value(value: &'a {gvalue}) -> Self {{
        {assert}from_glib({glib}(value.to_glib_none().0))
    }}
}}",
            name = enum_.name,
            glib = use_glib_type(env, "gobject_ffi::g_value_get_enum"),
            gvalue = use_glib_type(env, "Value"),
            genericwrongvaluetypechecker = use_glib_type(env, "value::GenericValueTypeChecker"),
            assert = assert,
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
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
            name = enum_.name,
            glib = use_glib_type(env, "gobject_ffi::g_value_set_enum"),
            gvalue = use_glib_type(env, "Value"),
            gtype = use_glib_type(env, "Type"),
        )?;
        writeln!(w)?;

        version_condition(w, env, None, version, false, 0)?;
        cfg_condition_no_doc(w, config.cfg_condition.as_ref(), false, 0)?;
        allow_deprecated(w, enum_.deprecated_version, false, 0)?;
        writeln!(
            w,
            "impl From<{name}> for {gvalue} {{
    #[inline]
    fn from(v: {name}) -> Self {{
        {assert}ToValue::to_value(&v)
    }}
}}",
            name = enum_.name,
            gvalue = use_glib_type(env, "Value"),
            assert = assert,
        )?;
        writeln!(w)?;
    }

    generate_default_impl(
        w,
        env,
        config,
        &enum_.name,
        enum_.version,
        enum_.members.iter(),
        |member| {
            let e_member = members.iter().find(|m| m.c_name == member.c_identifier)?;
            let member_config = config.members.matched(&member.name);
            let version = member_config
                .iter()
                .find_map(|m| m.version)
                .or(e_member.version);
            let cfg_condition = member_config.iter().find_map(|m| m.cfg_condition.as_ref());
            Some((version, cfg_condition, e_member.name.as_str()))
        },
    )?;

    Ok(())
}
