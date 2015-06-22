extern crate docopt;
extern crate xml;

use docopt::Docopt;
use library::*;

mod library;
mod parser;

static USAGE: &'static str = "
Usage: spore -d <dir> <lib>
";

fn main() {
    let args = Docopt::new(USAGE).unwrap()
        .parse().unwrap_or_else(|e| e.exit());
    let mut library = Library::new();
    library.read_file(args.get_str("<dir>"), args.get_str("<lib>"));
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
                Type::Callback(ref x) => println!("\trecord {}", x.name),
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
