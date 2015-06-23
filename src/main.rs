extern crate docopt;
extern crate xml;
extern crate toml;

use chunk::*;
use file_saver::*;
use library::*;

mod chunk;
mod config;
mod file_saver;
mod gobjects;
mod library;
mod parser;

fn main() {
    let cfg = config::Config::new();

    let v = vec!["ф1", "t2", "тестs3"];
    let c = v.into_chunks();
    let tmp =  c.into_iter().save_to_file("a.txt");

    let mut library = Library::new();
    library.read_file(&cfg.girs_dir, &cfg.library_name);
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
