use std::io::prelude::*;
use std::io;
use std::path::Path;

use analysis::types::IsIncomplete;
use env::Env;
use file_saver::save_to_file;
use library::{Type, MAIN_NAMESPACE};
use nameutil::crate_name;
use codegen::general;

struct CType {
    /// Name of type, as used in C.
    name: String,
    /// Expression describing when type is available (when defined only conditionally).
    cfg_condition: Option<String>,
}

pub fn generate(env :&Env) {
    let tests = env.config.target_path.join("tests");
    let abi_c = tests.join("abi.c");
    let abi_rs = tests.join("abi.rs");
    let manual_h = tests.join("manual.h");

    let ns = env.library.namespace(MAIN_NAMESPACE);
    let ctypes = ns.types
        .iter()
        .filter_map(|t| t.as_ref())
        .filter(|t| !t.is_incomplete(&env.library))
        .filter_map(|t| match *t {
            Type::Alias(_) |
            Type::Class(_) |
            Type::Record(_) |
            Type::Union(_) |
            Type::Enumeration(_) |
            Type::Bitfield(_) |
            Type::Interface(_)
            => {
                let full_name = format!("{}.{}", &ns.name, t.get_name());
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
                let cfg_condition = env.config.objects.get(&full_name).and_then(|obj| {
                    obj.cfg_condition.clone()
                });
                Some(CType {
                    name: name.to_owned(),
                    cfg_condition,
                })
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    if ctypes.is_empty() {
        return;
    }

    if !manual_h.exists() {
        save_to_file(&manual_h, env.config.make_backup, |w| {
            info!("Generating file {:?}", &manual_h);
            writeln!(w, "// Insert manual customizations here, they won't be overwritten.")
        });
    }
    save_to_file(&abi_c, env.config.make_backup, |w| {
        generate_abi_c(env, &abi_c, w)
    });
    save_to_file(&abi_rs, env.config.make_backup, |w| {
        generate_abi_rs(env, &abi_rs, w, &ctypes)
    });
}

/// Checks if type name is unlikely to correspond to a real C type name.
fn is_name_made_up(name: &str) -> bool {
    // Unnamed types are assigned name during parsing, those names contain an underscore.
    name.contains('_')
}

fn generate_abi_c(env: &Env, path: &Path, w: &mut Write) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w, "")?;
    writeln!(w, "/* For %z support in printf when using MinGW. */")?;
    writeln!(w, "#define _POSIX_C_SOURCE 200809L")?;
    writeln!(w, "#include <stdalign.h>")?;
    writeln!(w, "#include <stdio.h>")?;
    writeln!(w, "#include \"manual.h\"")?;

    let ns = env.library.namespace(MAIN_NAMESPACE);
    for include in ns.c_includes.iter() {
        writeln!(w, "#include <{}>", include)?;
    }

    writeln!(w, "{}", r##"
int main() {
  printf("%zu\n%zu\n", sizeof(ABI_TEST_TYPE), alignof(ABI_TEST_TYPE));
}"##)

}

fn generate_abi_rs(env: &Env, path: &Path, w: &mut Write, ctypes: &[CType]) -> io::Result<()> {
    info!("Generating file {:?}", path);
    general::start_comments(w, &env.config)?;
    writeln!(w, "")?;
    let name = format!("{}_sys", crate_name(&env.config.library_name));
    writeln!(w, "extern crate {};", &name)?;
    writeln!(w, "extern crate shell_words;")?;
    writeln!(w, "{}", r##"
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::mem::{align_of, size_of};
use std::process::Command;
use std::str;"##)?;
    writeln!(w, "use {}::*;", &name)?;
    writeln!(w, "{}", r##"

#[derive(Clone, Debug)]
struct Compiler {
    pub args: Vec<String>,
}

impl Compiler {
    pub fn new() -> Result<Compiler, Box<Error>> {
        let mut args = get_var("CC", "cc")?;
        args.extend(get_var("CFLAGS", "")?);
        args.extend(get_var("CPPFLAGS", "")?);
        Ok(Compiler { args })
    }

    pub fn define<'a, V: Into<Option<&'a str>>>(&mut self, var: &str, val: V) {
        let arg = match val.into() {
            None => format!("-D{}", var), 
            Some(val) => format!("-D{}={}", var, val),
        };
        self.args.push(arg);
    }

    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.args[0]);
        cmd.args(&self.args[1..]);
        cmd
    }
}

fn get_var(name: &str, default: &str) -> Result<Vec<String>, Box<Error>> {
    match env::var(name) {
        Ok(value) => Ok(shell_words::split(&value)?),
        Err(env::VarError::NotPresent) => Ok(shell_words::split(default)?),
        Err(err) => Err(format!("{} {}", name, err).into()),
    }
}

fn pkg_config_cflags(package: &str) -> Result<Vec<String>, Box<Error>> {
    let mut cmd = Command::new("pkg-config");
    cmd.arg("--cflags");
    cmd.arg(package);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(format!("command {:?} returned {}", 
                           &cmd, out.status).into());
    }
    let stdout = str::from_utf8(&out.stdout)?;
    Ok(shell_words::split(stdout)?)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ABI {
    size: usize,
    alignment: usize,
}

impl ABI {
    pub fn from_type<T: Sized>() -> ABI {
        ABI {
            size: size_of::<T>(),
            alignment: align_of::<T>(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct TestResults {
    /// Number of successfully completed tests.
    passed: usize,
    /// Total number of failed tests (including those that failed to compile).
    failed: usize,
    /// Number of tests that failed to compile.
    failed_compile: usize,
}

#[test]
fn test_cross_validate_abi_with_c() {
    // Configure compiler instance."##)?;

    let ns = env.library.namespace(MAIN_NAMESPACE);
    let package_name = ns.package_name.as_ref()
        .expect("Missing package name");

    writeln!(w, "\tlet package_name = \"{}\";", package_name)?;
    writeln!(w, "{}", r##"    let mut cc = Compiler::new()
        .expect("compiler from environment");
    let cflags = pkg_config_cflags(package_name)
        .expect("cflags from pkg-config");
    cc.args.extend(cflags);
    cc.args.push("-Wno-deprecated-declarations".to_owned());

    // Sanity check that compilation works.
    assert_eq!(ABI {size: 1, alignment: 1},
               get_c_abi(&cc, "char").expect("C ABI for char"),
               "failed to obtain correct ABI for char type");

    let mut results : TestResults = Default::default();
    for (name, rust_abi) in &get_rust_abi() {
        match get_c_abi(&cc, name) {
            Err(e) => {
                results.failed += 1;
                results.failed_compile += 1;
                eprintln!("{}", e);
                continue;
            },
            Ok(ref c_abi) => {
                if rust_abi == c_abi {
                    results.passed += 1;
                } else {
                    results.failed += 1;
                    eprintln!("ABI mismatch for {}\nRust: {:?}\nC:    {:?}",
                              name, rust_abi, c_abi);
                }
            }
        };
    }

    if results.failed + results.failed_compile != 0 {
        panic!("FAILED\nABI test results: {} passed; {} failed (compilation errors: {})",
               results.passed,
               results.failed,
               results.failed_compile);
    }
}

fn get_c_abi(cc: &Compiler, name: &str) -> Result<ABI, Box<Error>> {
    let mut cc = cc.clone();
    cc.define("ABI_TEST_TYPE", name);
    cc.args.push("tests/abi.c".to_owned());
    cc.args.push("-oabi".to_owned());

    let mut cc_cmd = cc.to_command();
    let status = cc_cmd.spawn()?.wait()?;
    if !status.success() {
        return Err(format!("compilation command {:?} failed, {}",
                           &cc_cmd, status).into());
    }

    let mut abi_cmd = Command::new("./abi");
    let output = abi_cmd.output()?;
    if !output.status.success() {
        return Err(format!("command {:?} failed, {:?}",
                           &abi_cmd, &output).into());
    }

    let stdout = str::from_utf8(&output.stdout)?;
    let mut words = stdout.split_whitespace();
    let size = words.next().unwrap().parse().unwrap();
    let alignment = words.next().unwrap().parse().unwrap();
    Ok(ABI {size, alignment})
}

fn get_rust_abi() -> BTreeMap<String, ABI> {
    let mut abi = BTreeMap::new();"##)?;
    
    for ctype in ctypes {
        general::cfg_condition(w, &ctype.cfg_condition, false, 1)?;
        writeln!(w, r##"    abi.insert("{ctype}".to_owned(), ABI::from_type::<{ctype}>());"##,
                 ctype=ctype.name)?;
    }

    writeln!(w, "{}", r##"    abi
}
"##)

}
