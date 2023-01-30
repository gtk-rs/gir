use std::{
    io::{self, prelude::*},
    path::Path,
};

use log::info;

use crate::{
    analysis::types::IsIncomplete,
    codegen::general,
    env::Env,
    file_saver::save_to_file,
    library::{self, Bitfield, Enumeration, Namespace, Type, MAIN_NAMESPACE},
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CType {
    /// Name of type, as used in C.
    name: String,
    /// Expression describing when type is available (when defined only
    /// conditionally).
    cfg_condition: Option<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CConstant {
    /// Identifier in C.
    name: String,
    /// Stringified value.
    value: String,
}

pub fn generate(env: &Env, crate_name: &str) {
    let ctypes = prepare_ctypes(env);
    let cconsts = prepare_cconsts(env);

    if ctypes.is_empty() && cconsts.is_empty() {
        return;
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
}

fn prepare_ctypes(env: &Env) -> Vec<CType> {
    let ns = env.library.namespace(MAIN_NAMESPACE);
    let mut types: Vec<CType> = ns
        .types
        .iter()
        .filter_map(Option::as_ref)
        .filter(|t| !t.is_incomplete(&env.library))
        .filter_map(|t| match t {
            Type::Record(library::Record {
                disguised: false, ..
            }) => prepare_ctype(env, ns, t),
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
        let typ = if let Some(typ) = typ {
            typ
        } else {
            continue;
        };
        let full_name = format!("{}.{}", &ns.name, typ.get_name());
        if env.type_status_sys(&full_name).ignored() {
            continue;
        }
        match typ {
            Type::Bitfield(Bitfield { members, .. }) => {
                for member in members {
                    // GLib assumes that bitflags are unsigned integers,
                    // see the GValue machinery around them for example
                    constants.push(CConstant {
                        name: format!("(guint) {}", member.c_identifier),
                        value: member
                            .value
                            .parse::<i32>()
                            .map(|i| (i as u32).to_string())
                            .unwrap_or_else(|_| member.value.clone()),
                    });
                }
            }
            Type::Enumeration(Enumeration { members, .. }) => {
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

        strip_cast(a).cmp(strip_cast(b))
    });
    constants
}

/// Checks if type name is unlikely to correspond to a real C type name.
fn is_name_made_up(name: &str) -> bool {
    // Unnamed types are assigned name during parsing, those names contain an
    // underscore.
    name.contains('_') && !name.ends_with("_t")
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
        writeln!(w, "#include <{include}>")?;
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
    writeln!(w, "#include <stdio.h>")?;
    writeln!(w)?;
    writeln!(w, "{}", r"int main() {")?;

    for ctype in ctypes {
        writeln!(
            w,
            "    printf(\"%s;%zu;%zu\\n\", \"{ctype}\", sizeof({ctype}), alignof({ctype}));",
            ctype = ctype.name
        )?;
    }

    writeln!(w, "    return 0;")?;
    writeln!(w, "{}", r"}")
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
    writeln!(w, "#include <stdio.h>")?;
    writeln!(
        w,
        "{}",
        r####"
#define PRINT_CONSTANT(CONSTANT_NAME) \
    printf("%s;", #CONSTANT_NAME); \
    printf(_Generic((CONSTANT_NAME), \
                    char *: "%s", \
                    const char *: "%s", \
                    char: "%c", \
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
                    float: "%f", \
                    double: "%f", \
                    long double: "%ld"), \
           CONSTANT_NAME); \
    printf("\n");
"####
    )?;

    writeln!(w, "{}", r"int main() {")?;

    for cconst in cconsts {
        writeln!(w, "    PRINT_CONSTANT({name});", name = cconst.name,)?;
    }

    writeln!(w, "    return 0;")?;
    writeln!(w, "{}", r"}")
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
    let ns = env.library.namespace(MAIN_NAMESPACE);
    let mut package_names = ns.package_names.join("\", \"");
    if !package_names.is_empty() {
        package_names = format!("\"{package_names}\"");
    }

    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;
    writeln!(w, "#![cfg(target_os = \"linux\")]")?;
    writeln!(w)?;

    if !ctypes.is_empty() {
        writeln!(w, "use {crate_name}::*;")?;
        writeln!(w, "use std::mem::{{align_of, size_of}};")?;
    }

    writeln!(w, "use std::env;")?;
    writeln!(w, "use std::error::Error;")?;
    writeln!(w, "use std::ffi::OsString;")?;
    writeln!(w, "use std::path::Path;")?;
    writeln!(w, "use std::process::Command;")?;
    writeln!(w, "use std::str;")?;
    writeln!(w, "use tempfile::Builder;")?;
    writeln!(w)?;
    writeln!(w, "static PACKAGES: &[&str] = &[{package_names}];")?;
    writeln!(
        w,
        "{}",
        r####"
#[derive(Clone, Debug)]
struct Compiler {
    pub args: Vec<String>,
}

impl Compiler {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut args = get_var("CC", "cc")?;
        args.push("-Wno-deprecated-declarations".to_owned());
        // For _Generic
        args.push("-std=c11".to_owned());
        // For %z support in printf when using MinGW.
        args.push("-D__USE_MINGW_ANSI_STDIO".to_owned());
        args.extend(get_var("CFLAGS", "")?);
        args.extend(get_var("CPPFLAGS", "")?);
        args.extend(pkg_config_cflags(PACKAGES)?);
        Ok(Self { args })
    }

    pub fn compile(&self, src: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
        let mut cmd = self.to_command();
        cmd.arg(src);
        cmd.arg("-o");
        cmd.arg(out);
        let status = cmd.spawn()?.wait()?;
        if !status.success() {
            return Err(format!("compilation command {cmd:?} failed, {status}").into());
        }
        Ok(())
    }

    fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.args[0]);
        cmd.args(&self.args[1..]);
        cmd
    }
}

fn get_var(name: &str, default: &str) -> Result<Vec<String>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => Ok(shell_words::split(&value)?),
        Err(env::VarError::NotPresent) => Ok(shell_words::split(default)?),
        Err(err) => Err(format!("{name} {err}").into()),
    }
}

fn pkg_config_cflags(packages: &[&str]) -> Result<Vec<String>, Box<dyn Error>> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let pkg_config = env::var_os("PKG_CONFIG")
        .unwrap_or_else(|| OsString::from("pkg-config"));
    let mut cmd = Command::new(pkg_config);
    cmd.arg("--cflags");
    cmd.args(packages);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(format!("command {cmd:?} returned {}", out.status).into());
    }
    let stdout = str::from_utf8(&out.stdout)?;
    Ok(shell_words::split(stdout.trim())?)
}


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Layout {
    size: usize,
    alignment: usize,
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
    let mut c_constants: Vec<(String, String)> = Vec::new();

    for l in get_c_output("constant").unwrap().lines() {
        let (name, value) = l.split_once(';').expect("Missing ';' separator");
        c_constants.push((name.to_owned(), value.to_owned()));
    }

    let mut results = Results::default();

    for ((rust_name, rust_value), (c_name, c_value)) in
        RUST_CONSTANTS.iter().zip(c_constants.iter())
    {
        if rust_name != c_name {
            results.record_failed();
            eprintln!("Name mismatch:\nRust: {rust_name:?}\nC:    {c_name:?}");
            continue;
        }

        if rust_value != c_value {
            results.record_failed();
            eprintln!(
                "Constant value mismatch for {rust_name}\nRust: {rust_value:?}\nC:    {c_value:?}",
            );
            continue;
        }

        results.record_passed();
    }

    results.expect_total_success();
}

#[test]
fn cross_validate_layout_with_c() {
    let mut c_layouts = Vec::new();

    for l in get_c_output("layout").unwrap().lines() {
        let (name, value) = l.split_once(';').expect("Missing first ';' separator");
        let (size, alignment) = value.split_once(';').expect("Missing second ';' separator");
        let size = size.parse().expect("Failed to parse size");
        let alignment = alignment.parse().expect("Failed to parse alignment");
        c_layouts.push((name.to_owned(), Layout { size, alignment }));
    }

    let mut results = Results::default();

    for ((rust_name, rust_layout), (c_name, c_layout)) in
        RUST_LAYOUTS.iter().zip(c_layouts.iter())
    {
        if rust_name != c_name {
            results.record_failed();
            eprintln!("Name mismatch:\nRust: {rust_name:?}\nC:    {c_name:?}");
            continue;
        }

        if rust_layout != c_layout {
            results.record_failed();
            eprintln!(
                "Layout mismatch for {rust_name}\nRust: {rust_layout:?}\nC:    {c_layout:?}",
            );
            continue;
        }

        results.record_passed();
    }

    results.expect_total_success();
}

fn get_c_output(name: &str) -> Result<String, Box<dyn Error>> {
    let tmpdir = Builder::new().prefix("abi").tempdir()?;
    let exe = tmpdir.path().join(name);
    let c_file = Path::new("tests").join(name).with_extension("c");

    let cc = Compiler::new().expect("configured compiler");
    cc.compile(&c_file, &exe)?;

    let mut abi_cmd = Command::new(exe);
    let output = abi_cmd.output()?;
    if !output.status.success() {
        return Err(format!("command {abi_cmd:?} failed, {output:?}").into());
    }

    Ok(String::from_utf8(output.stdout)?)
}

const RUST_LAYOUTS: &[(&str, Layout)] = &["####
    )?;
    for ctype in ctypes {
        general::cfg_condition(w, ctype.cfg_condition.as_ref(), false, 1)?;
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
