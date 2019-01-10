use std::io::{Result, Write};

use case::CaseExt;
use analysis;
use library;
use env::Env;
use super::child_properties;
use super::function;
use super::general;
use super::properties;
use super::signal;
use super::trait_impls;
use super::trampoline;

pub fn generate(
    w: &mut Write,
    env: &Env,
    analysis: &analysis::object::Info,
    generate_display_trait: bool,
) -> Result<()> {
    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, env, &analysis.imports));

    try!(general::define_object_type(
        w,
        env,
        &analysis.name,
        &analysis.c_type,
        &analysis.c_class_type.as_ref().map(|s| &s[..]),
        &analysis.rust_class_type.as_ref().map(|s| &s[..]),
        &analysis.get_type,
        analysis.is_interface,
        &analysis.supertypes,
    ));

    if need_generate_inherent(analysis) {
        try!(writeln!(w));
        try!(write!(w, "impl {} {{", analysis.name));
        for func_analysis in &analysis.constructors() {
            try!(function::generate(w, env, func_analysis, false, false, 1));
        }

        if !need_generate_trait(analysis) {
            for func_analysis in &analysis.methods() {
                try!(function::generate(w, env, func_analysis, false, false, 1));
            }

            for property in &analysis.properties {
                try!(properties::generate(w, env, property, false, false, 1));
            }

            for child_property in &analysis.child_properties {
                try!(child_properties::generate(
                    w,
                    env,
                    child_property,
                    false,
                    false,
                    1,
                ));
            }
        }

        for func_analysis in &analysis.functions() {
            try!(function::generate(w, env, func_analysis, false, false, 1));
        }

        if !need_generate_trait(analysis) {
            for signal_analysis in analysis
                .signals
                .iter()
                .chain(analysis.notify_signals.iter())
            {
                try!(signal::generate(
                    w,
                    env,
                    signal_analysis,
                    &analysis.trampolines,
                    false,
                    false,
                    1,
                ));
            }
        }

        try!(writeln!(w, "}}"));

        try!(general::declare_default_from_new(
            w,
            env,
            &analysis.name,
            &analysis.functions
        ));
    }

    try!(trait_impls::generate(
        w,
        &analysis.name,
        &analysis.functions,
        &analysis.specials,
        if need_generate_trait(analysis) {
            Some(&analysis.trait_name)
        } else {
            None
        },
    ));

    if analysis.concurrency != library::Concurrency::None {
        try!(writeln!(w));
    }

    match analysis.concurrency {
        library::Concurrency::Send | library::Concurrency::SendSync => {
            try!(writeln!(w, "unsafe impl Send for {} {{}}", analysis.name));
        }
        library::Concurrency::SendUnique => {
            if env.namespaces.is_glib_crate {
                try!(writeln!(w, "unsafe impl ::SendUnique for {} {{", analysis.name));
            } else {
                try!(writeln!(w, "unsafe impl glib::SendUnique for {} {{", analysis.name));
            }

            try!(writeln!(w, "    fn is_unique(&self) -> bool {{"));
            try!(writeln!(w, "        self.ref_count() == 1"));
            try!(writeln!(w, "    }}"));

            try!(writeln!(w, "}}"));
        },
        _ => (),
    }

    if let library::Concurrency::SendSync = analysis.concurrency {
        try!(writeln!(w, "unsafe impl Sync for {} {{}}", analysis.name));
    }

    try!(writeln!(w));
    try!(writeln!(w, "pub const NONE_{}: Option<&{}> = None;", analysis.name.to_snake().to_uppercase(), analysis.name));

    if need_generate_trait(analysis) {
        try!(writeln!(w));
        try!(generate_trait(w, env, analysis));
    }

    if !analysis.trampolines.is_empty() {
        for trampoline in &analysis.trampolines {
            try!(trampoline::generate(
                w,
                env,
                trampoline,
                need_generate_trait(analysis),
                &analysis.name,
            ));
        }
    }

    if generate_display_trait {
        try!(writeln!(
            w,
            "\nimpl fmt::Display for {} {{",
            analysis.name,
        ));
        // Generate Display trait implementation.
        try!(writeln!(w, "\tfn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{\n\
                            \t\twrite!(f, \"{}\")\n\
                          \t}}\n\
                        }}", analysis.name));
    }

    Ok(())
}

fn generate_trait(
    w: &mut Write,
    env: &Env,
    analysis: &analysis::object::Info,
) -> Result<()> {
    try!(write!(w, "pub trait {}: 'static {{", analysis.trait_name));

    for func_analysis in &analysis.methods() {
        try!(function::generate(w, env, func_analysis, true, true, 1));
    }
    for property in &analysis.properties {
        try!(properties::generate(w, env, property, true, true, 1));
    }
    for child_property in &analysis.child_properties {
        try!(child_properties::generate(
            w,
            env,
            child_property,
            true,
            true,
            1,
        ));
    }
    for signal_analysis in analysis
        .signals
        .iter()
        .chain(analysis.notify_signals.iter())
    {
        try!(signal::generate(
            w,
            env,
            signal_analysis,
            &analysis.trampolines,
            true,
            true,
            1,
        ));
    }
    try!(writeln!(w, "}}"));

    try!(writeln!(w));
    try!(write!(
        w,
        "impl<O: IsA<{}>> {} for O {{",
        analysis.name,
        analysis.trait_name,
    ));

    for func_analysis in &analysis.methods() {
        try!(function::generate(w, env, func_analysis, true, false, 1));
    }
    for property in &analysis.properties {
        try!(properties::generate(w, env, property, true, false, 1));
    }
    for child_property in &analysis.child_properties {
        try!(child_properties::generate(
            w,
            env,
            child_property,
            true,
            false,
            1,
        ));
    }
    for signal_analysis in analysis
        .signals
        .iter()
        .chain(analysis.notify_signals.iter())
    {
        try!(signal::generate(
            w,
            env,
            signal_analysis,
            &analysis.trampolines,
            true,
            false,
            1,
        ));
    }
    try!(writeln!(w, "}}"));

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
    if let Some(ref class_name) = analysis.rust_class_type {
        contents.push(format!("pub use self::{}::{{{}, {}, {}}};", module_name, analysis.name, class_name, format!("NONE_{}", analysis.name.to_snake().to_uppercase())));
    } else {
        contents.push(format!("pub use self::{}::{{{}, {}}};", module_name, analysis.name, format!("NONE_{}", analysis.name.to_snake().to_uppercase())));
    }
    if need_generate_trait(analysis) {
        contents.extend_from_slice(&cfgs);
        contents.push(format!(
            "pub use self::{}::{};",
            module_name,
            analysis.trait_name
        ));
        for cfg in &cfgs {
            traits.push(format!("\t{}", cfg));
        }
        traits.push(format!("\tpub use super::{};", analysis.trait_name));
    }
}
