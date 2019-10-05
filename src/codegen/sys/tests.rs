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
        generate_layout_c(env, &layout_c, w)
    });

    let constant_c = tests.join("constant.c");
    save_to_file(&constant_c, env.config.make_backup, |w| {
        generate_constant_c(env, &constant_c, w)
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
    let name = match t.get_glib_name() {
        None => return None,
        Some(name) => name,
    };
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

fn generate_layout_c(env: &Env, path: &Path, w: &mut dyn Write) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;
    writeln!(w, "#include \"manual.h\"")?;
    writeln!(w, "#include <stdalign.h>")?;
    writeln!(w, "#include <stdio.h>")?;

    writeln!(
        w,
        "{}",
        r##"
int main() {
    printf("%zu\n%zu", sizeof(ABI_TYPE_NAME), alignof(ABI_TYPE_NAME));
    return 0;
}"##
    )
}

fn generate_constant_c(env: &Env, path: &Path, w: &mut dyn Write) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;
    writeln!(w, "#include \"manual.h\"")?;
    writeln!(w, "#include <stdio.h>")?;

    writeln!(
        w,
        "{}",
        r####"
int main() {
    printf(_Generic((ABI_CONSTANT_NAME),
                    char *: "###gir test###%s###gir test###\n",
                    const char *: "###gir test###%s###gir test###\n",
                    char: "###gir test###%c###gir test###\n",
                    signed char: "###gir test###%hhd###gir test###\n",
                    unsigned char: "###gir test###%hhu###gir test###\n",
                    short int: "###gir test###%hd###gir test###\n",
                    unsigned short int: "###gir test###%hu###gir test###\n",
                    int: "###gir test###%d###gir test###\n",
                    unsigned int: "###gir test###%u###gir test###\n",
                    long: "###gir test###%ld###gir test###\n",
                    unsigned long: "###gir test###%lu###gir test###\n",
                    long long: "###gir test###%lld###gir test###\n",
                    unsigned long long: "###gir test###%llu###gir test###\n",
                    double: "###gir test###%f###gir test###\n",
                    long double: "###gir test###%ld###gir test###\n"),
           ABI_CONSTANT_NAME);
    return 0;
}"####
    )
}

fn generate_abi_rs(
    env: &Env,
    path: &Path,
    w: &mut dyn Write,
    crate_name: &str,
    ctypes: &[CType],
    cconsts: &[CConstant],
) -> io::Result<()> {
    let ns = env.library.namespace(MAIN_NAMESPACE);
    let package_name = ns.package_name.as_ref().expect("Missing package name");

    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w)?;

    writeln!(w, "extern crate {};", crate_name)?;
    writeln!(w, "extern crate shell_words;")?;
    writeln!(w, "extern crate tempdir;")?;
    writeln!(w, "use std::env;")?;
    writeln!(w, "use std::error::Error;")?;
    writeln!(w, "use std::path::Path;")?;
    writeln!(w, "use std::mem::{{align_of, size_of}};")?;
    writeln!(w, "use std::process::Command;")?;
    writeln!(w, "use std::str;")?;
    writeln!(w, "use {}::*;\n", crate_name)?;
    writeln!(w, "static PACKAGES: &[&str] = &[\"{}\"];", package_name)?;
    writeln!(
        w,
        "{}",
        r####"
#[derive(Clone, Debug)]
struct Compiler {
    pub args: Vec<String>,
}

impl Compiler {
    pub fn new() -> Result<Compiler, Box<dyn Error>> {
        let mut args = get_var("CC", "cc")?;
        args.push("-Wno-deprecated-declarations".to_owned());
        // For %z support in printf when using MinGW.
        args.push("-D__USE_MINGW_ANSI_STDIO".to_owned());
        args.extend(get_var("CFLAGS", "")?);
        args.extend(get_var("CPPFLAGS", "")?);
        args.extend(pkg_config_cflags(PACKAGES)?);
        Ok(Compiler { args })
    }

    pub fn define<'a, V: Into<Option<&'a str>>>(&mut self, var: &str, val: V) {
        let arg = match val.into() {
            None => format!("-D{}", var),
            Some(val) => format!("-D{}={}", var, val),
        };
        self.args.push(arg);
    }

    pub fn compile(&self, src: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
        let mut cmd = self.to_command();
        cmd.arg(src);
        cmd.arg("-o");
        cmd.arg(out);
        let status = cmd.spawn()?.wait()?;
        if !status.success() {
            return Err(format!("compilation command {:?} failed, {}",
                               &cmd, status).into());
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
        Err(err) => Err(format!("{} {}", name, err).into()),
    }
}

fn pkg_config_cflags(packages: &[&str]) -> Result<Vec<String>, Box<dyn Error>> {
    if packages.is_empty() {
        return Ok(Vec::new());
    }
    let mut cmd = Command::new("pkg-config");
    cmd.arg("--cflags");
    cmd.args(packages);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(format!("command {:?} returned {}",
                           &cmd, out.status).into());
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
    /// Number of tests that failed to compile.
    failed_to_compile: usize,
}

impl Results {
    fn record_passed(&mut self) {
        self.passed += 1;
    }
    fn record_failed(&mut self) {
        self.failed += 1;
    }
    fn record_failed_to_compile(&mut self) {
        self.failed += 1;
        self.failed_to_compile += 1;
    }
    fn summary(&self) -> String {
        format!(
            "{} passed; {} failed (compilation errors: {})",
            self.passed,
            self.failed,
            self.failed_to_compile)
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
    let tmpdir = tempdir::TempDir::new("abi").expect("temporary directory");
    let cc = Compiler::new().expect("configured compiler");

    assert_eq!("1",
               get_c_value(tmpdir.path(), &cc, "1").expect("C constant"),
               "failed to obtain correct constant value for 1");

    let mut results : Results = Default::default();
    for (i, &(name, rust_value)) in RUST_CONSTANTS.iter().enumerate() {
        match get_c_value(tmpdir.path(), &cc, name) {
            Err(e) => {
                results.record_failed_to_compile();
                eprintln!("{}", e);
            },
            Ok(ref c_value) => {
                if rust_value == c_value {
                    results.record_passed();
                } else {
                    results.record_failed();
                    eprintln!("Constant value mismatch for {}\nRust: {:?}\nC:    {:?}",
                              name, rust_value, c_value);
                }
            }
        };
        if (i + 1) % 25 == 0 {
            println!("constants ... {}", results.summary());
        }
    }
    results.expect_total_success();
}

#[test]
fn cross_validate_layout_with_c() {
    let tmpdir = tempdir::TempDir::new("abi").expect("temporary directory");
    let cc = Compiler::new().expect("configured compiler");

    assert_eq!(Layout {size: 1, alignment: 1},
               get_c_layout(tmpdir.path(), &cc, "char").expect("C layout"),
               "failed to obtain correct layout for char type");

    let mut results : Results = Default::default();
    for (i, &(name, rust_layout)) in RUST_LAYOUTS.iter().enumerate() {
        match get_c_layout(tmpdir.path(), &cc, name) {
            Err(e) => {
                results.record_failed_to_compile();
                eprintln!("{}", e);
            },
            Ok(c_layout) => {
                if rust_layout == c_layout {
                    results.record_passed();
                } else {
                    results.record_failed();
                    eprintln!("Layout mismatch for {}\nRust: {:?}\nC:    {:?}",
                              name, rust_layout, &c_layout);
                }
            }
        };
        if (i + 1) % 25 == 0 {
            println!("layout    ... {}", results.summary());
        }
    }
    results.expect_total_success();
}

fn get_c_layout(dir: &Path, cc: &Compiler, name: &str) -> Result<Layout, Box<dyn Error>> {
    let exe = dir.join("layout");
    let mut cc = cc.clone();
    cc.define("ABI_TYPE_NAME", name);
    cc.compile(Path::new("tests/layout.c"), &exe)?;

    let mut abi_cmd = Command::new(exe);
    let output = abi_cmd.output()?;
    if !output.status.success() {
        return Err(format!("command {:?} failed, {:?}",
                           &abi_cmd, &output).into());
    }

    let stdout = str::from_utf8(&output.stdout)?;
    let mut words = stdout.trim().split_whitespace();
    let size = words.next().unwrap().parse().unwrap();
    let alignment = words.next().unwrap().parse().unwrap();
    Ok(Layout {size, alignment})
}

fn get_c_value(dir: &Path, cc: &Compiler, name: &str) -> Result<String, Box<dyn Error>> {
    let exe = dir.join("constant");
    let mut cc = cc.clone();
    cc.define("ABI_CONSTANT_NAME", name);
    cc.compile(Path::new("tests/constant.c"), &exe)?;

    let mut abi_cmd = Command::new(exe);
    let output = abi_cmd.output()?;
    if !output.status.success() {
        return Err(format!("command {:?} failed, {:?}",
                           &abi_cmd, &output).into());
    }

    let output = str::from_utf8(&output.stdout)?.trim();
    if !output.starts_with("###gir test###") ||
       !output.ends_with("###gir test###") {
        return Err(format!("command {:?} return invalid output, {:?}",
                           &abi_cmd, &output).into());
    }

    Ok(String::from(&output[14..(output.len() - 14)]))
}

const RUST_LAYOUTS: &[(&str, Layout)] = &["####
    )?;
    for ctype in ctypes {
        general::cfg_condition(w, &ctype.cfg_condition, false, 1)?;
        writeln!(w, "\t(\"{ctype}\", Layout {{size: size_of::<{ctype}>(), alignment: align_of::<{ctype}>()}}),",
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
            "\t(\"{name}\", \"{value}\"),",
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
