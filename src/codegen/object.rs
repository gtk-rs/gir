use super::{
    child_properties, function, general,
    general::{cfg_deprecated_string, version_condition_no_doc, version_condition_string},
    properties, signal, trait_impls,
};
use crate::{
    analysis::{self, ref_mode::RefMode, rust_type::RustType, special_functions::Type},
    case::CaseExt,
    env::Env,
    library::{self, Nullable},
    nameutil,
    traits::IntoString,
};
use std::io::{Result, Write};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    analysis: &analysis::object::Info,
    generate_display_trait: bool,
) -> Result<()> {
    general::start_comments(w, &env.config)?;
    general::uses(w, env, &analysis.imports, analysis.version)?;

    general::define_object_type(
        w,
        env,
        &analysis.name,
        &analysis.c_type,
        analysis.c_class_type.as_deref(),
        &analysis.get_type,
        analysis.is_interface,
        &analysis.supertypes,
    )?;

    if need_generate_inherent(analysis) {
        writeln!(w)?;
        write!(w, "impl {} {{", analysis.name)?;
        for func_analysis in &analysis.constructors() {
            function::generate(
                w,
                env,
                func_analysis,
                Some(&analysis.specials),
                analysis.version,
                false,
                false,
                1,
            )?;
        }

        if !analysis.builder_properties.is_empty() {
            // generate builder method that returns the corresponding builder
            let builder_name = format!("{}Builder", analysis.name);
            writeln!(
                w,
                "
            // rustdoc-stripper-ignore-next
            /// Creates a new builder-style object to construct a [`{name}`].
            ///
            /// This method returns an instance of [`{builder_name}`] which can be used to create a [`{name}`].
            pub fn builder() -> {builder_name} {{
                {builder_name}::default()
            }}
        ",
                name = analysis.name,
                builder_name = builder_name
            )?;
        }

        if !need_generate_trait(analysis) {
            for func_analysis in &analysis.methods() {
                function::generate(
                    w,
                    env,
                    func_analysis,
                    Some(&analysis.specials),
                    analysis.version,
                    false,
                    false,
                    1,
                )?;
            }

            for property in &analysis.properties {
                properties::generate(w, env, property, false, false, 1)?;
            }

            for child_property in &analysis.child_properties {
                child_properties::generate(w, env, child_property, false, false, 1)?;
            }
        }

        for func_analysis in &analysis.functions() {
            function::generate(
                w,
                env,
                func_analysis,
                Some(&analysis.specials),
                analysis.version,
                false,
                false,
                1,
            )?;
        }

        if !need_generate_trait(analysis) {
            for signal_analysis in analysis
                .signals
                .iter()
                .chain(analysis.notify_signals.iter())
            {
                signal::generate(w, env, signal_analysis, false, false, 1)?;
            }
        }

        writeln!(w, "}}")?;

        general::declare_default_from_new(w, env, &analysis.name, &analysis.functions)?;
    }

    trait_impls::generate(
        w,
        env,
        &analysis.name,
        &analysis.functions,
        &analysis.specials,
        if need_generate_trait(analysis) {
            Some(&analysis.trait_name)
        } else {
            None
        },
        analysis.version,
        &None, // There is no need for #[cfg()] since it's applied on the whole file.
    )?;

    if !analysis.builder_properties.is_empty() {
        writeln!(w)?;
        generate_builder(w, env, analysis)?;
    }

    if analysis.concurrency != library::Concurrency::None {
        writeln!(w)?;
    }

    match analysis.concurrency {
        library::Concurrency::Send | library::Concurrency::SendSync => {
            writeln!(w, "unsafe impl Send for {} {{}}", analysis.name)?;
        }
        library::Concurrency::SendUnique => {
            if env.namespaces.is_glib_crate {
                writeln!(w, "unsafe impl ::SendUnique for {} {{", analysis.name)?;
            } else {
                writeln!(w, "unsafe impl glib::SendUnique for {} {{", analysis.name)?;
            }

            writeln!(w, "    fn is_unique(&self) -> bool {{")?;
            writeln!(w, "        self.ref_count() == 1")?;
            writeln!(w, "    }}")?;

            writeln!(w, "}}")?;
        }
        _ => (),
    }

    if let library::Concurrency::SendSync = analysis.concurrency {
        writeln!(w, "unsafe impl Sync for {} {{}}", analysis.name)?;
    }

    if !analysis.final_type {
        writeln!(w)?;
        writeln!(
            w,
            "pub const NONE_{}: Option<&{}> = None;",
            analysis.name.to_snake().to_uppercase(),
            analysis.name
        )?;
    }

    if need_generate_trait(analysis) {
        writeln!(w)?;
        generate_trait(w, env, analysis)?;
    }

    if generate_display_trait && !analysis.specials.has_trait(Type::Display) {
        writeln!(w, "\nimpl fmt::Display for {} {{", analysis.name,)?;
        // Generate Display trait implementation.
        writeln!(
            w,
            "\tfn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{\n\
             \t\tf.write_str(\"{}\")\n\
             \t}}\n\
             }}",
            analysis.name
        )?;
    }

    Ok(())
}

// TODO: instead create a Vec<> inside the Builder instead of Options.
fn generate_builder(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    let mut methods = vec![];
    let mut properties = vec![];
    writeln!(w, "#[derive(Clone, Default)]")?;
    writeln!(
        w,
        "// rustdoc-stripper-ignore-next
        /// A builder for generating a [`{}`].",
        analysis.name,
    )?;
    writeln!(w, "pub struct {}Builder {{", analysis.name)?;

    for property in &analysis.builder_properties {
        match RustType::try_new(env, property.typ) {
            Ok(type_string) => {
                let type_string = match type_string.as_str() {
                    s if nameutil::is_gstring(s) => "String",
                    "Vec<GString>" | "Vec<glib::GString>" | "Vec<crate::GString>" => "Vec<String>",
                    typ => typ,
                };
                let direction = if property.is_get {
                    library::ParameterDirection::In
                } else {
                    library::ParameterDirection::Out
                };

                let mut param_type = RustType::builder(env, property.typ)
                    .direction(direction)
                    .ref_mode(property.set_in_ref_mode)
                    .try_build()
                    .into_string();

                let (param_type_override, bounds, conversion) = match &param_type[..] {
                    typ if nameutil::is_gstring(typ) && property.set_in_ref_mode.is_ref() => (
                        Some("P".to_string()),
                        "<P: Into<glib::GString>>".to_string(),
                        ".into().to_string()",
                    ),
                    "&[&str]" => (Some("Vec<String>".to_string()), String::new(), ""),
                    _ if !property.bounds.is_empty() => {
                        let (bounds, _) = function::bounds(&property.bounds, &[], false, false);
                        let alias =
                            property
                                .bounds
                                .get_parameter_bound(&property.name)
                                .map(|bound| {
                                    bound.full_type_parameter_reference(
                                        RefMode::ByRef,
                                        Nullable(false),
                                    )
                                });
                        (alias, bounds, ".clone().upcast()")
                    }
                    typ if typ.starts_with('&') => (None, String::new(), ".clone()"),
                    _ => (None, String::new(), ""),
                };
                if let Some(param_type_override) = param_type_override {
                    param_type = param_type_override.to_string();
                }
                let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));
                let version_condition_string =
                    version_condition_string(env, property.version, false, 1);
                let deprecated_string =
                    cfg_deprecated_string(env, property.deprecated_version, false, 1);
                if let Some(ref version_condition_string) = version_condition_string {
                    writeln!(w, "{}", version_condition_string)?;
                }
                if let Some(ref deprecated_string) = deprecated_string {
                    writeln!(w, "{}", deprecated_string)?;
                }
                writeln!(w, "    {}: Option<{}>,", name, type_string)?;
                let version_prefix = version_condition_string
                    .map(|version| format!("{}\n", version))
                    .unwrap_or_default();

                let deprecation_prefix = deprecated_string
                    .map(|version| format!("{}\n", version))
                    .unwrap_or_default();

                methods.push(format!(
                    "\n{version_prefix}{deprecation_prefix}    pub fn {name}{bounds}(mut self, {name}: {param_type}) -> Self {{
        self.{name} = Some({name}{conversion});
        self
    }}",
                    version_prefix = version_prefix,
                    deprecation_prefix = deprecation_prefix,
                    param_type = param_type,
                    name = name,
                    conversion = conversion,
                    bounds = bounds
                ));
                properties.push(property);
            }
            Err(_) => writeln!(w, "    //{}: /*Unknown type*/,", property.name)?,
        }
    }
    writeln!(
        w,
        "}}

impl {name}Builder {{
    // rustdoc-stripper-ignore-next
    /// Create a new [`{name}Builder`].
    pub fn new() -> Self {{
        Self::default()
    }}
",
        name = analysis.name
    )?;

    writeln!(
        w,
        "
    // rustdoc-stripper-ignore-next
    /// Build the [`{name}`].
    pub fn build(self) -> {name} {{
        let mut properties: Vec<(&str, &dyn ToValue)> = vec![];",
        name = analysis.name
    )?;
    for property in &properties {
        let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));
        version_condition_no_doc(w, env, property.version, false, 2)?;
        writeln!(
            w,
            "\
            if let Some(ref {field}) = self.{field} {{
                properties.push((\"{name}\", {field}));
            }}",
            name = property.name,
            field = name
        )?;
    }
    let glib_crate_name = if env.namespaces.is_glib_crate {
        "crate"
    } else {
        "glib"
    };

    // The split allows us to not have clippy::let_and_return lint disabled
    if let Some(code) = analysis.builder_postprocess.as_ref() {
        writeln!(
            w,
            r#"        let ret = {}::Object::new::<{}>(&properties)
                .expect("Failed to create an instance of {}");"#,
            glib_crate_name, analysis.name, analysis.name,
        )?;
        writeln!(w, "        {{\n            {}\n        }}", code)?;
        writeln!(w, "    ret\n    }}")?;
    } else {
        writeln!(
            w,
            r#"        {}::Object::new::<{}>(&properties)
                .expect("Failed to create an instance of {}")"#,
            glib_crate_name, analysis.name, analysis.name,
        )?;
        writeln!(w, "\n    }}")?;
    }

    for method in methods {
        writeln!(w, "{}", method)?;
    }
    writeln!(w, "}}")
}

fn generate_trait(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    write!(w, "pub trait {}: 'static {{", analysis.trait_name)?;

    for func_analysis in &analysis.methods() {
        function::generate(
            w,
            env,
            func_analysis,
            Some(&analysis.specials),
            analysis.version,
            true,
            true,
            1,
        )?;
    }
    for property in &analysis.properties {
        properties::generate(w, env, property, true, true, 1)?;
    }
    for child_property in &analysis.child_properties {
        child_properties::generate(w, env, child_property, true, true, 1)?;
    }
    for signal_analysis in analysis
        .signals
        .iter()
        .chain(analysis.notify_signals.iter())
    {
        signal::generate(w, env, signal_analysis, true, true, 1)?;
    }
    writeln!(w, "}}")?;

    writeln!(w)?;
    write!(
        w,
        "impl<O: IsA<{}>> {} for O {{",
        analysis.name, analysis.trait_name,
    )?;

    for func_analysis in &analysis.methods() {
        function::generate(
            w,
            env,
            func_analysis,
            Some(&analysis.specials),
            analysis.version,
            true,
            false,
            1,
        )?;
    }
    for property in &analysis.properties {
        properties::generate(w, env, property, true, false, 1)?;
    }
    for child_property in &analysis.child_properties {
        child_properties::generate(w, env, child_property, true, false, 1)?;
    }
    for signal_analysis in analysis
        .signals
        .iter()
        .chain(analysis.notify_signals.iter())
    {
        signal::generate(w, env, signal_analysis, true, false, 1)?;
    }
    writeln!(w, "}}")?;

    Ok(())
}

fn need_generate_inherent(analysis: &analysis::object::Info) -> bool {
    analysis.has_constructors
        || analysis.has_functions
        || !need_generate_trait(analysis)
        || !analysis.builder_properties.is_empty()
}

fn need_generate_trait(analysis: &analysis::object::Info) -> bool {
    analysis.generate_trait
}

pub fn generate_reexports(
    env: &Env,
    analysis: &analysis::object::Info,
    module_name: &str,
    contents: &mut Vec<String>,
    traits: &mut Vec<String>,
) {
    let mut cfgs: Vec<String> = Vec::new();
    if let Some(cfg) = general::cfg_condition_string(&analysis.cfg_condition, false, 0) {
        cfgs.push(cfg);
    }
    if let Some(cfg) = general::version_condition_string(env, analysis.version, false, 0) {
        cfgs.push(cfg);
    }
    if let Some(cfg) = general::cfg_deprecated_string(env, analysis.deprecated_version, false, 0) {
        cfgs.push(cfg);
    }

    contents.push("".to_owned());
    contents.extend_from_slice(&cfgs);
    contents.push(format!("mod {};", module_name));
    contents.extend_from_slice(&cfgs);

    let none_type = if !analysis.final_type {
        format!(", NONE_{}", analysis.name.to_snake().to_uppercase())
    } else {
        String::new()
    };

    contents.push(format!(
        "pub use self::{}::{{{}{}}};",
        module_name, analysis.name, none_type
    ));
    if need_generate_trait(analysis) {
        for cfg in &cfgs {
            traits.push(format!("\t{}", cfg));
        }
        traits.push(format!(
            "\tpub use super::{}::{};",
            module_name, analysis.trait_name
        ));
    }

    if !analysis.builder_properties.is_empty() {
        contents.extend_from_slice(&cfgs);
        contents.push(format!(
            "pub use self::{}::{}Builder;",
            module_name, analysis.name
        ));
    }
}
