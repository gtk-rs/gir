use std::collections::HashMap;
use std::io::{Result, Write};
use case::CaseExt;

use analysis::c_type::rustify_pointers;
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
    try!(writeln!(w, ""));

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
    try!(generate_unions(w, env, &unions));
    try!(functions::generate_callbacks(w, env, &prepare(ns)));
    try!(generate_records(w, env, &records));
    try!(generate_classes_structs(w, env, &classes));
    try!(generate_interfaces_structs(w, &interfaces));

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
        if let Some(x) = typ.maybe_ref() {
            vec.push(x);
        }
    }
    vec.sort();
    vec
}

fn generate_aliases(w: &mut Write, env: &Env, items: &[&Alias])
        -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Aliases"));
    }
    for item in items {
        let (comment, c_type) = match ffi_type(env, item.typ, &item.target_c_type) {
            Ok(x) => ("", x),
            x @ Err(..) => ("//", x.into_string()),
        };
        try!(writeln!(w, "{}pub type {} = {};", comment, item.c_identifier, c_type));
    }
    if !items.is_empty() {
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_bitfields(w: &mut Write, env: &Env, items: &[&Bitfield])
        -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Flags"));
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);

        try!(writeln!(w, "bitflags! {{\n\t#[repr(C)]\n\tpub flags {}: c_uint {{", item.c_type));
        for member in &item.members {
            let member_config = config.as_ref()
                .map(|c| c.members.matched(&member.name)).unwrap_or_else(|| vec![]);
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
    if !constants.is_empty() {
        try!(writeln!(w, "// Constants"));
    }
    for constant in constants {
        let (mut comment, mut type_) =
            match ffi_type(env, constant.typ, &constant.c_type) {
                Ok(x) => ("", x),
                x @ Err(..) => ("//", x.into_string()),
            };
        if env.type_status_sys(&format!("{}.{}", env.config.library_name,
            constant.name)).ignored() {
            comment = "//";
        }
        let mut value = constant.value.clone();
        if type_ == "*mut c_char" {
            type_ = "*const c_char".into();
            value = format!("b\"{}\\0\" as *const u8 as *const c_char", escape_string(&value));
        } else if type_ == "gboolean" {
            let prefix = if env.config.library_name == "GLib" { "" } else { "glib::" };
            if value == "true" {
                value = format!("{}GTRUE", prefix);
            } else {
                value = format!("{}GFALSE", prefix);
            }
        }

        if let Some(_) = env.library.type_(constant.typ).maybe_ref_as::<Bitfield>() {
            try!(writeln!(w, "{}pub const {}: {} = {2} {{ bits: {} }};", comment,
                constant.c_identifier, type_, value));
        } else {
            try!(writeln!(w, "{}pub const {}: {} = {};", comment,
                constant.c_identifier, type_, value));
        }
    }
    if !constants.is_empty() {
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn escape_string(s: &str) -> String {
    let mut es = String::with_capacity(s.len() * 2);
    let _ = s.chars().map(|c| {
        match c {
            '\'' | '\"' | '\\' => {
                es.push('\\');
                es.push(c)
            }
            _ => es.push(c),
        }
    }).count();
    es
}

fn generate_enums(w: &mut Write, env: &Env, items: &[&Enumeration])
        -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Enums"));
    }
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
                .map(|c| c.members.matched(&member.name)).unwrap_or_else(|| vec![]);
            let is_alias = member_config.iter().any(|m| m.alias);
            let version = member_config.iter().filter_map(|m| m.version).next();

            if is_alias || vals.get(&member.value).is_some() {
                continue;
            }
            try!(version_condition(w, env, version, false, 1));
            try!(writeln!(w, "\t{} = {},",
                          &prepare_enum_member_name(&member.name), member.value));
            vals.insert(member.value.clone(), (member.name.clone(), version));
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

fn generate_unions(w: &mut Write, env: &Env, items: &[&Union])
        -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Unions"));
    }

    for item in items {
        if let Some(ref c_type) = item.c_type {
            // TODO: GLib/GObject special cases until we have proper union support in Rust
            if env.config.library_name == "GLib" && c_type == "GMutex" {
                // Two c_uint or a pointer => 64 bits on all
                // platforms currently supported by GLib
                try!(writeln!(w, "#[repr(C)]\npub struct {}(u64); // union", c_type));
            } else {
                try!(writeln!(w, "pub type {} = c_void; // union", c_type));
            }
        }
    }
    if !items.is_empty() {
        try!(writeln!(w, ""));
    }

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

fn generate_classes_structs(w: &mut Write, env: &Env, classes: &[&Class]) -> Result<()> {
    if !classes.is_empty() {
        try!(writeln!(w, "// Classes"));
    }
    for klass in classes {
        let (lines, commented) = generate_fields(env, &klass.name, &klass.fields);

        let comment = if commented { "//" } else { "" };
        if lines.is_empty() {
            try!(writeln!(w, "{comment}#[repr(C)]\n{comment}pub struct {name}(c_void);\n", comment=comment, name=klass.c_type));
        }
        else {
            try!(writeln!(w, "{comment}#[repr(C)]\n{comment}pub struct {name} {{", comment=comment, name=klass.c_type));

            for line in lines {
                try!(writeln!(w, "{}{}", comment, line));
            }
            try!(writeln!(w, "{}}}\n", comment));
        }
    }

    Ok(())
}

fn generate_interfaces_structs(w: &mut Write, interfaces: &[&Interface]) -> Result<()> {
    if !interfaces.is_empty() {
        try!(writeln!(w, "// Interfaces"));
    }
    for interface in interfaces {
        try!(writeln!(w, "#[repr(C)]\npub struct {}(c_void);", interface.c_type));
    }
    if !interfaces.is_empty() {
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_records(w: &mut Write, env: &Env, records: &[&Record]) -> Result<()> {
    if !records.is_empty() {
        try!(writeln!(w, "// Records"));
    }
    for record in records {
        let (lines, commented) = generate_fields(env, &record.name, &record.fields);

        let comment = if commented { "//" } else { "" };
        if lines.is_empty() {
            try!(writeln!(w, "{}#[repr(C)]\n{0}pub struct {}(c_void);\n", comment, record.c_type));
        }
        else {
            if record.name == "Value" {
                try!(writeln!(w, "#[cfg(target_pointer_width = \"128\")]"));
                try!(writeln!(w, "const ERROR: () = \"Your pointers are too big.\";"));
                try!(writeln!(w, ""));
            }
            try!(writeln!(w, "{}#[repr(C)]\n{0}pub struct {} {{", comment, record.c_type));
            for line in lines {
                try!(writeln!(w, "{}{}", comment, line));
            }
            try!(writeln!(w, "{}}}\n", comment));
        }
    }
    Ok(())
}

// TODO: GLib/GObject special cases until we have proper union support in Rust
fn is_union_special_case(c_type: &Option<String>) -> bool {
    if let Some(ref c_type) = c_type.as_ref() {
        c_type.as_str() == "GMutex"
    } else {
        false
    }
}

fn generate_fields(env: &Env, struct_name: &str, fields: &[Field]) -> (Vec<String>, bool){
    let mut lines = Vec::new();
    let mut commented = false;
    let mut truncated = false;

    //TODO: remove after GObject-2.0.gir fixed
    // Fix for wrong GValue size on i686-pc-windows-gnu due `c:type="gpointer"` in data field
    // instead guint64
    let is_gvalue = env.config.library_name == "GObject" && struct_name == "Value";

    // TODO: GLib/GObject special cases until we have proper union support in Rust
    let is_gweakref = env.config.library_name == "GObject" && struct_name == "WeakRef";

    for field in fields {
        let is_union = env.library.type_(field.typ).maybe_ref_as::<Union>().is_some();
        let is_bits = field.bits.is_some();
        let is_ptr = {
             if let Some(ref c_type) = field.c_type {
                 !rustify_pointers(c_type).0.is_empty()
             } else {
                 false
             }
        };

        if !is_gweakref && !truncated && !is_ptr && (is_union || is_bits) && !is_union_special_case(&field.c_type) {
            warn!("Field `{}::{}` not expressible in Rust, truncated",
                  struct_name, field.name);
            lines.push("\t_truncated_record_marker: c_void,".to_owned());
            truncated = true;
        }

        if truncated {
            if is_union {
                lines.push("\t//union,".to_owned());
            }
            else {
                let bits = field.bits.map(|n| format!(": {}", n)).unwrap_or_else(|| "".to_owned());
                lines.push(
                    format!("\t//{}: {}{},", field.name,
                            field.c_type.as_ref().map(|s| &s[..]).unwrap_or("fn"), bits));
            };
            continue;
        }

        if let Some(ref c_type) = field.c_type {
            let name = mangle_keywords(&*field.name);
            let mut c_type = ffi_type(env, field.typ, c_type);
            if c_type.is_err() {
                commented = true;
            }
            if is_gvalue && field.name == "data" {
                c_type = Ok("[u64; 2]".to_owned());
            }
            lines.push(format!("\tpub {}: {},", name, c_type.into_string()));
        } else if is_gweakref {
            // union containing a single pointer
            lines.push(format!("\tpub priv_: gpointer,"));
        }
        else {
            let name = mangle_keywords(&*field.name);
            if let Some(func) =
                env.library.type_(field.typ).maybe_ref_as::<Function>() {
                    let (com, sig) = functions::function_signature(env, func, true);
                    lines.push(format!("\tpub {}: Option<unsafe extern \"C\" fn{}>,", name, sig));
                    commented |= com;
                }
            else if let Some(c_type) = env.library.type_(field.typ).get_glib_name() {
                warn!("Field `{}::{}` missing c:type assumed `{}`",
                      struct_name, field.name, c_type);
                let c_type = ffi_type(env, field.typ, c_type);
                if c_type.is_err() {
                    commented = true;
                }
                lines.push(format!("\tpub {}: {},", name, c_type.into_string()));
            }
            else {
                lines.push(format!("\tpub {}: [{:?} {}],",
                                   name, field.typ, field.typ.full_name(&env.library)));
                commented = true;
            }
        }
    }
    (lines, commented)
}
