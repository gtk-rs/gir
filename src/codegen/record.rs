use std::io::{Result, Write};

use super::{function, general, trait_impls};
use crate::{
    analysis::{self, record_type::RecordType, special_functions::Type},
    env::Env,
    library,
    traits::MaybeRef,
};

pub fn generate(w: &mut dyn Write, env: &Env, analysis: &analysis::record::Info) -> Result<()> {
    let type_ = analysis.type_(&env.library);

    general::start_comments(w, &env.config)?;
    general::uses(w, env, &analysis.imports, type_.version)?;

    if RecordType::of(env.type_(analysis.type_id).maybe_ref().unwrap()) == RecordType::AutoBoxed {
        if let Some((ref glib_get_type, _)) = analysis.glib_get_type {
            general::define_auto_boxed_type(
                w,
                env,
                &analysis.name,
                &type_.c_type,
                analysis.boxed_inline,
                &analysis.init_function_expression,
                &analysis.copy_into_function_expression,
                &analysis.clear_function_expression,
                glib_get_type,
                &analysis.derives,
                analysis.visibility,
            )?;
        } else {
            panic!(
                "Record {} has record_boxed=true but don't have glib:get_type function",
                analysis.name
            );
        }
    } else if let (Some(ref_fn), Some(unref_fn)) = (
        analysis.specials.traits().get(&Type::Ref),
        analysis.specials.traits().get(&Type::Unref),
    ) {
        general::define_shared_type(
            w,
            env,
            &analysis.name,
            &type_.c_type,
            &ref_fn.glib_name,
            &unref_fn.glib_name,
            analysis.glib_get_type.as_ref().map(|(f, v)| {
                if v > &analysis.version {
                    (f.clone(), *v)
                } else {
                    (f.clone(), None)
                }
            }),
            &analysis.derives,
            analysis.visibility,
        )?;
    } else if let (Some(copy_fn), Some(free_fn)) = (
        analysis.specials.traits().get(&Type::Copy),
        analysis.specials.traits().get(&Type::Free),
    ) {
        general::define_boxed_type(
            w,
            env,
            &analysis.name,
            &type_.c_type,
            copy_fn,
            &free_fn.glib_name,
            analysis.boxed_inline,
            &analysis.init_function_expression,
            &analysis.copy_into_function_expression,
            &analysis.clear_function_expression,
            analysis.glib_get_type.as_ref().map(|(f, v)| {
                if v > &analysis.version {
                    (f.clone(), *v)
                } else {
                    (f.clone(), None)
                }
            }),
            &analysis.derives,
            analysis.visibility,
        )?;
    } else {
        panic!(
            "Missing memory management functions for {}",
            analysis.full_name
        );
    }

    if analysis
        .functions
        .iter()
        .any(|f| f.status.need_generate() && !f.hidden)
    {
        writeln!(w)?;
        write!(w, "impl {} {{", analysis.name)?;

        for func_analysis in &analysis.functions {
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

        writeln!(w, "}}")?;
    }

    general::declare_default_from_new(w, env, &analysis.name, &analysis.functions, false)?;

    trait_impls::generate(
        w,
        env,
        &analysis.name,
        &analysis.functions,
        &analysis.specials,
        None,
        analysis.version,
        None, // There is no need for #[cfg()] since it's applied on the whole file.
    )?;

    if analysis.concurrency != library::Concurrency::None {
        writeln!(w)?;
    }

    match analysis.concurrency {
        library::Concurrency::Send | library::Concurrency::SendSync => {
            writeln!(w, "unsafe impl Send for {} {{}}", analysis.name)?;
        }
        _ => (),
    }

    if analysis.concurrency == library::Concurrency::SendSync {
        writeln!(w, "unsafe impl Sync for {} {{}}", analysis.name)?;
    }

    Ok(())
}

pub fn generate_reexports(
    env: &Env,
    analysis: &analysis::record::Info,
    module_name: &str,
    contents: &mut Vec<String>,
) {
    let cfg_condition = general::cfg_condition_string(analysis.cfg_condition.as_ref(), false, 0);
    let version_cfg = general::version_condition_string(
        env,
        Some(analysis.type_id.ns_id),
        analysis.version,
        false,
        0,
    );
    let mut cfg = String::new();
    if let Some(s) = cfg_condition {
        cfg.push_str(&s);
        cfg.push('\n');
    };
    if let Some(s) = version_cfg {
        cfg.push_str(&s);
        cfg.push('\n');
    };
    contents.push(String::new());
    contents.push(format!("{cfg}mod {module_name};"));
    contents.push(format!(
        "{}{} use self::{}::{};",
        cfg,
        analysis.visibility.export_visibility(),
        module_name,
        analysis.name
    ));
}
