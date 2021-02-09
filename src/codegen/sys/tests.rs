use crate::{
    analysis::types::IsIncomplete,
    codegen::general,
    env::Env,
    file_saver::save_to_file,
    library::{self, Bitfield, Enumeration, Namespace, Type, MAIN_NAMESPACE},
};
use log::info;
use std::{
    io::{self, prelude::*},
    path::Path,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CType {
    /// Name of type, as used in C.
    name: String,
    /// Expression describing when type is available (when defined only conditionally).
    cfg_condition: Option<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CConstant {
    /// Identifier in C.
    name: String,
    /// Stringified value.
    value: String,
}

pub fn generate(env: &Env, crate_name: &str) -> bool {
    let ctypes = prepare_ctypes(env);
    let cconsts = prepare_cconsts(env);

    if ctypes.is_empty() && cconsts.is_empty() {
        return false;
    }

    let tests = env.config.target_path.join("tests");

    let manual_h = tests.join("manual.h");
    if !manual_h.exists() {
        save_to_file(&manual_h, env.config.make_backup, |w| {
            generate_manual_h(env, &manual_h, w)
        });
    }

    let layout_c = tests.join("layout.c");
    save_to_file(&layout_c, env.config.make_backup, |w| {
        generate_layout_c(env, &layout_c, w, &ctypes)
    });

    let constant_c = tests.join("constant.c");
    save_to_file(&constant_c, env.config.make_backup, |w| {
        generate_constant_c(env, &constant_c, w, &cconsts)
    });

    let abi_rs = tests.join("abi.rs");
    save_to_file(&abi_rs, env.config.make_backup, |w| {
        generate_abi_rs(env, &abi_rs, w, crate_name, &ctypes, &cconsts)
    });

    true
}

fn prepare_ctypes(env: &Env) -> Vec<CType> {
    let ns = env.library.namespace(MAIN_NAMESPACE);
    let mut types: Vec<CType> = ns
        .types
        .iter()
        .filter_map(Option::as_ref)
        .filter(|t| !t.is_incomplete(&env.library))
        .filter_map(|t| match *t {
            Type::Record(library::Record { disguised, .. }) if !disguised => {
                prepare_ctype(env, ns, t)
            }
            Type::Alias(_)
            | Type::Class(_)
            | Type::Union(_)
            | Type::Enumeration(_)
            | Type::Bitfield(_)
            | Type::Interface(_) => prepare_ctype(env, ns, t),
            _ => None,
        })
        .collect();

    types.sort();
    types
}

fn prepare_ctype(env: &Env, ns: &Namespace, t: &Type) -> Option<CType> {
    let full_name = format!("{}.{}", ns.name, t.get_name());
    if env.type_status_sys(&full_name).ignored() {
        return None;
    }
    let name = t.get_glib_name()?;

    if is_name_made_up(name) {
        return None;
    }
    let cfg_condition = env
        .config
        .objects
        .get(&full_name)
        .and_then(|obj| obj.cfg_condition.clone());
    Some(CType {
        name: name.to_owned(),
        cfg_condition,
    })
}

fn prepare_cconsts(env: &Env) -> Vec<CConstant> {
    let ns = env.library.namespace(MAIN_NAMESPACE);
    let mut constants: Vec<CConstant> = ns
        .constants
        .iter()
        .filter_map(|constant| {
            let full_name = format!("{}.{}", &ns.name, constant.name);
            if env.type_status_sys(&full_name).ignored() {
                return None;
            }
            let value = match constant {
                c if c.c_type == "gboolean" && c.value == "true" => "1",
                c if c.c_type == "gboolean" && c.value == "false" => "0",
                c => &c.value,
            };
            Some(CConstant {
                name: constant.c_identifier.clone(),
                value: value.to_owned(),
            })
        })
        .collect();

    for typ in &ns.types {
        let typ = if let Some(ref typ) = *typ {
            typ
        } else {
            continue;
        };
        let full_name = format!("{}.{}", &ns.name, typ.get_name());
        if env.type_status_sys(&full_name).ignored() {
            continue;
        }
        match *typ {
            Type::Bitfield(Bitfield { ref members, .. }) => {
                for member in members {
                    // GLib assumes that bitflags are unsigned integers,
                    // see the GValue machinery around them for example
                    constants.push(CConstant {
                        name: format!("(guint) {}", member.c_identifier),
                        value: member.value.clone(),
                    });
                }
            }
            Type::Enumeration(Enumeration { ref members, .. }) => {
                for member in members {
                    // GLib assumes that enums are signed integers,
                    // see the GValue machinery around them for example
                    constants.push(CConstant {
                        name: format!("(gint) {}", member.c_identifier),
                        value: member.value.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    constants.sort_by(|a, b| {
        fn strip_cast(x: &CConstant) -> &str {
            if x.name.starts_with("(gint) ") {
                &x.name[7..]
            } else if x.name.starts_with("(guint) ") {
                &x.name[8..]
            } else {
                x.name.as_str()
            }
        }

        strip_cast(a).cmp(&strip_cast(b))
    });
    constants
}

/// Checks if type name is unlikely to correspond to a real C type name.
fn is_name_made_up(name: &str) -> bool {
    // Unnamed types are assigned name during parsing, those names contain an underscore.
    name.contains('_')
}

fn generate_manual_h(env: &Env, path: &Path, w: &mut dyn Write) -> io::Result<()> {
    info!("Generating file {:?}", path);
    writeln!(
        w,
        "// Feel free to edit this file, it won't be regenerated by gir generator unless removed."
    )?;
    writeln!(w)?;

    let ns = env.library.namespace(MAIN_NAMESPACE);
    for include in &ns.c_includes {
        writeln!(w, "#include <{}>", include)?;
    }

    Ok(())
}

#[allow(clippy::write_literal)]
fn generate_layout_c(
    env: &Env,
    path: &Path,
    w: &mut dyn Write,
    ctypes: &[CType],
) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;
    writeln!(w, "#include \"manual.h\"")?;
    writeln!(w, "#include <stdalign.h>")?;
    writeln!(
        w,
        "{}",
        r####"
typedef struct {
    const char *name;
    size_t size;
    size_t alignent;
} Layout;

const Layout LAYOUTS[] = {"####
    )?;

    let n = ctypes.len();
    for (i, ctype) in ctypes.iter().enumerate() {
        write!(w, "{}", "    { ")?;
        write!(
            w,
            "\"{ctype}\", sizeof({ctype}), alignof({ctype})",
            ctype = ctype.name
        )?;

        if i == n - 1 {
            writeln!(w, "{}", " }")?;
        } else {
            writeln!(w, "{}", " },")?;
        }
    }

    writeln!(
        w,
        "{}",
        r####"};

const Layout *c_layouts(size_t *n) {
    *n = sizeof(LAYOUTS) / sizeof(Layout);
    return LAYOUTS;
}"####
    )
}

#[allow(clippy::write_literal)]
fn generate_constant_c(
    env: &Env,
    path: &Path,
    w: &mut dyn Write,
    cconsts: &[CConstant],
) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;
    writeln!(w, "#include \"manual.h\"")?;
    writeln!(w, "#include <glib.h>")?;
    writeln!(
        w,
        "{}",
        r####"
#define FORMAT_CONSTANT(CONSTANT_NAME) \
    _Generic((CONSTANT_NAME), \
        char *: "%s", \
        const char *: "%s", \
        signed char: "%hhd", \
        unsigned char: "%hhu", \
        short int: "%hd", \
        unsigned short int: "%hu", \
        int: "%d", \
        unsigned int: "%u", \
        long: "%ld", \
        unsigned long: "%lu", \
        long long: "%lld", \
        unsigned long long: "%llu", \
        double: "%f", \
        long double: "%ld")

typedef struct {
    char *name;
    char *value;
} Constant;

Constant *c_constants(size_t *n) {"####
    )?;
    writeln!(w, "    *n = {};", cconsts.len())?;

    // We are leaking this, but for a test it does not matter
    writeln!(w, "{}", "    Constant *res = g_new0(Constant, *n);")?;

    for (i, cconst) in cconsts.iter().enumerate() {
        writeln!(
            w,
            "    res[{index}].name = g_strdup(\"{name}\");",
            index = i,
            name = cconst.name
        )?;
        writeln!(
            w,
            "    res[{index}].value = g_strdup_printf(FORMAT_CONSTANT({name}), {name});",
            index = i,
            name = cconst.name,
        )?;
    }

    writeln!(w, "{}", "    return res;")?;
    writeln!(w, "{}", "}")?;

    writeln!(
        w,
        "{}",
        r####"
void c_constants_free(Constant *constants, size_t n) {
    size_t i;
    for (i = 0; i < n; i++) {
        g_free(constants[i].name);
        g_free(constants[i].value);
    }
    g_free(constants);
}"####
    )
}

#[allow(clippy::write_literal)]
fn generate_abi_rs(
    env: &Env,
    path: &Path,
    w: &mut dyn Write,
    crate_name: &str,
    ctypes: &[CType],
    cconsts: &[CConstant],
) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;

    writeln!(w, "use std::mem::{{align_of, size_of}};")?;
    writeln!(w, "use std::str;")?;
    writeln!(w, "use {}::*;", crate_name)?;
    writeln!(
        w,
        "{}",
        r####"
mod c_abi {
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[repr(C)]
    pub struct CConstant {
        pub name: *const libc::c_char,
        pub value: *const libc::c_char,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[repr(C)]
    pub struct CLayout {
        pub name: *const libc::c_char,
        pub size: usize,
        pub alignment: usize,
    }

    extern "C" {
        pub fn c_constants(n: *mut usize) -> *mut CConstant;
        pub fn c_constants_free(c: *mut CConstant, n: usize);
        pub fn c_layouts(n: *mut usize) -> *const CLayout;
    }
}

fn c_constants() -> Vec<(String, String)> {
    let mut res: Vec<(String, String)> = Vec::new();

    unsafe {
        let mut n = 0;
        let p = c_abi::c_constants(&mut n);
        let constants = std::slice::from_raw_parts(p, n);

        for c in constants {
            let c_name = std::ffi::CStr::from_ptr(c.name);
            let c_value = std::ffi::CStr::from_ptr(c.value);
            res.push((c_name.to_str().unwrap().to_owned(), c_value.to_str().unwrap().to_owned()));
        }

        c_abi::c_constants_free(p, n);
    }

    res
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Layout {
    size: usize,
    alignment: usize,
}

fn c_layouts() -> Vec<(String, Layout)> {
    let mut res = Vec::new();

    unsafe {
        let mut n = 0;
        let p = c_abi::c_layouts(&mut n);
        let layouts = std::slice::from_raw_parts(p, n);

        for l in layouts {
            let c_name = std::ffi::CStr::from_ptr(l.name);
            let name = c_name.to_str().unwrap().to_owned();
            let size = l.size;
            let alignment = l.alignment;
            res.push((name, Layout { size, alignment }));
        }
    }

    res
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct Results {
    /// Number of successfully completed tests.
    passed: usize,
    /// Total number of failed tests (including those that failed to compile).
    failed: usize,
}

impl Results {
    fn record_passed(&mut self) {
        self.passed += 1;
    }
    fn record_failed(&mut self) {
        self.failed += 1;
    }
    fn summary(&self) -> String {
        format!("{} passed; {} failed", self.passed, self.failed)
    }
    fn expect_total_success(&self) {
        if self.failed == 0 {
            println!("OK: {}", self.summary());
        } else {
            panic!("FAILED: {}", self.summary());
        };
    }
}

#[test]
fn cross_validate_constants_with_c() {
    let mut results = Results::default();

    for ((rust_name, rust_value), (c_name, c_value)) in
        RUST_CONSTANTS.iter().zip(c_constants().iter())
    {
        if rust_name != c_name {
            results.record_failed();
            eprintln!("Name mismatch:\nRust: {:?}\nC:    {:?}", rust_name, c_name,);
            continue;
        }

        if rust_value != c_value {
            results.record_failed();
            eprintln!(
                "Constant value mismatch for {}\nRust: {:?}\nC:    {:?}",
                rust_name, rust_value, &c_value
            );
            continue;
        }

        results.record_passed();
    }

    results.expect_total_success();
}

#[test]
fn cross_validate_layout_with_c() {
    let mut results = Results::default();

    for ((rust_name, rust_layout), (c_name, c_layout)) in
        RUST_LAYOUTS.iter().zip(c_layouts().iter())
    {
        if rust_name != c_name {
            results.record_failed();
            eprintln!("Name mismatch:\nRust: {:?}\nC:    {:?}", rust_name, c_name,);
            continue;
        }

        if rust_layout != c_layout {
            results.record_failed();
            eprintln!(
                "Layout mismatch for {}\nRust: {:?}\nC:    {:?}",
                rust_name, rust_layout, &c_layout
            );
            continue;
        }

        results.record_passed();
    }

    results.expect_total_success();
}

const RUST_LAYOUTS: &[(&str, Layout)] = &["####
    )?;
    for ctype in ctypes {
        general::cfg_condition(w, &ctype.cfg_condition, false, 1)?;
        writeln!(w, "    (\"{ctype}\", Layout {{size: size_of::<{ctype}>(), alignment: align_of::<{ctype}>()}}),",
                 ctype=ctype.name)?;
    }
    writeln!(
        w,
        "{}",
        r##"];

const RUST_CONSTANTS: &[(&str, &str)] = &["##
    )?;
    for cconst in cconsts {
        writeln!(
            w,
            "    (\"{name}\", \"{value}\"),",
            name = cconst.name,
            value = &general::escape_string(&cconst.value)
        )?;
    }
    writeln!(
        w,
        "{}",
        r##"];

"##
    )
}
