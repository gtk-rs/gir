use std::collections::HashMap;
use std::fs;
use std::io::{Result, Write};

use codegen::general::{self, version_condition};
use config::ExternalLibrary;
use env::Env;
use file_saver::*;
use library::*;
use nameutil::*;
use super::ffi_type::ffi_type;
use super::fields;
use super::functions;
use super::statics;
use traits::*;
use version::Version;

pub fn generate(env: &Env) {
    info!("Generating sys for {}", env.config.library_name);

    let path = env.config.target_path.join(file_name_sys("lib"));

    info!("Generating file {:?}", path);
    save_to_file(&path, env.config.make_backup, |w| generate_lib(w, env));
}

fn generate_lib(w: &mut Write, env: &Env) -> Result<()> {
    try!(general::start_comments(w, &env.config));
    try!(statics::begin(w));

    try!(generate_extern_crates(w, env));
    try!(include_custom_modules(w, env));
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
    try!(generate_interfaces_structs(w, env, &interfaces));

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

fn generate_extern_crates(w: &mut Write, env: &Env) -> Result<()> {
    for library in &env.config.external_libraries {
        try!(w.write_all(get_extern_crate_string(library).as_bytes()));
    }

    Ok(())
}

fn get_extern_crate_string(library: &ExternalLibrary) -> String {
    format!(
        "extern crate {}_sys as {};\n",
        library.crate_name.replace("-", "_"),
        crate_name(&library.namespace)
    )
}

fn include_custom_modules(w: &mut Write, env: &Env) -> Result<()> {
    let modules = try!(find_modules(env));
    if !modules.is_empty() {
        try!(writeln!(w, ""));
        for module in &modules {
            try!(writeln!(w, "mod {};", module));
        }
        try!(writeln!(w, ""));
        for module in &modules {
            try!(writeln!(w, "pub use {}::*;", module));
        }
    }

    Ok(())
}

fn find_modules(env: &Env) -> Result<Vec<String>> {
    let path = env.config.target_path.join("src");

    let mut vec = Vec::<String>::new();
    for entry in try!(fs::read_dir(path)) {
        let path = try!(entry).path();
        let ext = match path.extension() {
            Some(ext) => ext,
            None => continue,
        };
        if ext != "rs" {
            continue;
        }
        let file_stem = path.file_stem().expect("No file name");
        if file_stem == "lib" {
            continue;
        }
        let file_stem = file_stem
            .to_str()
            .expect("Can't convert file name to string")
            .to_owned();
        vec.push(file_stem);
    }
    vec.sort();

    Ok(vec)
}

fn prepare<T: Ord>(ns: &Namespace) -> Vec<&T>
where
    Type: MaybeRef<T>,
{
    let mut vec: Vec<&T> = Vec::with_capacity(ns.types.len());
    for typ in ns.types.iter().filter_map(|t| t.as_ref()) {
        if let Some(x) = typ.maybe_ref() {
            vec.push(x);
        }
    }
    vec.sort();
    vec
}

fn generate_aliases(w: &mut Write, env: &Env, items: &[&Alias]) -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Aliases"));
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        let (comment, c_type) = match ffi_type(env, item.typ, &item.target_c_type) {
            Ok(x) => ("", x),
            x @ Err(..) => ("//", x.into_string()),
        };
        try!(writeln!(
            w,
            "{}pub type {} = {};",
            comment,
            item.c_identifier,
            c_type
        ));
    }
    if !items.is_empty() {
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_bitfields(w: &mut Write, env: &Env, items: &[&Bitfield]) -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Flags"));
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);
        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }
        let mut vals: HashMap<String, (String, Option<Version>)> = HashMap::new();

        try!(writeln!(
            w,
            "bitflags! {{\n\t#[repr(C)]\n\tpub struct {}: c_uint {{",
            item.c_type
        ));
        for member in &item.members {
            let member_config = config
                .as_ref()
                .map(|c| c.members.matched(&member.name))
                .unwrap_or_else(|| vec![]);
            let version = member_config.iter().filter_map(|m| m.version).next();

            try!(version_condition(w, env, version, false, 2));
            let val: i64 = member.value.parse().unwrap();
            try!(writeln!(
                w,
                "\t\tconst {} = {};",
                member.name.to_uppercase(),
                val as u32
            ));
            vals.insert(member.value.clone(), (member.name.clone(), version));
        }
        try!(writeln!(w, "\t}}\n}}"));

        for member in &item.members {
            if let Some(&(ref value, version)) = vals.get(&member.value) {
                try!(version_condition(w, env, version, false, 0));
                try!(writeln!(
                    w,
                    "pub const {}: {} = {1}::{};",
                    member.c_identifier,
                    item.c_type,
                    value.to_uppercase(),
                ));
            }
        }
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_constants(w: &mut Write, env: &Env, constants: &[Constant]) -> Result<()> {
    if !constants.is_empty() {
        try!(writeln!(w, "// Constants"));
    }
    for constant in constants {
        let full_name = format!("{}.{}", env.namespaces.main().name, constant.name);
        let config = env.config.objects.get(&full_name);
        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }
        let (comment, mut type_) = match ffi_type(env, constant.typ, &constant.c_type) {
            Ok(x) => ("", x),
            x @ Err(..) => ("//", x.into_string()),
        };
        let mut value = constant.value.clone();
        if type_ == "*mut c_char" {
            type_ = "*const c_char".into();
            value = format!(
                "b\"{}\\0\" as *const u8 as *const c_char",
                general::escape_string(&value)
            );
        } else if type_ == "gboolean" {
            let prefix = if env.config.library_name == "GLib" {
                ""
            } else {
                "glib::"
            };
            if value == "true" {
                value = format!("{}GTRUE", prefix);
            } else {
                value = format!("{}GFALSE", prefix);
            }
        }

        if env.library
            .type_(constant.typ)
            .maybe_ref_as::<Bitfield>()
            .is_some()
        {
            try!(writeln!(
                w,
                "{}pub const {}: {} = {2} {{ bits: {} }};",
                comment,
                constant.c_identifier,
                type_,
                value
            ));
        } else {
            try!(writeln!(
                w,
                "{}pub const {}: {} = {};",
                comment,
                constant.c_identifier,
                type_,
                value
            ));
        }
    }
    if !constants.is_empty() {
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_enums(w: &mut Write, env: &Env, items: &[&Enumeration]) -> Result<()> {
    if !items.is_empty() {
        try!(writeln!(w, "// Enums"));
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);
        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }
        if item.members.len() == 1 {
            try!(writeln!(w, "pub type {} = c_int;", item.name));
            try!(writeln!(
                w,
                "pub const {}: {} = {};",
                item.members[0].c_identifier,
                item.name,
                item.members[0].value
            ));
            try!(writeln!(w, "pub type {} = {};", item.c_type, item.name));
            try!(writeln!(w, ""));
            continue;
        }
        let mut vals: HashMap<String, (String, Option<Version>)> = HashMap::new();
        try!(writeln!(w, "pub type {} = c_int;", item.c_type));
        for member in &item.members {
            let member_config = config
                .as_ref()
                .map(|c| c.members.matched(&member.name))
                .unwrap_or_else(|| vec![]);
            let is_alias = member_config.iter().any(|m| m.alias);
            let version = member_config.iter().filter_map(|m| m.version).next();

            if is_alias || vals.get(&member.value).is_some() {
                continue;
            }

            try!(version_condition(w, env, version, false, 0));
            try!(writeln!(
                w,
                "pub const {}: {} = {};",
                member.c_identifier,
                item.c_type,
                member.value,
            ));
            vals.insert(member.value.clone(), (member.name.clone(), version));
        }
        try!(writeln!(w, ""));
    }

    Ok(())
}

fn generate_unions(w: &mut Write, env: &Env, unions: &[&Union]) -> Result<()> {
    if !unions.is_empty() {
        try!(writeln!(w, "// Unions"));
    }
    for union in unions {
        if union.c_type.is_none() {
            continue;
        }
        let full_name = format!("{}.{}", env.namespaces.main().name, union.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        let fields = fields::from_union(env, union);
        try!(generate_from_fields(w, &fields));
    }
    Ok(())
}

fn generate_debug_impl(w: &mut Write, name: &str, impl_content: &str) -> Result<()> {
    writeln!(
        w,
        "impl ::std::fmt::Debug for {} {{\n\
            \tfn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {{\n\
                \t\t{}\n\
            \t}}\n\
        }}\n",
        name,
        impl_content)
}

fn generate_classes_structs(w: &mut Write, env: &Env, classes: &[&Class]) -> Result<()> {
    if !classes.is_empty() {
        try!(writeln!(w, "// Classes"));
    }
    for class in classes {
        let full_name = format!("{}.{}", env.namespaces.main().name, class.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        let fields = fields::from_class(env, class);
        try!(generate_from_fields(w, &fields));
    }
    Ok(())
}

fn generate_interfaces_structs(w: &mut Write, env: &Env, interfaces: &[&Interface]) -> Result<()> {
    if !interfaces.is_empty() {
        try!(writeln!(w, "// Interfaces"));
    }
    for interface in interfaces {
        let full_name = format!("{}.{}", env.namespaces.main().name, interface.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        try!(writeln!(
            w,
            "#[repr(C)]\npub struct {}(c_void);\n",
            interface.c_type
        ));
        try!(generate_debug_impl(
            w,
            &interface.c_type,
            &format!("write!(f, \"{name} @ {{:?}}\", self as *const _)",
                     name=interface.c_type)
        ));
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
        let full_name = format!("{}.{}", env.namespaces.main().name, record.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        if record.c_type == "GHookList" {
            // 1. GHookList is useful.
            // 2. GHookList contains bitfields.
            // 3. Bitfields are unrepresentable in Rust.
            // 4. ...
            // 5. Thus, we use custom generated GHookList.
            //    Hopefully someone will profit from all this.
            try!(generate_ghooklist(w));
        } else if record.disguised {
            try!(generate_disguised(w, record));
        } else {
            let fields = fields::from_record(env, record);
            try!(generate_from_fields(w, &fields));
        }
    }
    Ok(())
}

fn generate_ghooklist(w: &mut Write) -> Result<()> {
    w.write_all(br#"#[repr(C)]
#[derive(Copy, Clone)]
pub struct GHookList {
    pub seq_id: c_ulong,
    #[cfg(any(not(windows), not(target_pointer_width = "64")))]
    pub hook_size_and_setup: gpointer,
    #[cfg(all(windows, target_pointer_width = "64"))]
    pub hook_size_and_setup: c_ulong,
    pub hooks: *mut GHook,
    pub dummy3: gpointer,
    pub finalize_hook: GHookFinalizeFunc,
    pub dummy: [gpointer; 2],
}

impl ::std::fmt::Debug for GHookList {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "GHookList @ {:?}", self as *const _)
    }
}

"#)
}

fn generate_disguised(w: &mut Write, record: &Record) -> Result<()> {
    try!(writeln!(w, "#[repr(C)]"));
    try!(writeln!(w, "pub struct _{name}(c_void);", name=record.c_type));
    try!(writeln!(w, ""));
    try!(writeln!(w, "pub type {name} = *mut _{name};", name=record.c_type));
    writeln!(w, "")
}

fn generate_from_fields(w: &mut Write, fields: &fields::Fields) -> Result<()> {
    try!(writeln!(w, "#[repr(C)]"));
    let traits = fields.derived_traits().join(", ");
    if !traits.is_empty() {
        try!(writeln!(w, "#[derive({traits})]", traits=traits));
    }
    if fields.external {
        // It would be nice to represent those using extern types
        // from RFC 1861, once they are available in stable Rust.
        // https://github.com/rust-lang/rust/issues/43467
        try!(writeln!(w, "pub struct {name}(c_void);", name=&fields.name));
    } else {
        try!(writeln!(w, "pub {kind} {name} {{", kind=fields.kind, name=&fields.name));
        for field in &fields.fields {
            try!(writeln!(w, "\tpub {field_name}: {field_type},",
                          field_name=&field.name,
                          field_type=&field.typ));
        }
        if let Some(ref reason) = fields.truncated {
            try!(writeln!(w, "\t_truncated_record_marker: c_void,"));
            try!(writeln!(w, "\t// {}", reason));
        }
        try!(writeln!(w, "}}"));
    }
    try!(writeln!(w, ""));

    try!(writeln!(w, "impl ::std::fmt::Debug for {name} {{", name=&fields.name));
    try!(writeln!(w, "\tfn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {{"));
    try!(writeln!(w, "\t\tf.debug_struct(&format!(\"{name} @ {{:?}}\", self as *const _))", name=&fields.name));
    for field in fields.fields.iter().filter(|f| f.debug) {
        // TODO: We should generate debug for field manually if automatic one is not available.
        try!(writeln!(w, "\t\t .field(\"{field_name}\", {field_get})",
                      field_name=&field.name,
                      field_get=&field.access_str()));
    }
    try!(writeln!(w, "\t\t .finish()"));
    try!(writeln!(w, "\t}}"));
    try!(writeln!(w, "}}"));
    writeln!(w, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_extern_crate_string() {
        let lib = ExternalLibrary {
            namespace: "Gdk".to_owned(),
            crate_name: "gdk".to_owned(),
        };
        assert_eq!(
            get_extern_crate_string(&lib),
            "extern crate gdk_sys as gdk;\n".to_owned()
        );

        let lib = ExternalLibrary {
            namespace: "GdkPixbuf".to_owned(),
            crate_name: "gdk_pixbuf".to_owned(),
        };
        assert_eq!(
            get_extern_crate_string(&lib),
            "extern crate gdk_pixbuf_sys as gdk_pixbuf;\n".to_owned()
        );

        let lib = ExternalLibrary {
            namespace: "GdkPixbuf".to_owned(),
            crate_name: "some-crate".to_owned(),
        };
        assert_eq!(
            get_extern_crate_string(&lib),
            "extern crate some_crate_sys as gdk_pixbuf;\n".to_owned()
        );
    }
}
