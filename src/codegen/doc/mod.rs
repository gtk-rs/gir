use std::io::{Result, Write};

use analysis::namespaces::MAIN;
use env::Env;
use file_saver::save_to_file;
use library::*;
use library::Type as LType;
use writer::primitives;

use stripper_interface as stripper;
use stripper_interface::Type as SType;
use stripper_interface::TypeStruct;
use stripper_interface::{FILE, /*FILE_COMMENT, */MOD_COMMENT};

pub trait ToStripperType {
    fn convert(&self) -> stripper::TypeStruct;
    fn doc(&self) -> Option<String>;
    fn doc_deprecated(&self) -> Option<String>;
}

macro_rules! impl_to_stripper_type {
    ($ty:ident, $enum_var:ident) => {
        impl ToStripperType for $ty {
            fn convert(&self) -> TypeStruct {
                TypeStruct::new(SType::$enum_var, &self.name)
            }

            fn doc(&self) -> Option<String> {
                self.doc.clone()
            }

            fn doc_deprecated(&self) -> Option<String> {
                self.doc_deprecated.clone()
            }
        }
    }
}

impl_to_stripper_type!(Alias, Struct);
impl_to_stripper_type!(Constant, Const);
impl_to_stripper_type!(Member, Variant);
impl_to_stripper_type!(Enumeration, Enum);
impl_to_stripper_type!(Bitfield, Type);
impl_to_stripper_type!(Record, Type);
impl_to_stripper_type!(Field, Variant);
impl_to_stripper_type!(Union, Struct);
impl_to_stripper_type!(Function, Fn);
impl_to_stripper_type!(Interface, Trait);
impl_to_stripper_type!(Class, Struct);
impl_to_stripper_type!(Namespace, Mod);

const DEPRECATED : &'static str = "/// ====>> DEPRECATED <<====";

pub fn generate(env: &Env) {
    let path =  env.config.target_path.join("doc.cmts");
    println!("Generating documentation {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_doc(w, &env.library));
}

fn generate_doc(mut w: &mut Write, lib: &Library) -> Result<()> {
    let namespace = lib.namespace(MAIN);

    /*if let Some(ref namespace_doc) = namespace.doc {
        for line in namespace_doc.split("\n") {
            output.push_str(&format!("{}{}\n", FILE_COMMENT, line));
        }
    }
    try!(create_sub_docs(&mut output, &namespace.constants, None));*/
    for ty in namespace.types.iter().filter_map(|t| t.as_ref()) {
        try!(handle_type(&mut w, &ty, None, lib));
    }
    Ok(())
}

fn handle_type(w: &mut Write, ty: &LType, parent: Option<Box<TypeStruct>>, lib: &Library) -> Result<()> {
    match *ty {
        LType::Alias(ref a) => {
            try!(writeln!(w, "{}src/auto/{}.rs", FILE, a.name.to_lowercase()));
            create_sub_doc(w, a, parent)
        },
        LType::Enumeration(ref e) => create_enum_doc(w, &e, parent),
        LType::Function(ref f) => create_fn_doc(w, &f, parent),
        LType::Interface(ref i) => create_trait_doc(w, &i, parent),
        LType::Class(ref c) => create_class_doc(w, &c, parent, lib),
        _ => Ok(()),
    }
}

fn create_class_doc(w: &mut Write, class: &Class, parent: Option<Box<TypeStruct>>, lib: &Library) -> Result<()> {
    let tabs : String = primitives::tabs(compute_ident(&parent));
    let mut ty = class.convert();
    ty.parent = parent;

    try!(writeln!(w, "{}src/auto/{}.rs", FILE, class.name.to_lowercase()));
    let found = if let Some(ref class_doc) = class.doc {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
        try!(write_lines(w, &class_doc, &tabs));
        true
    } else {
        false
    };

    if let Some(ref class_doc) = class.doc_deprecated {
        if found == false {
            try!(writeln!(w, "{}{}",
                          MOD_COMMENT,
                          ty));
        }
        try!(writeln!(w, "{}{}", tabs, DEPRECATED));
        try!(write_lines(w, &class_doc, &tabs));
    }
    for function in class.functions.iter() {
        try!(create_fn_doc(w, &function, Some(Box::new(ty.clone()))));
    }
    for child_id in class.children.iter() {
        if let &LType::Class(ref child) = lib.type_(*child_id) {
            try!(create_class_doc(w, &child, None, lib));
        }
    }
    Ok(())
}

fn create_trait_doc(w: &mut Write, trait_: &Interface, parent: Option<Box<TypeStruct>>) -> Result<()> {
    let tabs : String = primitives::tabs(compute_ident(&parent));
    let mut ty = trait_.convert();
    ty.parent = parent;
    let found = if let Some(ref trait_doc) = trait_.doc {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
        try!(write_lines(w, &trait_doc, &tabs));
        true
    } else {
        false
    };

    if let Some(ref trait_doc) = trait_.doc_deprecated {
        if found == false {
            try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
        }
        try!(writeln!(w, "{}{}", tabs, DEPRECATED));
        try!(write_lines(w, &trait_doc, &tabs));
    }
    for function in trait_.functions.iter() {
        try!(create_fn_doc(w, &function, Some(Box::new(ty.clone()))));
    }
    Ok(())
}

fn create_enum_doc(w: &mut Write, enum_: &Enumeration, parent: Option<Box<TypeStruct>>) -> Result<()> {
    let ident = compute_ident(&parent);
    let mut ty = enum_.convert();
    ty.parent = parent;
    let tabs : String = primitives::tabs(ident);

    let found = if let Some(ref enum_doc) = enum_.doc {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
        try!(write_lines(w, &enum_doc, &tabs));
        true
    } else {
        false
    };

    if let Some(ref enum_doc) = enum_.doc_deprecated {
        if found == false {
            try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
        }
        try!(writeln!(w, "{}{}", tabs, DEPRECATED));
        try!(write_lines(w, &enum_doc, &tabs));
    }
    let tabs : String = primitives::tabs(ident + 1);
    for member_doc in enum_.members.iter() {
        let mut sub_ty : TypeStruct = member_doc.convert();

        if let Some(ref m_doc) = member_doc.doc {
            sub_ty.parent = Some(Box::new(ty.clone()));
            try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));
            try!(write_lines(w, &m_doc, &tabs));
        }
    }
    Ok(())
}

fn create_fn_doc(w: &mut Write, fn_: &Function, parent: Option<Box<TypeStruct>>) -> Result<()> {
    let tabs : String = primitives::tabs(compute_ident(&parent) + 1);

    let mut docs = match fn_.doc {
        Some(ref d) => format!("{}/// {}\n", tabs, d.split("\n").collect::<Vec<&str>>().join(&format!("\n{}/// ", tabs))),
        None    => String::new(),
    };
    if let Some(ref fn_doc) = fn_.doc_deprecated {
        docs.push_str(&format!("{}{}\n{}/// {}", tabs, DEPRECATED, tabs, fn_doc.split("\n").collect::<Vec<&str>>().join(&format!("\n{}/// ", tabs))));
    }

    for parameter in fn_.parameters.iter() {
        if let Some(ref parameter_doc) = parameter.doc {
            add_line_to_doc(&mut docs);
            docs.push_str(&format!("{}/// {}\n", tabs, parameter_doc.split("\n").collect::<Vec<&str>>().join("\n/// ")));
        }
        if let Some(ref parameter_doc) = parameter.doc_deprecated {
            add_line_to_doc(&mut docs);
            docs.push_str(&format!("{}{}\n{}/// {}\n", tabs, DEPRECATED, tabs, parameter_doc.split("\n").collect::<Vec<&str>>().join("\n/// ")));
        }
    }
    if let Some(ref doc) = fn_.ret.doc {
        add_line_to_doc(&mut docs);
        docs.push_str(&format!("{}/// {}\n", tabs, doc.split("\n").collect::<Vec<&str>>().join("\n/// ")));
    }
    if let Some(ref doc) = fn_.ret.doc_deprecated {
        add_line_to_doc(&mut docs);
        docs.push_str(&format!("{}{}\n{}/// {}\n", tabs, DEPRECATED, tabs, doc.split("\n").collect::<Vec<&str>>().join("\n/// ")));
    }
    if docs.len() > 0 {
        let mut sub_ty = fn_.convert();
        sub_ty.parent = parent;
        try!(write!(w, "{}{}\n{}", MOD_COMMENT, sub_ty, &docs));
    }
    Ok(())
}

fn write_lines(w: &mut Write, lines: &str, tabs: &str) -> Result<()> {
    for line in lines.split("\n") {
        try!(writeln!(w, "{}{}", tabs, line));
    }
    Ok(())
}

fn add_line_to_doc(doc: &mut String) {
    if doc.len() > 1 {
        doc.push_str("\n");
    }
}

/*fn create_sub_docs<T: library::ToStripperType>(w: &mut Write, tys: &[T], parent: Option<Box<TypeStruct>>) -> Result<()> {
    for ty in tys {
        try!(create_sub_doc(w, ty, parent.clone()));
    }
    Ok(())
}*/

fn create_sub_doc<T: ToStripperType>(w: &mut Write, ty: &T, parent: Option<Box<TypeStruct>>) -> Result<()> {
    let tabs : String = primitives::tabs(compute_ident(&parent));
    let mut sub_ty = ty.convert();
    sub_ty.parent = parent;

    if let Some(doc) = ty.doc() {
        try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));
        for line in doc.split("\n") {
            try!(writeln!(w, "{}/// {}", tabs, line));
        }
    }
    if let Some(doc) = ty.doc_deprecated() {
        try!(writeln!(w, "{}{}\n{}{}", MOD_COMMENT, sub_ty, tabs, DEPRECATED));
        for line in doc.split("\n") {
            try!(writeln!(w, "{}/// {}", tabs, line));
        }
    }
    Ok(())
}

fn compute_ident(e: &Option<Box<TypeStruct>>) -> usize {
    match *e {
        Some(ref e) if e.parent.is_some() => compute_ident(&e.parent) + 1,
        _ => 0,
    }
}
