use super::{child_properties, function, general, properties, signal, trait_impls, trampoline};
use crate::{
    analysis::{self, rust_type::rust_type},
    case::CaseExt,
    codegen::general::{version_condition, version_condition_string},
    env::Env,
    library, nameutil,
};
use std::io::{Result, Write};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    analysis: &analysis::object::Info,
    generate_display_trait: bool,
) -> Result<()> {
    general::start_comments(w, &env.config)?;
    general::uses(w, env, &analysis.imports)?;

    general::define_object_type(
        w,
        env,
        &analysis.name,
        &analysis.c_type,
        &analysis.c_class_type.as_ref().map(|s| &s[..]),
        &analysis.rust_class_type.as_ref().map(|s| &s[..]),
        &analysis.get_type,
        analysis.is_interface,
        &analysis.supertypes,
    )?;

    if need_generate_inherent(analysis) {
        writeln!(w)?;
        write!(w, "impl {} {{", analysis.name)?;
        for func_analysis in &analysis.constructors() {
            function::generate(w, env, func_analysis, false, false, 1)?;
        }

        if !need_generate_trait(analysis) {
            for func_analysis in &analysis.methods() {
                function::generate(w, env, func_analysis, false, false, 1)?;
            }

            for property in &analysis.properties {
                properties::generate(w, env, property, false, false, 1)?;
            }

            for child_property in &analysis.child_properties {
                child_properties::generate(w, env, child_property, false, false, 1)?;
            }
        }

        for func_analysis in &analysis.functions() {
            function::generate(w, env, func_analysis, false, false, 1)?;
        }

        if !need_generate_trait(analysis) {
            for signal_analysis in analysis
                .signals
                .iter()
                .chain(analysis.notify_signals.iter())
            {
                signal::generate(
                    w,
                    env,
                    signal_analysis,
                    &analysis.trampolines,
                    false,
                    false,
                    1,
                )?;
            }
        }

        writeln!(w, "}}")?;

        general::declare_default_from_new(w, env, &analysis.name, &analysis.functions)?;
    }

    trait_impls::generate(
        w,
        &analysis.name,
        &analysis.functions,
        &analysis.specials,
        if need_generate_trait(analysis) {
            Some(&analysis.trait_name)
        } else {
            None
        },
    )?;

    if analysis.concurrency != library::Concurrency::None {
        writeln!(w)?;
    }

    // TODO: include parent properties.
    if !analysis.builder_properties.is_empty() {
        generate_builder(w, env, analysis)?;
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

    if !analysis.trampolines.is_empty() {
        for trampoline in &analysis.trampolines {
            trampoline::generate(
                w,
                env,
                trampoline,
                need_generate_trait(analysis),
                &analysis.name,
            )?;
        }
    }

    if generate_display_trait {
        writeln!(w, "\nimpl fmt::Display for {} {{", analysis.name,)?;
        // Generate Display trait implementation.
        writeln!(
            w,
            "\tfn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{\n\
             \t\twrite!(f, \"{}\")\n\
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
    writeln!(
        w,
        "#[cfg(any(feature = \"builders\", feature = \"dox\"))]
pub struct {}Builder {{",
        analysis.name
    )?;
    for property in &analysis.builder_properties {
        match rust_type(env, property.typ) {
            Ok(type_string) => {
                let type_string = match type_string.as_str() {
                    "GString" => "String",
                    "Vec<GString>" => "Vec<String>",
                    typ => typ,
                };
                let (param_type, conversion) = match type_string {
                    "String" => ("&str", ".to_string()"),
                    typ => (typ, ""),
                };
                let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));
                let version_condition_string =
                    version_condition_string(env, property.version, false, 1);
                if let Some(ref version_condition_string) = version_condition_string {
                    writeln!(w, "{}", version_condition_string)?;
                }
                writeln!(w, "    {}: Option<{}>,", name, type_string)?;
                let prefix = version_condition_string
                    .map(|version| format!("{}\n", version))
                    .unwrap_or_default();
                methods.push(format!(
                    "\n{prefix}    pub fn {name}(mut self, {name}: {param_type}) -> Self {{
        self.{name} = Some({name}{conversion});
        self
    }}",
                    prefix = prefix,
                    param_type = param_type,
                    name = name,
                    conversion = conversion
                ));
                properties.push(property);
            }
            Err(_) => writeln!(w, "    //{}: /*Unknown type*/,", property.name)?,
        }
    }
    writeln!(
        w,
        "}}

#[cfg(any(feature = \"builders\", feature = \"dox\"))]
impl {}Builder {{
    pub fn new() -> Self {{
        Self {{",
        analysis.name
    )?;
    for property in &properties {
        version_condition(w, env, property.version, false, 3)?;
        let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));
        writeln!(w, "            {}: None,", name)?;
    }
    writeln!(
        w,
        "        }}
    }}

    pub fn build(self) -> {} {{
        let mut properties: Vec<(&str, &dyn ToValue)> = vec![];",
        analysis.name
    )?;
    for property in &properties {
        let name = nameutil::mangle_keywords(nameutil::signal_to_snake(&property.name));
        let version_condition_string = version_condition_string(env, property.version, false, 2);
        let condition_tabs = if version_condition_string.is_some() {
            "\t"
        } else {
            ""
        };
        if let Some(ref version_condition_string) = version_condition_string {
            writeln!(w, "{}", version_condition_string)?;
            writeln!(w, "        {{")?;
        }
        writeln!(
            w,
            "{tabs}        if let Some(ref {field}) = self.{field} {{
{tabs}            properties.push((\"{name}\", {field}));
{tabs}        }}",
            name = property.name,
            field = name,
            tabs = condition_tabs
        )?;
        if version_condition_string.is_some() {
            writeln!(w, "        }}")?;
        }
    }
    writeln!(w,
"        crate::Object::new({}::static_type(), &properties).expect(\"object new\").downcast().expect(\"downcast\")
    }}", analysis.name)?;
    for method in methods {
        writeln!(w, "{}", method)?;
    }
    writeln!(w, "}}")
}

fn generate_trait(w: &mut dyn Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    write!(w, "pub trait {}: 'static {{", analysis.trait_name)?;

    for func_analysis in &analysis.methods() {
        function::generate(w, env, func_analysis, true, true, 1)?;
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
        signal::generate(
            w,
            env,
            signal_analysis,
            &analysis.trampolines,
            true,
            true,
            1,
        )?;
    }
    writeln!(w, "}}")?;

    writeln!(w)?;
    write!(
        w,
        "impl<O: IsA<{}>> {} for O {{",
        analysis.name, analysis.trait_name,
    )?;

    for func_analysis in &analysis.methods() {
        function::generate(w, env, func_analysis, true, false, 1)?;
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
        signal::generate(
            w,
            env,
            signal_analysis,
            &analysis.trampolines,
            true,
            false,
            1,
        )?;
    }
    writeln!(w, "}}")?;

    Ok(())
}

fn need_generate_inherent(analysis: &analysis::object::Info) -> bool {
    analysis.has_constructors || analysis.has_functions || !need_generate_trait(analysis)
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
    contents.push("".to_owned());
    contents.extend_from_slice(&cfgs);
    contents.push(format!("mod {};", module_name));
    contents.extend_from_slice(&cfgs);

    let none_type = if !analysis.final_type {
        format!(", NONE_{}", analysis.name.to_snake().to_uppercase())
    } else {
        String::new()
    };

    if let Some(ref class_name) = analysis.rust_class_type {
        contents.push(format!(
            "pub use self::{}::{{{}, {}{}}};",
            module_name, analysis.name, class_name, none_type
        ));
    } else {
        contents.push(format!(
            "pub use self::{}::{{{}{}}};",
            module_name, analysis.name, none_type
        ));
    }
    if need_generate_trait(analysis) {
        contents.extend_from_slice(&cfgs);
        contents.push(format!(
            "pub use self::{}::{};",
            module_name, analysis.trait_name
        ));
        for cfg in &cfgs {
            traits.push(format!("\t{}", cfg));
        }
        traits.push(format!("\tpub use super::{};", analysis.trait_name));
    }

    if !analysis.builder_properties.is_empty() {
        contents.extend_from_slice(&cfgs);
        contents.push(format!(
            "#[cfg(any(feature = \"builders\", feature = \"dox\"))]
pub use self::{}::{}Builder;",
            module_name, analysis.name
        ));
    }
}
