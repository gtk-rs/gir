use std::{
    fs,
    io::{Error, ErrorKind, Result, Write},
};

use log::info;

use super::{ffi_type::ffi_type, fields, functions, statics};
use crate::{
    codegen::general::{self, cfg_condition, version_condition},
    config::constants,
    env::Env,
    file_saver::*,
    library::*,
    nameutil::*,
    traits::*,
};

pub fn generate(env: &Env) {
    info!("Generating sys for {}", env.config.library_name);

    let path = env.config.auto_path.join(file_name_sys("lib"));

    info!("Generating file {:?}", path);
    save_to_file(&path, env.config.make_backup, |w| generate_lib(w, env));
}

fn write_link_attr(w: &mut dyn Write, shared_libs: &[String]) -> Result<()> {
    for it in shared_libs {
        writeln!(
            w,
            "#[link(name = \"{}\")]",
            shared_lib_name_to_link_name(it)
        )?;
    }

    Ok(())
}

fn generate_lib(w: &mut dyn Write, env: &Env) -> Result<()> {
    general::start_comments(w, &env.config)?;
    statics::begin(w)?;

    include_custom_modules(w, env)?;
    statics::after_extern_crates(w)?;

    if env.config.library_name != "GLib" {
        statics::use_glib(w)?;
    }
    match &*env.config.library_name {
        "GLib" => statics::only_for_glib(w)?,
        "GObject" => statics::only_for_gobject(w)?,
        "Gtk" => statics::only_for_gtk(w)?,
        _ => (),
    }
    writeln!(w)?;

    let ns = env.library.namespace(MAIN_NAMESPACE);
    let records = prepare(ns);
    let classes = prepare(ns);
    let interfaces = prepare(ns);
    let bitfields = prepare(ns);
    let enums = prepare(ns);
    let unions = prepare(ns);

    generate_aliases(w, env, &prepare(ns))?;
    generate_enums(w, env, &enums)?;
    generate_constants(w, env, &ns.constants)?;
    generate_bitfields(w, env, &bitfields)?;
    generate_unions(w, env, &unions)?;
    functions::generate_callbacks(w, env, &prepare(ns))?;
    generate_records(w, env, &records)?;
    generate_classes_structs(w, env, &classes)?;
    generate_interfaces_structs(w, env, &interfaces)?;

    if env.namespaces.main().shared_libs.is_empty()
        && !(records.iter().all(|x| x.functions.is_empty())
            && classes.iter().all(|x| x.functions.is_empty())
            && interfaces.iter().all(|x| x.functions.is_empty())
            && bitfields.iter().all(|x| x.functions.is_empty())
            && enums.iter().all(|x| x.functions.is_empty())
            && unions.iter().all(|x| x.functions.is_empty()))
    {
        return Err(Error::new(
            ErrorKind::Other,
            "No shared library found, but functions were found",
        ));
    }

    if !env.namespaces.main().shared_libs.is_empty() {
        write_link_attr(w, &env.namespaces.main().shared_libs)?;
        writeln!(w, "extern \"C\" {{")?;
        functions::generate_enums_funcs(w, env, &enums)?;
        functions::generate_bitfields_funcs(w, env, &bitfields)?;
        functions::generate_unions_funcs(w, env, &unions)?;
        functions::generate_records_funcs(w, env, &records)?;
        functions::generate_classes_funcs(w, env, &classes)?;
        functions::generate_interfaces_funcs(w, env, &interfaces)?;
        functions::generate_other_funcs(w, env, &ns.functions)?;

        writeln!(w, "\n}}")?;
    }

    Ok(())
}

fn include_custom_modules(w: &mut dyn Write, env: &Env) -> Result<()> {
    let modules = find_modules(env)?;
    if !modules.is_empty() {
        writeln!(w)?;
        for module in &modules {
            writeln!(w, "mod {module};")?;
        }
        writeln!(w)?;
        for module in &modules {
            writeln!(w, "pub use {module}::*;")?;
        }
    }

    Ok(())
}

fn find_modules(env: &Env) -> Result<Vec<String>> {
    let mut vec = Vec::<String>::new();
    for entry in fs::read_dir(&env.config.auto_path)? {
        let path = entry?.path();
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
    for typ in ns.types.iter().filter_map(Option::as_ref) {
        if let Some(x) = typ.maybe_ref() {
            vec.push(x);
        }
    }
    vec.sort();
    vec
}

fn generate_aliases(w: &mut dyn Write, env: &Env, items: &[&Alias]) -> Result<()> {
    if !items.is_empty() {
        writeln!(w, "// Aliases")?;
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        let (comment, c_type) = match ffi_type(env, item.typ, &item.target_c_type) {
            Ok(x) => ("", x.into_string()),
            x @ Err(..) => ("//", x.into_string()),
        };
        writeln!(w, "{}pub type {} = {};", comment, item.c_identifier, c_type)?;
    }
    if !items.is_empty() {
        writeln!(w)?;
    }

    Ok(())
}

fn generate_bitfields(w: &mut dyn Write, env: &Env, items: &[&Bitfield]) -> Result<()> {
    if !items.is_empty() {
        writeln!(w, "// Flags")?;
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);
        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }
        writeln!(w, "pub type {} = c_uint;", item.c_type)?;
        for member in &item.members {
            let member_config = config
                .as_ref()
                .map_or_else(Vec::new, |c| c.members.matched(&member.name));
            let version = member_config
                .iter()
                .find_map(|m| m.version)
                .or(member.version);

            let val: i64 = member.value.parse().unwrap();

            version_condition(w, env, None, version, false, 0)?;
            writeln!(
                w,
                "pub const {}: {} = {};",
                member.c_identifier, item.c_type, val as u32,
            )?;
        }
        writeln!(w)?;
    }

    Ok(())
}

fn generate_constant_cfg_configure(
    w: &mut dyn Write,
    configured_constants: &[&constants::Constant],
    commented: bool,
) -> Result<()> {
    let cfg_condition_ = configured_constants
        .iter()
        .find_map(|f| f.cfg_condition.as_ref());
    cfg_condition(w, cfg_condition_, commented, 1)?;
    Ok(())
}

fn generate_constants(w: &mut dyn Write, env: &Env, constants: &[Constant]) -> Result<()> {
    if !constants.is_empty() {
        writeln!(w, "// Constants")?;
    }
    for constant in constants {
        let full_name = format!("{}.{}", env.namespaces.main().name, constant.name);
        let config = env.config.objects.get(&full_name);
        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }
        let (comment, mut type_) = match ffi_type(env, constant.typ, &constant.c_type) {
            Ok(x) => ("", x.into_string()),
            x @ Err(..) => ("//", x.into_string()),
        };
        let mut value = constant.value.clone();
        if type_ == "*mut c_char" {
            type_ = "&[u8]".into();
            value = format!("b\"{}\\0\"", general::escape_string(&value));
        } else if type_ == "gboolean" {
            value = if value == "true" {
                use_glib_if_needed(env, "GTRUE")
            } else {
                use_glib_if_needed(env, "GFALSE")
            };
        } else if env
            .library
            .type_(constant.typ)
            .maybe_ref_as::<Bitfield>()
            .is_some()
        {
            let val: i64 = constant.value.parse().unwrap();
            value = (val as u32).to_string();
        }

        if let Some(obj) = config {
            let configured_constants = obj.constants.matched(&full_name);
            generate_constant_cfg_configure(w, &configured_constants, !comment.is_empty())?;
        }

        writeln!(
            w,
            "{}pub const {}: {} = {};",
            comment, constant.c_identifier, type_, value
        )?;
    }
    if !constants.is_empty() {
        writeln!(w)?;
    }

    Ok(())
}

fn generate_enums(w: &mut dyn Write, env: &Env, items: &[&Enumeration]) -> Result<()> {
    if !items.is_empty() {
        writeln!(w, "// Enums")?;
    }
    for item in items {
        let full_name = format!("{}.{}", env.namespaces.main().name, item.name);
        let config = env.config.objects.get(&full_name);
        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }
        writeln!(w, "pub type {} = c_int;", item.c_type)?;
        for member in &item.members {
            let member_config = config
                .as_ref()
                .map_or_else(Vec::new, |c| c.members.matched(&member.name));
            let is_alias = member_config.iter().any(|m| m.alias);
            let version = member_config
                .iter()
                .find_map(|m| m.version)
                .or(member.version);

            if is_alias {
                continue;
            }

            version_condition(w, env, None, version, false, 0)?;
            writeln!(
                w,
                "pub const {}: {} = {};",
                member.c_identifier, item.c_type, member.value,
            )?;
        }
        writeln!(w)?;
    }

    Ok(())
}

fn generate_unions(w: &mut dyn Write, env: &Env, unions: &[&Union]) -> Result<()> {
    if !unions.is_empty() {
        writeln!(w, "// Unions")?;
    }
    for union in unions {
        if union.c_type.is_none() {
            continue;
        }
        let full_name = format!("{}.{}", env.namespaces.main().name, union.name);
        let config = env.config.objects.get(&full_name);

        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }

        let align = config.and_then(|c| c.align);
        let fields = fields::from_union(env, union);
        generate_from_fields(w, &fields, align)?;
    }
    Ok(())
}

fn generate_debug_impl(w: &mut dyn Write, name: &str, impl_content: &str) -> Result<()> {
    writeln!(
        w,
        "impl ::std::fmt::Debug for {name} {{\n\
         \tfn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {{\n\
         \t\t{impl_content}\n\
         \t}}\n\
         }}\n"
    )
}

fn generate_classes_structs(w: &mut dyn Write, env: &Env, classes: &[&Class]) -> Result<()> {
    if !classes.is_empty() {
        writeln!(w, "// Classes")?;
    }
    for class in classes {
        let full_name = format!("{}.{}", env.namespaces.main().name, class.name);
        let config = env.config.objects.get(&full_name);

        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }

        let align = config.and_then(|c| c.align);
        let fields = fields::from_class(env, class);
        generate_from_fields(w, &fields, align)?;
    }
    Ok(())
}

fn generate_opaque_type(w: &mut dyn Write, name: &str) -> Result<()> {
    writeln!(
        w,
        r#"#[repr(C)]
pub struct {name} {{
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}}
"#
    )
}

fn generate_interfaces_structs(
    w: &mut dyn Write,
    env: &Env,
    interfaces: &[&Interface],
) -> Result<()> {
    if !interfaces.is_empty() {
        writeln!(w, "// Interfaces")?;
    }
    for interface in interfaces {
        let full_name = format!("{}.{}", env.namespaces.main().name, interface.name);
        if !env.type_status_sys(&full_name).need_generate() {
            continue;
        }
        generate_opaque_type(w, &interface.c_type)?;
        generate_debug_impl(
            w,
            &interface.c_type,
            &format!(
                "write!(f, \"{name} @ {{self:p}}\")",
                name = interface.c_type
            ),
        )?;
    }
    if !interfaces.is_empty() {
        writeln!(w)?;
    }

    Ok(())
}

fn generate_records(w: &mut dyn Write, env: &Env, records: &[&Record]) -> Result<()> {
    if !records.is_empty() {
        writeln!(w, "// Records")?;
    }
    for record in records {
        let full_name = format!("{}.{}", env.namespaces.main().name, record.name);
        let config = env.config.objects.get(&full_name);

        if let Some(false) = config.map(|c| c.status.need_generate()) {
            continue;
        }

        if record.c_type == "GHookList" {
            // 1. GHookList is useful.
            // 2. GHookList contains bitfields.
            // 3. Bitfields are unrepresentable in Rust.
            // 4. ...
            // 5. Thus, we use custom generated GHookList.
            //    Hopefully someone will profit from all this.
            generate_ghooklist(w)?;
        } else if record.disguised {
            generate_disguised(w, record)?;
        } else {
            let align = config.and_then(|c| c.align);
            let fields = fields::from_record(env, record);
            generate_from_fields(w, &fields, align)?;
        }
    }
    Ok(())
}

fn generate_ghooklist(w: &mut dyn Write) -> Result<()> {
    w.write_all(
        br#"#[repr(C)]
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
        write!(f, "GHookList @ {self:p}")
    }
}

"#,
    )
}

fn generate_disguised(w: &mut dyn Write, record: &Record) -> Result<()> {
    generate_opaque_type(w, &format!("_{}", record.c_type))?;
    writeln!(w, "pub type {name} = *mut _{name};", name = record.c_type)?;
    writeln!(w)
}

fn generate_from_fields(
    w: &mut dyn Write,
    fields: &fields::Fields,
    align: Option<u32>,
) -> Result<()> {
    cfg_condition(w, fields.cfg_condition.as_ref(), false, 0)?;
    if let Some(align) = align {
        writeln!(w, "#[repr(align({align}))]")?;
    }
    let traits = fields.derived_traits().join(", ");
    if !traits.is_empty() {
        writeln!(w, "#[derive({traits})]")?;
    }
    if fields.external {
        // It would be nice to represent those using extern types
        // from RFC 1861, once they are available in stable Rust.
        // https://github.com/rust-lang/rust/issues/43467
        generate_opaque_type(w, &fields.name)?;
    } else {
        writeln!(w, "#[repr(C)]")?;
        writeln!(
            w,
            "pub {kind} {name} {{",
            kind = fields.kind,
            name = &fields.name
        )?;
        for field in &fields.fields {
            writeln!(
                w,
                "\tpub {field_name}: {field_type},",
                field_name = &field.name,
                field_type = &field.typ
            )?;
        }
        if let Some(ref reason) = fields.truncated {
            writeln!(w, "\t_truncated_record_marker: c_void,")?;
            writeln!(w, "\t// {reason}")?;
        }
        writeln!(w, "}}\n")?;
    }

    cfg_condition(w, fields.cfg_condition.as_ref(), false, 0)?;
    writeln!(
        w,
        "impl ::std::fmt::Debug for {name} {{",
        name = &fields.name
    )?;
    writeln!(
        w,
        "\tfn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {{"
    )?;
    writeln!(
        w,
        "\t\tf.debug_struct(&format!(\"{name} @ {{self:p}}\"))",
        name = &fields.name
    )?;
    for field in fields.fields.iter().filter(|f| f.debug) {
        // TODO: We should generate debug for field manually if automatic one is not
        // available.
        writeln!(
            w,
            "\t\t .field(\"{field_name}\", {field_get})",
            field_name = &field.name,
            field_get = &field.access_str()
        )?;
    }
    writeln!(w, "\t\t .finish()")?;
    writeln!(w, "\t}}")?;
    writeln!(w, "}}")?;
    writeln!(w)
}
