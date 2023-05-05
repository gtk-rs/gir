use std::{
    collections::{BTreeMap, HashSet},
    io::{Result, Write},
};

use super::{
    child_properties, function, general,
    general::{
        cfg_deprecated_string, not_version_condition_no_docsrs, version_condition,
        version_condition_no_doc, version_condition_string,
    },
    properties, signal, trait_impls,
};
use crate::{
    analysis::{
        self, bounds::BoundType, object::has_builder_properties, record_type::RecordType,
        ref_mode::RefMode, rust_type::RustType, special_functions::Type,
    },
    env::Env,
    library::{self, Nullable},
    nameutil,
    traits::IntoString,
};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    analysis: &analysis::object::Info,
    generate_display_trait: bool,
) -> Result<()> {
    general::start_comments(w, &env.config)?;
    if analysis
        .functions
        .iter()
        .any(|f| f.deprecated_version.is_some())
    {
        writeln!(w, "#![allow(deprecated)]")?;
    }
    general::uses(w, env, &analysis.imports, analysis.version)?;

    let config = &env.config.objects[&analysis.full_name];
    if config.default_value.is_some() {
        log::error!(
            "`default_value` can only be used on flags and enums. {} is neither. Ignoring \
             `default_value`.",
            analysis.name,
        );
    }

    // Collect all supertypes that were added at a later time. The `glib::wrapper!`
    // call needs to be done multiple times with different `#[cfg]` directives
    // if there is a difference.
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

    if namespaces.is_empty() || analysis.is_fundamental {
        writeln!(w)?;
        if analysis.is_fundamental {
            general::define_fundamental_type(
                w,
                env,
                &analysis.name,
                &analysis.c_type,
                &analysis.get_type,
                analysis.ref_fn.as_deref(),
                analysis.unref_fn.as_deref(),
                &analysis.supertypes,
                analysis.visibility,
            )?;
        } else {
            general::define_object_type(
                w,
                env,
                &analysis.name,
                &analysis.c_type,
                analysis.c_class_type.as_deref(),
                &analysis.get_type,
                analysis.is_interface,
                &analysis.supertypes,
                analysis.visibility,
            )?;
        }
    } else {
        // Write the `glib::wrapper!` calls from the highest version to the lowest and
        // remember which supertypes have to be removed for the next call.
        let mut remove_types: HashSet<library::TypeId> = HashSet::new();

        let mut previous_version = None;
        let mut previous_ns_id = None;
        for (ns_id, versions) in &namespaces {
            for (&version, stypes) in versions.iter().rev() {
                let supertypes = analysis
                    .supertypes
                    .iter()
                    .filter(|t| !remove_types.contains(&t.type_id))
                    .cloned()
                    .collect::<Vec<_>>();

                writeln!(w)?;
                if previous_version.is_some() {
                    not_version_condition_no_docsrs(
                        w,
                        env,
                        Some(*ns_id),
                        previous_version,
                        false,
                        0,
                    )?;
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
                    analysis.visibility,
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
        not_version_condition_no_docsrs(w, env, previous_ns_id, previous_version, false, 0)?;
        general::define_object_type(
            w,
            env,
            &analysis.name,
            &analysis.c_type,
            analysis.c_class_type.as_deref(),
            &analysis.get_type,
            analysis.is_interface,
            &supertypes,
            analysis.visibility,
        )?;
    }

    if (analysis.need_generate_inherent() && analysis.should_generate_impl_block())
        || !analysis.final_type
    {
        writeln!(w)?;
        write!(w, "impl {} {{", analysis.name)?;

        if !analysis.final_type {
            writeln!(
                w,
                "
        pub const NONE: Option<&'static {}> = None;
    ",
                analysis.name
            )?;
        }

        for func_analysis in &analysis.constructors() {
            function::generate(
                w,
                env,
                Some(analysis.type_id),
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
            /// This method returns an instance of [`{builder_name}`](crate::builders::{builder_name}) which can be used to create [`{name}`] objects.
            pub fn builder() -> {builder_name} {{
                {builder_name}::new()
            }}
        ",
                name = analysis.name,
                builder_name = builder_name
            )?;
        }

        if !analysis.need_generate_trait() {
            for func_analysis in &analysis.methods() {
                function::generate(
                    w,
                    env,
                    Some(analysis.type_id),
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
                Some(analysis.type_id),
                func_analysis,
                Some(&analysis.specials),
                analysis.version,
                false,
                false,
                1,
            )?;
        }

        if !analysis.need_generate_trait() {
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
        if analysis.need_generate_trait() {
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
        _ => (),
    }

    if let library::Concurrency::SendSync = analysis.concurrency {
        writeln!(w, "unsafe impl Sync for {} {{}}", analysis.name)?;
    }

    if analysis.need_generate_trait() {
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

fn generate_builder(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    let glib_crate_name = if env.namespaces.is_glib_crate {
        "crate"
    } else {
        "glib"
    };

    writeln!(
        w,
        "// rustdoc-stripper-ignore-next
        /// A [builder-pattern] type to construct [`{}`] objects.
        ///
        /// [builder-pattern]: https://doc.rust-lang.org/1.0.0/style/ownership/builders.html",
        analysis.name,
    )?;
    writeln!(w, "#[must_use = \"The builder must be built to be used\"]")?;
    writeln!(
        w,
        "pub struct {name}Builder {{
            builder: {glib_name}::object::ObjectBuilder<'static, {name}>,
        }}

        impl {name}Builder {{
        fn new() -> Self {{
            Self {{ builder: {glib_name}::object::Object::builder() }}
        }}",
        name = analysis.name,
        glib_name = glib_crate_name,
    )?;
    for (builder_props, super_tid) in &analysis.builder_properties {
        for property in builder_props {
            let direction = if property.is_get {
                library::ParameterDirection::In
            } else {
                library::ParameterDirection::Out
            };
            let param_type = RustType::builder(env, property.typ)
                .direction(direction)
                .ref_mode(property.set_in_ref_mode)
                .try_build();
            let comment_prefix = if param_type.is_err() { "//" } else { "" };
            let mut param_type_str = param_type.into_string();
            let (param_type_override, bounds, conversion) = match param_type_str.as_str() {
                "&str" => (
                    Some(format!("impl Into<{glib_crate_name}::GString>")),
                    String::new(),
                    ".into()",
                ),
                "&[&str]" => (
                    Some(format!("impl Into<{glib_crate_name}::StrV>")),
                    String::from(""),
                    ".into()",
                ),
                _ if !property.bounds.is_empty() => {
                    let (bounds, _) = function::bounds(&property.bounds, &[], false, false);
                    let param_bound = property.bounds.get_parameter_bound(&property.name);
                    let alias = param_bound.map(|bound| {
                        bound.full_type_parameter_reference(RefMode::ByRef, Nullable(false), false)
                    });
                    let conversion = param_bound.and_then(|bound| match bound.bound_type {
                        BoundType::AsRef(_) => Some(".as_ref().clone()"),
                        _ => None,
                    });
                    (alias, bounds, conversion.unwrap_or(".clone().upcast()"))
                }
                typ if typ.starts_with('&') => {
                    let should_clone =
                        if let crate::library::Type::Record(record) = env.type_(property.typ) {
                            match RecordType::of(record) {
                                RecordType::Boxed => "",
                                RecordType::AutoBoxed => {
                                    if !record.has_copy() {
                                        ""
                                    } else {
                                        ".clone()"
                                    }
                                }
                                _ => ".clone()",
                            }
                        } else {
                            ".clone()"
                        };

                    (None, String::new(), should_clone)
                }
                _ => (None, String::new(), ""),
            };
            if let Some(param_type_override) = param_type_override {
                param_type_str = param_type_override.to_string();
            }
            let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));

            let version_condition_string =
                version_condition_string(env, Some(super_tid.ns_id), property.version, false, 1);
            let deprecated_string =
                cfg_deprecated_string(env, Some(*super_tid), property.deprecated_version, false, 1);
            let version_prefix = version_condition_string
                .map(|version| format!("{comment_prefix}{version}\n"))
                .unwrap_or_default();

            let deprecation_prefix = deprecated_string
                .map(|version| format!("{comment_prefix}{version}\n"))
                .unwrap_or_default();

            writeln!(
                w,
                "
                        {version_prefix}{deprecation_prefix}    {comment_prefix}pub fn {name}{bounds}(self, {name}: {param_type_str}) -> Self {{
                        {comment_prefix}    Self {{ builder: self.builder.property(\"{property_name}\", {name}{conversion}), }}
                        {comment_prefix}}}",
                property_name = property.name,
            )?;
        }
    }

    writeln!(
        w,
        "
    // rustdoc-stripper-ignore-next
    /// Build the [`{name}`].
    #[must_use = \"Building the object from the builder is usually expensive and is not expected to have side effects\"]
    pub fn build(self) -> {name} {{",
        name = analysis.name,
    )?;

    // The split allows us to not have clippy::let_and_return lint disabled
    if let Some(code) = analysis.builder_postprocess.as_ref() {
        writeln!(w, "    let ret = self.builder.build();")?;
        writeln!(w, "        {{\n            {code}\n        }}")?;
        writeln!(w, "    ret\n    }}")?;
    } else {
        writeln!(w, "    self.builder.build() }}")?;
    }
    writeln!(w, "}}")
}

fn generate_trait(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    write!(
        w,
        "mod sealed {{
    pub trait Sealed {{}}
    impl<T: super::IsA<super::{1}>> Sealed for T {{}}
}}

pub trait {}: IsA<{}> + sealed::Sealed + 'static {{",
        analysis.trait_name, analysis.name
    )?;

    for func_analysis in &analysis.methods() {
        function::generate(
            w,
            env,
            Some(analysis.type_id),
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

    writeln!(w)?;
    writeln!(
        w,
        "impl<O: IsA<{}>> {} for O {{}}",
        analysis.name, analysis.trait_name,
    )?;

    Ok(())
}

pub fn generate_reexports(
    env: &Env,
    analysis: &analysis::object::Info,
    module_name: &str,
    contents: &mut Vec<String>,
    traits: &mut Vec<String>,
    builders: &mut Vec<String>,
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

    contents.push(String::new());
    contents.extend_from_slice(&cfgs);
    contents.push(format!("mod {module_name};"));
    contents.extend_from_slice(&cfgs);

    contents.push(format!(
        "{} use self::{}::{};",
        analysis.visibility.export_visibility(),
        module_name,
        analysis.name,
    ));

    if analysis.need_generate_trait() {
        for cfg in &cfgs {
            traits.push(format!("\t{cfg}"));
        }
        traits.push(format!(
            "\tpub use super::{}::{};",
            module_name, analysis.trait_name
        ));
    }

    if has_builder_properties(&analysis.builder_properties) {
        for cfg in &cfgs {
            builders.push(format!("\t{cfg}"));
        }
        builders.push(format!(
            "\tpub use super::{}::{}Builder;",
            module_name, analysis.name
        ));
    }
}
