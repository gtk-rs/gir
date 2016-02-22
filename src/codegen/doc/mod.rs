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
use self::format::reformat_doc;
use stripper_lib::Type as SType;
use stripper_lib::{TypeStruct, write_file_name, write_item_doc};

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
    fn convert(&self) -> TypeStruct;
    fn doc(&self) -> Option<&str>;
    fn doc_deprecated(&self) -> Option<&str>;
}

macro_rules! impl_to_stripper_type {
    ($ty:ident, $enum_var:ident) => {
        impl ToStripperType for $ty {
            fn convert(&self) -> TypeStruct {
                TypeStruct::new(SType::$enum_var, &self.name)
            }

            fn doc(&self) -> Option<&str> {
                self.doc.as_ref().map(|s| &s[..])
            }

            fn doc_deprecated(&self) -> Option<&str> {
                self.doc_deprecated.as_ref().map(|s| &s[..])
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
    let path =  env.config.target_path.join("docs.md");
    println!("Generating documentation {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_doc(w, &env));
}

fn generate_doc(mut w: &mut Write, env: &Env) -> Result<()> {
    try!(write_file_name(w, None));

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
    let ty = TypeStruct::new(SType::Struct, &info.name);
    let ty_ext = TypeStruct::new(SType::Trait, &format!("{}Ext", info.name));
    let has_trait = info.has_children;
    let impl_self = if has_trait { Some(info.type_id) } else { None };
    let class: &ToStripperType;
    let functions: &[Function];
    let implements: Vec<TypeId>;

    match *env.library.type_(info.type_id) {
        Type::Class(ref cl) => {
            class = cl;
            functions = &cl.functions;
            implements = impl_self.iter()
                .chain(cl.parents.iter())
                .chain(cl.implements.iter())
                .map(|&tid| tid)
                .collect();
        }
        Type::Interface(ref iface) => {
            class = iface;
            functions = &iface.functions;
            implements = impl_self.iter()
                .chain(iface.prereq_parents.iter())
                .map(|&tid| tid)
                .collect();
        }
        _ => unreachable!(),
    }

    if class.doc().is_some() || class.doc_deprecated().is_some() {
        try!(write_item_doc(w, &ty, |w| {
            if let Some(doc) = class.doc() {
                try!(writeln!(w, "{}", reformat_doc(doc, &symbols)));
            }

            try!(writeln!(w, "\n# Implements\n"));
            let implements = implements.iter()
                .filter(|&tid| !env.type_status(&tid.full_name(&env.library)).ignored())
                .map(|&tid| format!("[`{name}Ext`](trait.{name}Ext.html)",
                                    name = env.library.type_(tid).get_name()))
                .collect::<Vec<_>>();
            try!(writeln!(w, "{}", &implements.join(", ")));

            if let Some(doc) = class.doc_deprecated() {
                try!(writeln!(w, "\n# Deprecated\n"));
                try!(writeln!(w, "{}", reformat_doc(doc, &symbols)));
            }
            Ok(())
        }));
    }

    if has_trait {
        try!(write_item_doc(w, &ty_ext, |w| {
            writeln!(w, "Trait containing all `{}` methods.", ty.name)
        }));
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

    try!(write_item_doc(w, &ty, |w| {
        if let Some(doc) = enum_.doc() {
            try!(writeln!(w, "{}", reformat_doc(doc, symbols)));
        }
        if let Some(doc) = enum_.doc_deprecated() {
            try!(writeln!(w, "\n# Deprecated\n"));
            try!(writeln!(w, "{}", reformat_doc(doc, symbols)));
        }
        Ok(())
    }));

    for member in enum_.members.iter() {
        let mut sub_ty : TypeStruct = member.convert();

        if member.doc().is_some() || member.doc_deprecated().is_some() {
            sub_ty.parent = Some(Box::new(ty.clone()));
            try!(write_item_doc(w, &sub_ty, |w| {
                if let Some(doc) = member.doc() {
                    try!(writeln!(w, "{}", reformat_doc(doc, symbols)));
                }
                if let Some(doc) = member.doc_deprecated() {
                    try!(writeln!(w, "\n# Deprecated\n"));
                    try!(writeln!(w, "{}", reformat_doc(doc, symbols)));
                }
                Ok(())
            }));
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

    write_item_doc(w, &sub_ty, |w| {
        if let Some(doc) = fn_.doc() {
            try!(writeln!(w, "{}", reformat_doc(&fix_param_names(doc, &self_name), symbols)));
        };
        if let Some(doc) = fn_.doc_deprecated() {
            try!(writeln!(w, "\n# Deprecated\n"));
            try!(writeln!(w, "{}", reformat_doc(&fix_param_names(doc, &self_name), symbols)));
        };

        if fn_.parameters.iter().any(|x| {
               x.instance_parameter == false && !x.name.is_empty() && x.doc().is_some()
           }) {
            try!(writeln!(w, "\n# Parameters\n"));
        }
        for parameter in fn_.parameters.iter() {
            if parameter.instance_parameter || parameter.name.is_empty() {
                continue
            }
            if let Some(doc) = parameter.doc() {
                try!(writeln!(w, "## `{}`", nameutil::mangle_keywords(&parameter.name[..])));
                try!(writeln!(w, "{}", reformat_doc(&fix_param_names(doc, &self_name), symbols)));
            }
        }

        if let Some(doc) = fn_.ret.doc() {
            try!(writeln!(w, "\n# Returns\n"));
            try!(writeln!(w, "{}", reformat_doc(&fix_param_names(doc, &self_name), symbols)));
        }
        Ok(())
    })
}

fn create_sub_doc<T: ToStripperType>(w: &mut Write, ty: &T, symbols: &symbols::Info) -> Result<()> {
    let sub_ty = ty.convert();

    if ty.doc().is_some() || ty.doc_deprecated().is_some() {
        try!(write_item_doc(w, &sub_ty, |w| {
            if let Some(doc) = ty.doc() {
                try!(writeln!(w, "{}", reformat_doc(doc, symbols)));
            }
            if let Some(doc) = ty.doc_deprecated() {
                try!(writeln!(w, "\n# Deprecated\n"));
                try!(writeln!(w, "{}", reformat_doc(doc, symbols)));
            }
            Ok(())
        }));
    }
    Ok(())
}
