use std::collections::HashMap;
use std::io::{Result, Write};
use case::CaseExt;

use analysis::ref_mode::RefMode;
use analysis::rust_type::parameter_rust_type;
use codegen::general::{self, version_condition};
use env::Env;
use file_saver::*;
use library::*;
use nameutil::*;
use super::ffi_type::ffi_type;
use super::functions;
use super::statics;
use traits::*;
use version::Version;

pub fn generate(env: &Env) {
    println!("generating sys for {}", env.config.library_name);

    let path =  env.config.target_path.join(file_name_sys("lib"));

    println!("Generating file {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_lib(w, env));
}

fn generate_lib(w: &mut Write, env: &Env) -> Result<()>{
    try!(general::start_comments(w, &env.config));
    try!(statics::begin(w));

    try!(generate_extern_crates(w, env));
    try!(statics::after_extern_crates(w));

    if env.config.library_name != "GLib" {
        try!(statics::use_glib(w));
    }
    match &*env.config.library_name {
        "GLib" => try!(statics::only_for_glib(w)),
        "GObject" => try!(statics::only_for_gobject(w)),
        "Gtk" => try!(statics::only_for_gtk(w)),
        _ => (),
    }

    let ns = env.library.namespace(MAIN_NAMESPACE);
    let records = prepare(ns);
    let classes = prepare(ns);
    let interfaces = prepare(ns);
    let bitfields = prepare(ns);
    let enums = prepare(ns);
    let unions = prepare(ns);

    try!(generate_aliases(w, env, &prepare(ns)));
    try!(generate_enums(w, env, &enums));
    try!(generate_constants(w, env, &ns.constants));
    try!(generate_bitfields(w, env, &bitfields));
    try!(generate_unions(w, &unions));
    try!(functions::generate_callbacks(w, env, &prepare(ns)));
    try!(generate_records(w, env, &records));
    try!(generate_classes_structs(w, &classes));
    try!(generate_interfaces_structs(w, &interfaces));

    try!(writeln!(w, ""));
    try!(writeln!(w, "extern \"C\" {{"));
    try!(functions::generate_enums_funcs(w, env, &enums));
    try!(functions::generate_bitfields_funcs(w, env, &bitfields));
    try!(functions::generate_unions_funcs(w, env, &unions));
    try!(functions::generate_records_funcs(w, env, &records));
    try!(functions::generate_classes_funcs(w, env, &classes));
    try!(functions::generate_interfaces_funcs(w, env, &interfaces));
    try!(functions::generate_other_funcs(w, env, &ns.functions));

    try!(writeln!(w, "\n}}"));

    Ok(())
}

fn generate_extern_crates(w: &mut Write, env: &Env) -> Result<()>{
    for library_name in &env.config.external_libraries {
        try!(writeln!(w, "extern crate {0}_sys as {0};", crate_name(library_name)));
    }

    Ok(())
}

fn prepare<T: Ord>(ns: &Namespace) -> Vec<&T>
where Type: MaybeRef<T> {
    let mut vec: Vec<&T> = Vec::with_capacity(ns.types.len());
    for typ in ns.types.iter().filter_map(|t| t.as_ref()) {
        if let Some(ref x) = typ.maybe_ref() {
            vec.push(x);
        }
    }
    vec.sort();
    vec
}

fn generate_aliases(w: &mut Write, env: &Env, items: &[&Alias])
        -> Result<()> {
    try!(writeln!(w, ""));
    for item in items {
        let (comment, c_type) = match ffi_type(env, item.typ, &item.target_c_type) {
            Ok(x) => ("", x),
            x @ Err(..) => ("//", x.into_string().into()),
        };
        try!(writeln!(w, "{}pub type {} = {};", comment, item.c_identifier, c_type));
    }

    Ok(())
}

fn generate_bitfields(w: &mut Write, env: &Env, items: &[&Bitfield])
        -> Result<()> {
    try!(writeln!(w, ""));
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);

        try!(writeln!(w, "bitflags! {{\n\t#[repr(C)]\n\tflags {}: c_uint {{", item.c_type));
        for member in &item.members {
            let member_config = config.as_ref()
                .map(|c| c.members.matched(&member.name)).unwrap_or(vec![]);
            let version = member_config.iter().filter_map(|m| m.version).next();

            try!(version_condition(w, env, version, false, 2));
            let val: i64 = member.value.parse().unwrap();
            try!(writeln!(w, "\t\tconst {} = {},", member.c_identifier, val as u32));
        }
        try!(writeln!(w, "\t}}\n}}"));
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_constants(w: &mut Write, env: &Env, constants: &[Constant]) -> Result<()> {
    try!(writeln!(w, ""));
    for constant in constants {
        let direction = ParameterDirection::In;
        let ref_mode = RefMode::of(&env.library, constant.typ, direction);
        let (mut comment, mut type_) =
            match parameter_rust_type(env, constant.typ, direction,
                                      Nullable(false), ref_mode) {
                Ok(x) => ("", x),
                x @ Err(..) => ("//", x.into_string().into()),
            };
        if env.type_status_sys(&format!("{}.{}", env.config.library_name,
            constant.name)).ignored() {
            comment = "//";
        }
        let mut value = constant.value.clone();
        if type_ == "&str" {
            type_ = "&'static str".into();
            value = format!("r##\"{}\"##", value);
        } else if type_ == "Glyph" && env.config.library_name == "Pango"  {
            //Fix single constant alias
            type_ = "PangoGlyph".into();
        }
        try!(writeln!(w, "{}pub const {}: {} = {};", comment,
            constant.c_identifier, type_, value));
    }

    Ok(())
}

fn generate_enums(w: &mut Write, env: &Env, items: &[&Enumeration])
        -> Result<()> {
    try!(writeln!(w, ""));
    for item in items {
        if item.members.len() == 1 {
            try!(writeln!(w, "pub type {} = c_int;", item.name));
            try!(writeln!(w, "pub const {}: {} = {};",
                          item.members[0].c_identifier, item.name, item.members[0].value));
            try!(writeln!(w, "pub type {} = {};", item.c_type, item.name));
            try!(writeln!(w, ""));
            continue;
        }

        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);

        let mut vals: HashMap<String, (String, Option<Version>)> = HashMap::new();
        try!(writeln!(w, "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n#[repr(C)]"));
        try!(writeln!(w, "pub enum {} {{", item.c_type));
        for member in &item.members {
            let member_config = config.as_ref()
                .map(|c| c.members.matched(&member.name)).unwrap_or(vec![]);
            let is_alias = member_config.iter().any(|m| m.alias);
            let version = member_config.iter().filter_map(|m| m.version).next();

            if is_alias || vals.get(&member.value).is_some() {
                continue;
            }
            try!(version_condition(w, env, version, false, 1));
            try!(writeln!(w, "\t{} = {},",
                          &prepare_enum_member_name(&member.name), member.value));
            vals.insert(member.value.clone(), (member.name.clone(), version.clone()));
        }
        try!(writeln!(w, "}}"));
        for member in &item.members {
            if let Some(&(ref value, version)) = vals.get(&member.value) {
                try!(version_condition(w, env, version, false, 0));
                try!(writeln!(w, "pub const {}: {} = {1}::{};", member.c_identifier, item.c_type,
                          &prepare_enum_member_name(value)));
            }
        }
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_unions(w: &mut Write, items: &[&Union])
        -> Result<()> {
    try!(writeln!(w, ""));
    for item in items {
        if let Some(ref c_type) = item.c_type {
            try!(writeln!(w, "pub type {} = c_void; // union", c_type));
        }
    }
    try!(writeln!(w, ""));

    Ok(())
}

fn prepare_enum_member_name(name: &str) -> String {
    let cameled = name.to_camel();
    if name.chars().next().unwrap().is_digit(10) {
        format!("_{}", cameled)
    } else {
        cameled
    }
}

fn generate_classes_structs(w: &mut Write, classes: &[&Class]) -> Result<()> {
    try!(writeln!(w, ""));
    for klass in classes {
        try!(writeln!(w, "#[repr(C)]\npub struct {}(c_void);", klass.c_type));
    }

    Ok(())
}

fn generate_interfaces_structs(w: &mut Write, interfaces: &[&Interface]) -> Result<()> {
    try!(writeln!(w, ""));
    for interface in interfaces {
        try!(writeln!(w, "#[repr(C)]\npub struct {}(c_void);", interface.c_type));
    }

    Ok(())
}

fn generate_records(w: &mut Write, env: &Env, records: &[&Record]) -> Result<()> {
    try!(writeln!(w, ""));
    for record in records {
        let mut lines = Vec::new();
        let mut commented = false;
        let mut truncated = false;
        for field in &record.fields {
            let is_union = env.library.type_(field.typ).maybe_ref_as::<Union>().is_some();
            let is_bits = field.bits.is_some();
            if !truncated && (is_union || is_bits) {
                warn!("Record `{}` field `{}` not expressible in Rust, truncated",
                      record.name, field.name);
                lines.push(format!("\t_truncated_record_marker: c_void,"));
                truncated = true;
            }
            if truncated {
                if is_union {
                    lines.push(format!("\t//union,"));
                }
                else {
                    let bits = field.bits.map(|n| format!(": {}", n)).unwrap_or("".into());
                    lines.push(
                        format!("\t//{}: {}{},", field.name,
                                field.c_type.as_ref().map(|s| &s[..]).unwrap_or("fn"), bits));
                };
                continue;
            }

            let vis = if field.private { "" } else { "pub " };

            if let Some(ref c_type) = field.c_type {
                let name = mangle_keywords(&*field.name);
                let c_type = ffi_type(env, field.typ, c_type);
                if c_type.is_err() {
                    commented = true;
                }
                lines.push(format!("\t{}{}: {},", vis, name, c_type.to_cow_str()));
            }
            else {
                let name = mangle_keywords(&*field.name);
                if let Some(ref func) =
                        env.library.type_(field.typ).maybe_ref_as::<Function>() {
                    let (com, sig) = functions::function_signature(env, func, true);
                    lines.push(format!("\t{}{}: Option<unsafe extern \"C\" fn{}>,", vis, name, sig));
                    commented |= com;
                }
                else if let Some(c_type) = env.library.type_(field.typ).get_glib_name() {
                    warn!("Record `{}`, field `{}` missing c:type assumed `{}`",
                          record.name, field.name, c_type);
                    let c_type = ffi_type(env, field.typ, c_type);
                    if c_type.is_err() {
                        commented = true;
                    }
                    lines.push(format!("\t{}{}: {},", vis, name, c_type.to_cow_str()));
                }
                else {
                    lines.push(format!("\t{}{}: [{:?} {}],",
                        vis, name, field.typ, field.typ.full_name(&env.library)));
                    commented = true;
                }
            }
        }
        let comment = if commented { "//" } else { "" };
        if lines.is_empty() {
            try!(writeln!(w, "{}#[repr(C)]\n{0}pub struct {}(c_void);\n", comment, record.c_type));
        }
        else {
            try!(writeln!(w, "{}#[repr(C)]\n{0}pub struct {} {{", comment, record.c_type));
            for line in lines {
                try!(writeln!(w, "{}{}", comment, line));
            }
            try!(writeln!(w, "{}}}\n", comment));
        }
    }
    Ok(())
}
