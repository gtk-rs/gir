extern crate docopt;
extern crate xml;
extern crate toml;

use std::fs::File;
use std::io::prelude::*;
use docopt::Docopt;
use library::*;

mod gobjects;
mod library;
mod parser;

static USAGE: &'static str = "
Usage: gir [-d <girs_dir>] [<library>]

Options:
    -d PATH            Directory for girs
";

fn main() {
    let args = Docopt::new(USAGE).unwrap()
        .parse().unwrap_or_else(|e| e.exit());

    let toml = read_toml("Gir.toml");

    let girs_dir = match args.get_str("-d") {
        "" => toml.lookup("options.girs_dir")
                  .unwrap_or_else(|| panic!("No options.girs_dir in config"))
                  .as_str().unwrap(),
        a => a
    };

    let library_name = match args.get_str("<library>") {
        "" => toml.lookup("options.library")
                .unwrap_or_else(|| panic!("No options.library in config"))
                .as_str().unwrap(),
        a => a
    };

    let objects = gobjects::parse_toml(toml.lookup("object").unwrap());
    for object in objects.values() { println!("Objects: {:?}", object); }

    let mut library = Library::new();
    library.read_file(girs_dir, library_name);
    library.check_resolved();
    show(&library);
}

#[allow(dead_code)]
fn show(library: &Library) {
    for namespace in &library.namespaces {
        println!("Namespace: {}", namespace);
        let prefix = format!("{}.", namespace);
        for (ref name, ref typ) in &library.types {
            if !name.starts_with(&prefix) {
                continue;
            }
            match *typ.borrow() {
                Type::Class(ref x) => println!("\tclass {}", x.name),
                Type::Record(ref x) => println!("\trecord {}", x.name),
                Type::Union(ref x) => println!("\tunion {}", x.name),
                Type::Interface(ref x) => println!("\tinterface {}", x.name),
                Type::Callback(ref x) => println!("\tcallback {}", x.name),
                Type::Bitfield(ref x) => println!("\tbitfield {}", x.name),
                Type::Enumeration(ref x) => println!("\tenumeration {}", x.name),
                _ => println!("\t{} ???", name),
            }
        }
        for (ref name, ref c) in &library.constants {
            if !name.starts_with(&prefix) {
                continue;
            }
            println!("\tconst {} = {}", c.name, c.value);
        }
        for (ref name, ref f) in &library.functions {
            if !name.starts_with(&prefix) {
                continue;
            }
            println!("\tfunction {}", f.name);
        }
    }
}

fn read_toml(filename: &str) -> toml::Value {
    let mut input = String::new();
    File::open(filename).and_then(|mut f| {
        f.read_to_string(&mut input)
    }).unwrap();
    let mut parser = toml::Parser::new(&input);
    match parser.parse() {
        Some(toml) => toml::Value::Table(toml),
        None => {
            for err in &parser.errors {
                let (loline, locol) = parser.to_linecol(err.lo);
                let (hiline, hicol) = parser.to_linecol(err.hi);
                println!("{}:{}:{}-{}:{} error: {}",
                         filename, loline, locol, hiline, hicol, err.desc);
            }
            panic!("Errors in config")
        }
    }
}
