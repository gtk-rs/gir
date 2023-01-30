use std::io::{Result, Write};

use crate::{
    analysis::{
        functions::Info,
        special_functions::{Infos, Type},
    },
    codegen::general::{cfg_condition_no_doc, version_condition},
    version::Version,
    Env,
};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    type_name: &str,
    functions: &[Info],
    specials: &Infos,
    trait_name: Option<&str>,
    scope_version: Option<Version>,
    cfg_condition: Option<&str>,
) -> Result<()> {
    for (type_, special_info) in specials.traits().iter() {
        if let Some(info) = lookup(functions, &special_info.glib_name) {
            match type_ {
                Type::Compare => {
                    if !specials.has_trait(Type::Equal) {
                        generate_eq_compare(
                            w,
                            env,
                            type_name,
                            info,
                            trait_name,
                            scope_version,
                            cfg_condition,
                        )?;
                    }
                    generate_ord(
                        w,
                        env,
                        type_name,
                        info,
                        trait_name,
                        scope_version,
                        cfg_condition,
                    )?;
                }
                Type::Equal => generate_eq(
                    w,
                    env,
                    type_name,
                    info,
                    trait_name,
                    scope_version,
                    cfg_condition,
                )?,
                Type::Display => generate_display(
                    w,
                    env,
                    type_name,
                    info,
                    trait_name,
                    scope_version,
                    cfg_condition,
                )?,
                Type::Hash => generate_hash(
                    w,
                    env,
                    type_name,
                    info,
                    trait_name,
                    scope_version,
                    cfg_condition,
                )?,
                _ => {}
            }
        }
    }
    Ok(())
}

fn lookup<'a>(functions: &'a [Info], name: &str) -> Option<&'a Info> {
    functions
        .iter()
        .find(|f| !f.status.ignored() && f.glib_name == name)
}

fn generate_call(func_name: &str, args: &[&str], trait_name: Option<&str>) -> String {
    let mut args_string = String::new();
    let in_trait = trait_name.is_some();

    if in_trait {
        args_string.push_str("self");
    }

    if !args.is_empty() {
        if in_trait {
            args_string.push_str(", ");
        }
        args_string.push_str(&args.join(", "));
    }

    if let Some(trait_name) = trait_name {
        format!("{trait_name}::{func_name}({args_string})")
    } else {
        format!("self.{func_name}({args_string})")
    }
}

fn generate_display(
    w: &mut dyn Write,
    env: &Env,
    type_name: &str,
    func: &Info,
    trait_name: Option<&str>,
    scope_version: Option<Version>,
    cfg_condition: Option<&str>,
) -> Result<()> {
    use crate::analysis::out_parameters::Mode;

    writeln!(w)?;
    let version = Version::if_stricter_than(func.version, scope_version);
    version_condition(w, env, None, version, false, 0)?;
    cfg_condition_no_doc(w, cfg_condition, false, 0)?;

    let call = generate_call(func.codegen_name(), &[], trait_name);
    let body = if let Mode::Throws(_) = func.outs.mode {
        format!(
            "\
            if let Ok(val) = {call} {{
                f.write_str(val)
            }} else {{
                Err(fmt::Error)
            }}"
        )
    } else {
        format!("f.write_str(&{call})")
    };

    writeln!(
        w,
        "\
impl fmt::Display for {type_name} {{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{
        {body}
    }}
}}"
    )
}

fn generate_hash(
    w: &mut dyn Write,
    env: &Env,
    type_name: &str,
    func: &Info,
    trait_name: Option<&str>,
    scope_version: Option<Version>,
    cfg_condition: Option<&str>,
) -> Result<()> {
    writeln!(w)?;
    let version = Version::if_stricter_than(func.version, scope_version);
    version_condition(w, env, None, version, false, 0)?;
    cfg_condition_no_doc(w, cfg_condition, false, 0)?;

    let call = generate_call(func.codegen_name(), &[], trait_name);

    writeln!(
        w,
        "\
impl hash::Hash for {type_name} {{
    #[inline]
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {{
        hash::Hash::hash(&{call}, state)
    }}
}}"
    )
}

fn generate_eq(
    w: &mut dyn Write,
    env: &Env,
    type_name: &str,
    func: &Info,
    trait_name: Option<&str>,
    scope_version: Option<Version>,
    cfg_condition: Option<&str>,
) -> Result<()> {
    writeln!(w)?;
    let version = Version::if_stricter_than(func.version, scope_version);
    version_condition(w, env, None, version, false, 0)?;
    cfg_condition_no_doc(w, cfg_condition, false, 0)?;

    let call = generate_call(func.codegen_name(), &["other"], trait_name);

    writeln!(
        w,
        "\
impl PartialEq for {type_name} {{
    #[inline]
    fn eq(&self, other: &Self) -> bool {{
        {call}
    }}
}}

impl Eq for {type_name} {{}}"
    )
}

fn generate_eq_compare(
    w: &mut dyn Write,
    env: &Env,
    type_name: &str,
    func: &Info,
    trait_name: Option<&str>,
    scope_version: Option<Version>,
    cfg_condition: Option<&str>,
) -> Result<()> {
    writeln!(w)?;
    let version = Version::if_stricter_than(func.version, scope_version);
    version_condition(w, env, None, version, false, 0)?;
    cfg_condition_no_doc(w, cfg_condition, false, 0)?;

    let call = generate_call(func.codegen_name(), &["other"], trait_name);

    writeln!(
        w,
        "\
impl PartialEq for {type_name} {{
    #[inline]
    fn eq(&self, other: &Self) -> bool {{
        {call} == 0
    }}
}}

impl Eq for {type_name} {{}}"
    )
}

fn generate_ord(
    w: &mut dyn Write,
    env: &Env,
    type_name: &str,
    func: &Info,
    trait_name: Option<&str>,
    scope_version: Option<Version>,
    cfg_condition: Option<&str>,
) -> Result<()> {
    writeln!(w)?;
    let version = Version::if_stricter_than(func.version, scope_version);
    version_condition(w, env, None, version, false, 0)?;
    cfg_condition_no_doc(w, cfg_condition, false, 0)?;

    let call = generate_call(func.codegen_name(), &["other"], trait_name);

    writeln!(
        w,
        "\
impl PartialOrd for {type_name} {{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {{
        {call}.partial_cmp(&0)
    }}
}}

impl Ord for {type_name} {{
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {{
        {call}.cmp(&0)
    }}
}}"
    )
}
