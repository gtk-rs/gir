use std::io::{Result, Write};

use analysis;
use analysis::symbols;
use analysis::namespaces::MAIN;
use env::Env;
use file_saver::save_to_file;
use library::*;
use library::Type as LType;
use nameutil;
use regex::{Captures, Regex};
use writer::primitives;

use stripper_interface as stripper;
use stripper_interface::Type as SType;
use stripper_interface::TypeStruct;
use stripper_interface::{FILE, /*FILE_COMMENT, */MOD_COMMENT};

mod format;

trait FunctionTraitType {
    fn functions(&self) -> &Vec<Function>;
    fn name(&self) -> &str;
}

macro_rules! impl_function_trait_type {
    ($ty:ident) => {
        impl FunctionTraitType for $ty {
            fn functions(&self) -> &Vec<Function> {
                &self.functions
            }

            fn name(&self) -> &str {
                &self.name
            }
        }
    }
}

impl_function_trait_type!(Class);
impl_function_trait_type!(Interface);

trait ToStripperType {
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
impl_to_stripper_type!(Parameter, Variant);

pub fn generate(env: &Env) {
    let path =  env.config.target_path.join("doc.cmts");
    println!("Generating documentation {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_doc(w, &env));
}

fn generate_doc(mut w: &mut Write, env: &Env) -> Result<()> {
    try!(writeln!(w, "{}*", FILE));

    let namespace = env.library.namespace(MAIN);
    for obj in env.config.objects.values() {
        if !obj.status.need_generate() {
            continue;
        }

        let info = analysis::object::class(env, obj)
            .or_else(|| analysis::object::interface(env, obj));
        if let Some(info) = info {
            try!(create_object_doc(w, env, &info));
        }
    }

    let symbols = env.symbols.borrow();
    for ty in namespace.types.iter().filter_map(|t| t.as_ref()) {
        try!(handle_type(&mut w, &ty, &symbols));
    }
    Ok(())
}

fn handle_type(w: &mut Write, ty: &LType, symbols: &symbols::Info) -> Result<()> {
    match *ty {
        LType::Alias(ref a) => create_sub_doc(w, a, symbols),
        LType::Enumeration(ref e) => create_enum_doc(w, &e, symbols),
        LType::Function(ref f) => create_fn_doc(w, &f, None, symbols),
        _ => Ok(()),
    }
}

fn create_object_doc(w: &mut Write, env: &Env, info: &analysis::object::Info) -> Result<()> {
    let symbols = env.symbols.borrow();
    let tabs = "";
    let ty = TypeStruct::new(SType::Struct, &info.name);
    let ty_ext = TypeStruct::new(SType::Trait, &format!("{}Ext", info.name));
    let has_trait = info.has_children;
    let class: &ToStripperType;
    let functions: &[Function];
    let implements: Vec<TypeId>;

    match *env.library.type_(info.type_id) {
        Type::Class(ref cl) => {
            class = cl;
            functions = &cl.functions;
            implements = cl.parents.iter()
                .chain(cl.implements.iter())
                .map(|&tid| tid)
                .collect();
        }
        Type::Interface(ref iface) => {
            class = iface;
            functions = &iface.functions;
            implements = iface.prereq_parents.clone();
        }
        _ => unreachable!(),
    }

    try!(writeln!(w, "{}{}", MOD_COMMENT, ty));

    if let Some(ref class_doc) = class.doc() {
        try!(write_lines(w, &class_doc, &tabs, &symbols));
    }

    try!(write_header(w, &tabs, "Implements"));
    let implements = implements.iter()
        .filter(|&tid| !env.type_status(&tid.full_name(&env.library)).ignored())
        .map(|&tid| format!("`{}Ext`", env.library.type_(tid).get_name()))
        .collect::<Vec<_>>();
    try!(write_verbatim(w, &implements.join(", "), &tabs));

    if let Some(ref class_doc) = class.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &class_doc, &tabs, &symbols));
    }

    if has_trait {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty_ext));
        try!(write_verbatim(w, &format!("Trait containing all `{}` methods.", ty.name), &tabs));
    }

    let ty = TypeStruct { ty: SType::Impl, ..ty };

    for function in functions {
        let ty = if has_trait && function.parameters.iter().any(|p| p.instance_parameter) {
            ty_ext.clone()
        }
        else {
            ty.clone()
        };
        try!(create_fn_doc(w, &function, Some(Box::new(ty)), &symbols));
    }
    Ok(())
}

fn create_enum_doc(w: &mut Write, enum_: &Enumeration, symbols: &symbols::Info) -> Result<()> {
    let ty = enum_.convert();
    let tabs = "";

    if enum_.doc().is_some() || enum_.doc_deprecated().is_some() {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
    }

    if let Some(ref enum_doc) = enum_.doc() {
        try!(write_lines(w, &enum_doc, &tabs, symbols));
    }
    if let Some(ref enum_doc) = enum_.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &enum_doc, &tabs, symbols));
    }

    let tabs = "\t";
    for member_doc in enum_.members.iter() {
        let mut sub_ty : TypeStruct = member_doc.convert();

        if member_doc.doc().is_some() || member_doc.doc_deprecated().is_some() {
            sub_ty.parent = Some(Box::new(ty.clone()));
            try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));
        }
        if let Some(ref m_doc) = member_doc.doc() {
            try!(write_lines(w, &m_doc, &tabs, symbols));
        }
        if let Some(ref m_doc) = member_doc.doc_deprecated() {
            try!(write_header(w, &tabs, "Deprecated"));
            try!(write_lines(w, &m_doc, &tabs, symbols));
        }
    }
    Ok(())
}

lazy_static! {
    static ref PARAM_NAME: Regex = Regex::new(r"@(\w+)\b").unwrap();
}

fn fix_param_names(doc: &str, self_name: &Option<String>) -> String {
    PARAM_NAME.replace_all(doc, |caps: &Captures| {
        if let Some(ref self_name) = *self_name {
            if &caps[1] == self_name {
                return "@self".into()
            }
        }
        format!("@{}", nameutil::mangle_keywords(&caps[1]))
    })
}

fn create_fn_doc(w: &mut Write, fn_: &Function, parent: Option<Box<TypeStruct>>,
                 symbols: &symbols::Info) -> Result<()> {
    let tabs : String = primitives::tabs(compute_indent(&parent) + 1);

    if fn_.doc().is_none() && fn_.doc_deprecated().is_none() && fn_.ret.doc().is_none() {
        if fn_.parameters.iter().all(|x| {
            x.doc().is_none()
        }) {
            return Ok(());
        }
    }
    let mut sub_ty = fn_.convert();
    sub_ty.parent = parent;

    let self_name: Option<String> = fn_.parameters.iter()
        .find(|p| p.instance_parameter)
        .map(|p| p.name.clone());

    try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));

    if let Some(ref docs) = fn_.doc() {
        try!(write_lines(w, &fix_param_names(docs, &self_name), &tabs, symbols));
    };
    if let Some(ref docs) = fn_.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &fix_param_names(docs, &self_name), &tabs, symbols));
    };

    if fn_.parameters.iter().any(|x| {
           x.instance_parameter == false && !x.name.is_empty() && x.doc().is_some()
       }) {
        try!(write_header(w, &tabs, "Parameters"));
    }
    for parameter in fn_.parameters.iter() {
        if parameter.instance_parameter || parameter.name.is_empty() {
            continue
        }
        if let Some(ref parameter_doc) = parameter.doc() {
            try!(writeln!(w, "{}/// ## `{}`", tabs,
                          nameutil::mangle_keywords(&parameter.name[..])));
            try!(write_lines(w, &fix_param_names(parameter_doc, &self_name), &tabs, symbols));
        }
    }

    if let Some(ref doc) = fn_.ret.doc() {
        try!(write_header(w, &tabs, "Returns"));
        try!(write_lines(w, &fix_param_names(doc, &self_name), &tabs, symbols));
    }
    Ok(())
}

fn write_lines(w: &mut Write, lines: &str, tabs: &str, symbols: &symbols::Info) -> Result<()> {
    write_verbatim(w, &format::reformat_doc(&lines, symbols), tabs)
}

fn write_verbatim(w: &mut Write, lines: &str, tabs: &str) -> Result<()> {
    for line in lines.split("\n") {
        try!(writeln!(w, "{}/// {}", tabs, line));
    }
    Ok(())
}

fn create_sub_doc<T: ToStripperType>(w: &mut Write, ty: &T, symbols: &symbols::Info) -> Result<()> {
    let tabs = "";
    let sub_ty = ty.convert();

    if ty.doc().is_some() || ty.doc_deprecated().is_some() {
        try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));
    }

    if let Some(doc) = ty.doc() {
        try!(write_lines(w, &doc, &tabs, symbols));
    }
    if let Some(doc) = ty.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &doc, &tabs, symbols));
    }
    Ok(())
}

fn compute_indent(e: &Option<Box<TypeStruct>>) -> usize {
    match *e {
        Some(ref e) if e.parent.is_some() => compute_indent(&e.parent) + 1,
        _ => 0,
    }
}

fn write_header(w: &mut Write, tabs: &str, header: &str) -> Result<()> {
    writeln!(w, "{0}///\n{0}/// # {1}\n{0}///", tabs, header)
}
