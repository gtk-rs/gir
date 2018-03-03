use std::io::prelude::*;
use std::io;
use std::path::Path;

use analysis::types::IsIncomplete;
use env::Env;
use file_saver::save_to_file;
use library::{Type, MAIN_NAMESPACE};
use nameutil::crate_name;

pub fn generate(env :&Env) {
    let tests = env.config.target_path.join("tests");
    let abi_c = tests.join("abi.c");
    let abi_rs = tests.join("abi.rs");

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
                if !env.type_status_sys(&full_name).ignored() {
                    t.get_glib_name()
                } else {
                    None
                }
            },
            _ => None,
        })
        .filter(|s| !is_name_made_up(s))
        .collect::<Vec<_>>();

    if ctypes.is_empty() {
        return;
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

    writeln!(w, "/* For %z support in printf when using MinGW. */")?;
    writeln!(w, "#define _POSIX_C_SOURCE 200809L")?;
    writeln!(w, "#include <stdalign.h>")?;
    writeln!(w, "#include <stdio.h>")?;

    let ns = env.library.namespace(MAIN_NAMESPACE);
    for include in ns.c_includes.iter() {
        writeln!(w, "#include <{}>", include)?;
    }

    writeln!(w, "{}", r##"
int main() {
  printf("%zu\n%zu\n", sizeof(ABI_TEST_TYPE), alignof(ABI_TEST_TYPE));
}"##)

}

fn generate_abi_rs(env: &Env, path: &Path, w: &mut Write, ctypes: &[&str]) -> io::Result<()> {
    info!("Generating file {:?}", path);

    let name = format!("{}_sys", crate_name(&env.config.library_name));
    writeln!(w, "extern crate {};", &name)?;
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
        Ok(value) => Ok(shell_split(&value)),
        Err(env::VarError::NotPresent) => Ok(shell_split(default)),
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
    Ok(shell_split(stdout))
}

fn shell_split(s: &str) -> Vec<String> {
    // FIXME: Use shell word splitting rules.
    s.split_whitespace()
        .map(|s| s.to_owned())
        .collect()
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
               get_c_abi(&cc, "char").expect("C ABI for char"));

    let rust = get_rust_abi();
    let mut failed = false;
    for (name, rust_abi) in &rust {
        match get_c_abi(&cc, name) {
            Err(e) => {
                failed = true;
                eprintln!("{}", e);
                continue;
            },
            Ok(ref c_abi) => {
                if rust_abi != c_abi {
                    failed = true;
                    eprintln!("ABI mismatch for {}\nRust: {:?}\nC:    {:?}",
                              name, rust_abi, c_abi);
                }
            }
        };
    }
    assert!(!failed);
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
        return Err(format!("command {:?} failed, {}",
                           &abi_cmd, output.status).into());
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
        writeln!(w, r##"    abi.insert("{ctype}".to_owned(), ABI::from_type::<{ctype}>());"##,
                 ctype=ctype)?;
    }

    writeln!(w, "{}", r##"    abi
}

"##)

}


