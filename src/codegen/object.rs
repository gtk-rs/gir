use super::{
    child_properties, function, general,
    general::{
        cfg_deprecated_string, not_version_condition_no_dox, version_condition,
        version_condition_no_doc, version_condition_string,
    },
    properties, signal, trait_impls,
};
use crate::{
    analysis::{
        self, object::has_builder_properties, ref_mode::RefMode, rust_type::RustType,
        special_functions::Type,
    },
    env::Env,
    library::{self, Nullable},
    nameutil,
    traits::IntoString,
};
use std::collections::{BTreeMap, HashSet};
use std::io::{Result, Write};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    analysis: &analysis::object::Info,
    generate_display_trait: bool,
) -> Result<()> {
    general::start_comments(w, &env.config)?;
    general::uses(w, env, &analysis.imports, analysis.version)?;

    // Collect all supertypes that were added at a later time. The `glib::wrapper!` call
    // needs to be done multiple times with different `#[cfg]` directives if there is a difference.
    let mut namespaces = Vec::new();
    for p in &analysis.supertypes {
        use crate::library::*;
        let mut versions = BTreeMap::new();

        match *env.library.type_(p.type_id) {
            Type::Interface(Interface { .. }) | Type::Class(Class { .. })
                if !p.status.ignored() =>
            {
                let full_name = p.type_id.full_name(&env.library);
                // TODO: Might want to add a configuration on the object to override this per
                // supertype in case the supertype existed in older versions but newly became on
                // for this very type.
                if let Some(object) = env.analysis.objects.get(&full_name) {
                    let parent_version = object.version;
                    let namespace_min_version = env
                        .config
                        .min_required_version(env, Some(object.type_id.ns_id));
                    if parent_version > analysis.version && parent_version > namespace_min_version {
                        versions
                            .entry(parent_version)
                            .and_modify(|t: &mut Vec<_>| t.push(p))
                            .or_insert_with(|| vec![p]);
                        if !versions.is_empty() {
                            namespaces.push((p.type_id.ns_id, versions));
                        }
                    }
                }
            }
            _ => continue,
        }
    }

    if namespaces.is_empty() {
        writeln!(w)?;
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
    } else {
        // Write the `glib::wrapper!` calls from the highest version to the lowest and remember
        // which supertypes have to be removed for the next call.
        let mut remove_types: HashSet<library::TypeId> = HashSet::new();

        let mut previous_version = None;
        let mut previous_ns_id = None;
        for (ns_id, versions) in namespaces.iter() {
            for (&version, stypes) in versions.iter().rev() {
                let supertypes = analysis
                    .supertypes
                    .iter()
                    .filter(|t| !remove_types.contains(&t.type_id))
                    .cloned()
                    .collect::<Vec<_>>();

                writeln!(w)?;
                if previous_version.is_some() {
                    not_version_condition_no_dox(w, env, Some(*ns_id), previous_version, false, 0)?;
                    version_condition_no_doc(w, env, Some(*ns_id), version, false, 0)?;
                } else {
                    version_condition(w, env, Some(*ns_id), version, false, 0)?;
                }
                general::define_object_type(
                    w,
                    env,
                    &analysis.name,
                    &analysis.c_type,
                    analysis.c_class_type.as_deref(),
                    &analysis.get_type,
                    analysis.is_interface,
                    &supertypes,
                )?;

                for t in stypes {
                    remove_types.insert(t.type_id);
                }

                previous_ns_id = Some(*ns_id);
                previous_version = version;
            }
        }

        // Write the base `glib::wrapper!`.
        let supertypes = analysis
            .supertypes
            .iter()
            .filter(|t| !remove_types.contains(&t.type_id))
            .cloned()
            .collect::<Vec<_>>();
        writeln!(w)?;
        not_version_condition_no_dox(w, env, previous_ns_id, previous_version, false, 0)?;
        general::define_object_type(
            w,
            env,
            &analysis.name,
            &analysis.c_type,
            analysis.c_class_type.as_deref(),
            &analysis.get_type,
            analysis.is_interface,
            &supertypes,
        )?;
    }

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

        if has_builder_properties(&analysis.builder_properties) {
            // generate builder method that returns the corresponding builder
            let builder_name = format!("{}Builder", analysis.name);
            writeln!(
                w,
                "
            // rustdoc-stripper-ignore-next
            /// Creates a new builder-pattern struct instance to construct [`{name}`] objects.
            ///
            /// This method returns an instance of [`{builder_name}`] which can be used to create [`{name}`] objects.
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

        general::declare_default_from_new(
            w,
            env,
            &analysis.name,
            &analysis.functions,
            has_builder_properties(&analysis.builder_properties),
        )?;
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
        None, // There is no need for #[cfg()] since it's applied on the whole file.
    )?;

    if has_builder_properties(&analysis.builder_properties) {
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
        writeln!(
            w,
            "
impl {} {{
    pub const NONE: Option<&'static {}> = None;
}}
",
            analysis.name, analysis.name
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
        /// A [builder-pattern] type to construct [`{}`] objects.
        ///
        /// [builder-pattern]: https://doc.rust-lang.org/1.0.0/style/ownership/builders.html",
        analysis.name,
    )?;
    writeln!(w, "pub struct {}Builder {{", analysis.name)?;
    for (builder_props, super_tid) in &analysis.builder_properties {
        for property in builder_props {
            match RustType::try_new(env, property.typ) {
                Ok(type_string) => {
                    let type_string = match type_string.as_str() {
                        s if nameutil::is_gstring(s) => "String",
                        "Vec<GString>" | "Vec<glib::GString>" | "Vec<crate::GString>" => {
                            "Vec<String>"
                        }
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
                        "&str" => (None, String::new(), ".to_string()"),
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
                                            false,
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

                    let version_condition_string = version_condition_string(
                        env,
                        Some(super_tid.ns_id),
                        property.version,
                        false,
                        1,
                    );
                    let deprecated_string = cfg_deprecated_string(
                        env,
                        Some(*super_tid),
                        property.deprecated_version,
                        false,
                        1,
                    );
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
                    properties.push((property, super_tid));
                }
                Err(_) => writeln!(w, "    //{}: /*Unknown type*/,", property.name)?,
            }
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
    for (property, super_tid) in &properties {
        let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));
        version_condition_no_doc(w, env, Some(super_tid.ns_id), property.version, false, 2)?;
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
        || has_builder_properties(&analysis.builder_properties)
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
    if let Some(cfg) = general::cfg_condition_string(analysis.cfg_condition.as_ref(), false, 0) {
        cfgs.push(cfg);
    }
    if let Some(cfg) = general::version_condition_string(env, None, analysis.version, false, 0) {
        cfgs.push(cfg);
    }
    if let Some(cfg) = general::cfg_deprecated_string(
        env,
        Some(analysis.type_id),
        analysis.deprecated_version,
        false,
        0,
    ) {
        cfgs.push(cfg);
    }

    contents.push("".to_owned());
    contents.extend_from_slice(&cfgs);
    contents.push(format!("mod {};", module_name));
    contents.extend_from_slice(&cfgs);

    contents.push(format!("pub use self::{}::{};", module_name, analysis.name,));

    if need_generate_trait(analysis) {
        for cfg in &cfgs {
            traits.push(format!("\t{}", cfg));
        }
        traits.push(format!(
            "\tpub use super::{}::{};",
            module_name, analysis.trait_name
        ));
    }
}
