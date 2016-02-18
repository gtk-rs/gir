use std::io::{Result, Write};

use analysis;
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
        let class_analysis = match info {
            Some(info) => info,
            None => continue,
        };
        let has_trait = class_analysis.has_children;

        try!(handle_type(w, &env.library.type_(class_analysis.type_id), None, has_trait,
                         true, &env));
    }

    for ty in namespace.types.iter().filter_map(|t| t.as_ref()) {
        try!(handle_type(&mut w, &ty, None, false, false, &env));
    }
    Ok(())
}

fn handle_type(w: &mut Write, ty: &LType, parent: Option<Box<TypeStruct>>,
               has_trait: bool, handle_structs: bool,
               env: &Env) -> Result<()> {
    match *ty {
        LType::Alias(ref a) => create_sub_doc(w, a, parent, env),
        LType::Enumeration(ref e) => create_enum_doc(w, &e, parent, env),
        LType::Function(ref f) => create_fn_doc(w, &f, parent, env),
        LType::Interface(ref i) if handle_structs => create_class_doc(w, i, parent, has_trait,
                                                                      env),
        LType::Class(ref c) if handle_structs => create_class_doc(w, c, parent, has_trait,
                                                                  env),
        _ => Ok(()),
    }
}

fn create_class_doc<T: FunctionTraitType + ToStripperType>(w: &mut Write, class: &T,
                                                           parent: Option<Box<TypeStruct>>,
                                                           has_trait: bool,
                                                           env: &Env)
                                                          -> Result<()> {
    let tabs : String = primitives::tabs(compute_indent(&parent));
    let ty = TypeStruct { parent: parent, ..class.convert() };
    let ty_ext = TypeStruct {
        ty: SType::Trait,
        name: format!("{}Ext", ty.name),
        ..ty.clone()
    };

    //try!(writeln!(w, "{}src/auto/{}.rs", FILE, module_name(&class.name())));
    if class.doc().is_some() || class.doc_deprecated().is_some() {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
    }

    if let Some(ref class_doc) = class.doc() {
        try!(write_lines(w, &class_doc, &tabs, env));
    }
    if let Some(ref class_doc) = class.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &class_doc, &tabs, env));
    }

    let ty = TypeStruct { ty: SType::Impl, ..ty };

    for function in class.functions().iter() {
        let ty = if has_trait && function.parameters.iter().any(|p| p.instance_parameter) {
            ty_ext.clone()
        }
        else {
            ty.clone()
        };
        try!(create_fn_doc(w, &function, Some(Box::new(ty)), env));
    }
    Ok(())
}

fn create_enum_doc(w: &mut Write, enum_: &Enumeration,
                   parent: Option<Box<TypeStruct>>,
                   env: &Env) -> Result<()> {
    let indent = compute_indent(&parent);
    let mut ty = enum_.convert();
    ty.parent = parent;
    let tabs : String = primitives::tabs(indent);

    if enum_.doc().is_some() || enum_.doc_deprecated().is_some() {
        try!(writeln!(w, "{}{}", MOD_COMMENT, ty));
    }

    if let Some(ref enum_doc) = enum_.doc() {
        try!(write_lines(w, &enum_doc, &tabs, env));
    }
    if let Some(ref enum_doc) = enum_.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &enum_doc, &tabs, env));
    }

    let tabs : String = primitives::tabs(indent + 1);
    for member_doc in enum_.members.iter() {
        let mut sub_ty : TypeStruct = member_doc.convert();

        if member_doc.doc().is_some() || member_doc.doc_deprecated().is_some() {
            sub_ty.parent = Some(Box::new(ty.clone()));
            try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));
        }
        if let Some(ref m_doc) = member_doc.doc() {
            try!(write_lines(w, &m_doc, &tabs, env));
        }
        if let Some(ref m_doc) = member_doc.doc_deprecated() {
            try!(write_header(w, &tabs, "Deprecated"));
            try!(write_lines(w, &m_doc, &tabs, env));
        }
    }
    Ok(())
}

fn create_fn_doc(w: &mut Write, fn_: &Function, parent: Option<Box<TypeStruct>>,
                 env: &Env) -> Result<()> {
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
    try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));

    if let Some(ref docs) = fn_.doc() {
        try!(write_lines(w, &docs, &tabs, env));
    };
    if let Some(ref docs) = fn_.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &docs, &tabs, env));
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
            try!(writeln!(w, "{}/// ## {}:", tabs, parameter.name));
            try!(write_lines(w, &parameter_doc, &tabs, env));
        }
    }

    if let Some(ref doc) = fn_.ret.doc() {
        try!(write_header(w, &tabs, "Returns"));
        try!(write_lines(w, &doc, &tabs, env));
    }
    Ok(())
}

fn write_lines(w: &mut Write, lines: &str, tabs: &str,
               env: &Env) -> Result<()> {
    for line in format::reformat_doc(&lines, env).split("\n") {
        try!(writeln!(w, "{}/// {}", tabs, line));
    }
    Ok(())
}

fn create_sub_doc<T: ToStripperType>(w: &mut Write, ty: &T,
                                     parent: Option<Box<TypeStruct>>,
                                     env: &Env) -> Result<()> {
    let tabs : String = primitives::tabs(compute_indent(&parent));
    let mut sub_ty = ty.convert();
    sub_ty.parent = parent;

    if ty.doc().is_some() || ty.doc_deprecated().is_some() {
        try!(writeln!(w, "{}{}", MOD_COMMENT, sub_ty));
    }

    if let Some(doc) = ty.doc() {
        try!(write_lines(w, &doc, &tabs, env));
    }
    if let Some(doc) = ty.doc_deprecated() {
        try!(write_header(w, &tabs, "Deprecated"));
        try!(write_lines(w, &doc, &tabs, env));
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
