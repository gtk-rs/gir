use std::{
    collections::{BTreeMap, HashSet},
    io::{Result, Write},
};

use super::{
    child_properties, function, general,
    general::{cfg_deprecated_string, version_condition_string},
    properties, signal, trait_impls,
};
use crate::{
    analysis::{
        self, bounds::BoundType, object::has_builder_properties, record_type::RecordType,
        ref_mode::RefMode, rust_type::RustType, safety_assertion_mode::SafetyAssertionMode,
    },
    env::Env,
    library, nameutil,
    traits::IntoString,
};

pub fn generate(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
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
    let mut ns_versions: BTreeMap<u16, BTreeMap<_, Vec<_>>> = BTreeMap::new();
    for p in &analysis.supertypes {
        use crate::library::*;

        match *env.library.type_(p.type_id) {
            Type::Interface(Interface { .. }) | Type::Class(Class { .. })
                if !p.status.ignored() =>
            {
                let full_name = p.type_id.full_name(&env.library);
                if let Some(object) = env.analysis.objects.get(&full_name) {
                    let parent_version = object.version;
                    let namespace_min_version = env
                        .config
                        .min_required_version(env, Some(object.type_id.ns_id));
                    if parent_version > analysis.version && parent_version > namespace_min_version {
                        ns_versions
                            .entry(object.type_id.ns_id)
                            .or_default()
                            .entry(parent_version)
                            .or_default()
                            .push(p);
                    }
                }
            }
            _ => continue,
        }
    }
    let namespaces: Vec<_> = ns_versions.into_iter().collect();

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
                analysis.type_id,
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
                analysis.type_id,
            )?;
        }
    } else {
        // Write the `glib::wrapper!` calls from the highest version to the lowest.
        // Each block is gated by its version feature and `not` of all higher version
        // features. The base block is gated by `not(any(all versioned features))`.
        let mut remove_types: HashSet<library::TypeId> = HashSet::new();

        for (ns_id, versions) in &namespaces {
            let namespace_name = if *ns_id == analysis::namespaces::MAIN {
                None
            } else {
                Some(env.namespaces[*ns_id].crate_name.clone())
            };

            let all_version_cfgs: Vec<String> = versions
                .keys()
                .filter_map(|v| v.map(|v| v.to_cfg(namespace_name.as_deref())))
                .collect();
            let n = all_version_cfgs.len();
            let mut rev_index = 0usize;

            for (&version, stypes) in versions.iter().rev() {
                let supertypes = analysis
                    .supertypes
                    .iter()
                    .filter(|t| !remove_types.contains(&t.type_id))
                    .cloned()
                    .collect::<Vec<_>>();

                writeln!(w)?;
                if version.is_some() {
                    let asc_index = n - 1 - rev_index;
                    let positive = &all_version_cfgs[..=asc_index];
                    let negative = all_version_cfgs.get(asc_index + 1);

                    let mut parts: Vec<String> = positive.to_vec();
                    if let Some(neg) = negative {
                        parts.push(format!("not({neg})"));
                    }

                    if parts.len() == 1 {
                        writeln!(w, "#[cfg({})]", parts[0])?;
                    } else {
                        writeln!(w, "#[cfg(all({}))]", parts.join(", "))?;
                    }

                    if rev_index == 0 {
                        let highest_cfg = &all_version_cfgs[n - 1];
                        writeln!(w, "#[cfg_attr(docsrs, doc(cfg({})))]", highest_cfg)?;
                    }

                    rev_index += 1;
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
                    analysis.type_id,
                )?;

                for t in stypes {
                    remove_types.insert(t.type_id);
                }
            }

            // Write the base `glib::wrapper!` gated by not(any(all versioned features)).
            let supertypes = analysis
                .supertypes
                .iter()
                .filter(|t| !remove_types.contains(&t.type_id))
                .cloned()
                .collect::<Vec<_>>();
            writeln!(w)?;
            if all_version_cfgs.len() == 1 {
                writeln!(w, "#[cfg(not({}))]", all_version_cfgs[0])?;
            } else {
                let all = all_version_cfgs.join(", ");
                writeln!(w, "#[cfg(not(any({all})))]")?;
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
                analysis.type_id,
            )?;
        }
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
                        bound.full_type_parameter_reference(RefMode::ByRef, false, false)
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
                                RecordType::AutoBoxed if !record.has_copy() => "",
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
        // We don't generate an assertion macro for the case where you have a build post-process
        // as it is only used to initialize gtk in gtk::ApplicationBuilder which is too early to assert anything
        writeln!(w, "    let ret = self.builder.build();")?;
        writeln!(w, "        {{\n            {code}\n        }}")?;
        writeln!(w, "    ret\n    }}")?;
    } else {
        if env.config.generate_safety_asserts {
            writeln!(w, "{}", SafetyAssertionMode::InMainThread)?;
        }
        writeln!(w, "    self.builder.build() }}")?;
    }
    writeln!(w, "}}")
}

fn generate_trait(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    write!(
        w,
        "pub trait {}: IsA<{}> + 'static {{",
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
