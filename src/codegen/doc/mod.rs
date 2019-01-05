use std::borrow::Cow;
use std::io::{Result, Write};

use analysis;
use analysis::namespaces::MAIN;
use case::CaseExt;
use config::gobjects::GObject;
use env::Env;
use file_saver::save_to_file;
use library::*;
use library::Type as LType;
use nameutil;
use regex::{Captures, Regex};
use self::format::reformat_doc;
use stripper_lib::Type as SType;
use stripper_lib::{write_file_name, write_item_doc, TypeStruct};
use traits::*;
use version::Version;

mod format;

trait ToStripperType {
    fn to_stripper_type(&self) -> TypeStruct;
}

macro_rules! impl_to_stripper_type {
    ($ty:ident, $enum_var:ident, $useless:expr) => {
        impl ToStripperType for $ty {
            fn to_stripper_type(&self) -> TypeStruct {
                TypeStruct::new(SType::$enum_var,
                                &format!("connect_{}", self.name.replace('-', "_")))
            }
        }
    };
    ($ty:ident, $enum_var:ident) => {
        impl ToStripperType for $ty {
            fn to_stripper_type(&self) -> TypeStruct {
                TypeStruct::new(SType::$enum_var, &self.name)
            }
        }
    }
}

trait FunctionLikeType {
    fn doc(&self) -> &Option<String>;
    fn doc_deprecated(&self) -> &Option<String>;
    fn ret(&self) -> &Parameter;
    fn parameters(&self) -> &[Parameter];
    fn version(&self) -> &Option<Version>;
    fn deprecated_version(&self) -> &Option<Version>;
}

macro_rules! impl_function_like_type {
    ($ty:ident) => {
        impl FunctionLikeType for $ty {
            fn doc(&self) -> &Option<String> {
                &self.doc
            }
            fn doc_deprecated(&self) -> &Option<String> {
                &self.doc_deprecated
            }
            fn ret(&self) -> &Parameter {
                &self.ret
            }
            fn parameters(&self) -> &[Parameter] {
                &self.parameters
            }
            fn version(&self) -> &Option<Version> {
                &self.version
            }
            fn deprecated_version(&self) -> &Option<Version> {
                &self.deprecated_version
            }
        }
    }
}

impl_to_stripper_type!(Member, Variant);
impl_to_stripper_type!(Enumeration, Enum);
impl_to_stripper_type!(Bitfield, Type);
impl_to_stripper_type!(Record, Struct);
impl_to_stripper_type!(Class, Struct);
impl_to_stripper_type!(Function, Fn);
impl_to_stripper_type!(Signal, Fn, false);

impl_function_like_type!(Function);
impl_function_like_type!(Signal);

pub fn generate(env: &Env) {
    info!("Generating documentation {:?}", env.config.doc_target_path);
    save_to_file(&env.config.doc_target_path, env.config.make_backup, |w| generate_doc(w, env));
}

#[allow(clippy::type_complexity)]
fn generate_doc(w: &mut Write, env: &Env) -> Result<()> {
    write_file_name(w, None)?;
    let mut generators: Vec<(&str, Box<Fn(&mut Write, &Env) -> Result<()>>)> = Vec::new();

    for info in env.analysis.objects.values() {
        if info.type_id.ns_id == MAIN {
            generators.push((
                &info.name,
                Box::new(move |w, e| create_object_doc(w, e, info)),
            ));
        }
    }

    for info in env.analysis.records.values() {
        if info.type_id.ns_id == MAIN {
            generators.push((
                &info.name,
                Box::new(move |w, e| create_record_doc(w, e, info)),
            ));
        }
    }

    for (tid, type_) in env.library.namespace_types(MAIN) {
        if let LType::Enumeration(ref enum_) = *type_ {
            if !env.config
                .objects
                .get(&tid.full_name(&env.library))
                .map_or(true, |obj| obj.status.ignored())
            {
                generators.push((
                    &enum_.name[..],
                    Box::new(move |w, e| create_enum_doc(w, e, enum_)),
                ));
            }
        }
    }

    generators.sort_by_key(|&(name, _)| name);
    for (_, f) in generators {
        f(w, env)?;
    }

    Ok(())
}

fn create_object_doc(w: &mut Write, env: &Env, info: &analysis::object::Info) -> Result<()> {
    let symbols = env.symbols.borrow();
    let ty = TypeStruct::new(SType::Struct, &info.name);
    let ty_ext = TypeStruct::new(SType::Trait, &info.trait_name);
    let has_trait = info.generate_trait;
    let doc;
    let functions: &[Function];
    let signals: &[Signal];
    let properties: &[Property];

    match *env.library.type_(info.type_id) {
        Type::Class(ref cl) => {
            doc = cl.doc.as_ref();
            functions = &cl.functions;
            signals = &cl.signals;
            properties = &cl.properties;
        }
        Type::Interface(ref iface) => {
            doc = iface.doc.as_ref();
            functions = &iface.functions;
            signals = &iface.signals;
            properties = &iface.properties;
        }
        _ => unreachable!(),
    }

    write_item_doc(w, &ty, |w| {
        if let Some(ver) = info.deprecated_version {
            write!(w, "`[Deprecated since {}]` ", ver)?;
        }
        if let Some(doc) = doc {
            writeln!(w, "{}", reformat_doc(doc, &symbols))?;
        } else {
            writeln!(w)?;
        }
        if let Some(version) = info.version {
            writeln!(w, "\nFeature: `{}`", version.to_feature())?;
        }

        let impl_self = if has_trait { Some(info.type_id) } else { None };
        let implements = impl_self
            .iter()
            .chain(env.class_hierarchy.supertypes(info.type_id))
            .filter(|&tid| {
                !env.type_status(&tid.full_name(&env.library)).ignored()
            })
            .map(|&tid| get_type_trait_for_implements(env, tid))
            .collect::<Vec<_>>();
        if !implements.is_empty() {
            writeln!(w, "\n# Implements\n")?;
            writeln!(w, "{}", &implements.join(", "))?;
        }
        Ok(())
    })?;

    if has_trait {
        write_item_doc(w, &ty_ext, |w| {
            if let Some(ver) = info.deprecated_version {
                write!(w, "`[Deprecated since {}]` ", ver)?;
            }
            writeln!(w, "Trait containing all `{}` methods.", ty.name)?;

            if let Some(version) = info.version {
                writeln!(w, "\nFeature: `{}`", version.to_feature())?;
            }

            let mut implementors = Some(info.type_id)
                .into_iter()
                .chain(env.class_hierarchy.subtypes(info.type_id))
                .filter(|&tid| {
                    !env.type_status(&tid.full_name(&env.library)).ignored()
                })
                .map(|tid| {
                    format!(
                        "[`{name}`](struct.{name}.html)",
                        name = env.library.type_(tid).get_name()
                    )
                })
                .collect::<Vec<_>>();
            implementors.sort();

            writeln!(w, "\n# Implementors\n")?;
            writeln!(w, "{}", implementors.join(", "))?;
            Ok(())
        })?;
    }

    let ty = TypeStruct {
        ty: SType::Impl,
        ..ty
    };

    for function in functions {
        let ty = if has_trait && function.parameters.iter().any(|p| p.instance_parameter) {
            ty_ext.clone()
        } else {
            ty.clone()
        };
        create_fn_doc(w, env, function, Some(Box::new(ty)))?;
    }
    for signal in signals {
        create_fn_doc(
            w,
            env,
            signal,
            Some(Box::new(ty_ext.clone()))
        )?;
    }
    for property in properties {
        create_property_doc(
            w,
            env,
            property,
            Some(Box::new(ty_ext.clone()))
        )?;
    }
    Ok(())
}

fn create_record_doc(w: &mut Write, env: &Env, info: &analysis::record::Info) -> Result<()> {
    let record: &Record = env.library.type_(info.type_id).to_ref_as();
    let ty = record.to_stripper_type();
    let symbols = env.symbols.borrow();

    write_item_doc(w, &ty, |w| {
        if let Some(ref doc) = record.doc {
            if let Some(ver) = info.deprecated_version {
                write!(w, "`[Deprecated since {}]` ", ver)?;
            }
            writeln!(w, "{}", reformat_doc(doc, &symbols))?;
        }
        if let Some(ver) = info.deprecated_version {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if record.doc_deprecated.is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = record.doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, &symbols))?;
        }
        if let Some(version) = info.version {
            writeln!(w, "\nFeature: `{}`", version.to_feature())?;
        }
        Ok(())
    })?;

    let ty = TypeStruct {
        ty: SType::Impl,
        ..ty
    };
    for function in &record.functions {
        create_fn_doc(w, env, function, Some(Box::new(ty.clone())))?;
    }
    Ok(())
}

fn create_enum_doc(w: &mut Write, env: &Env, enum_: &Enumeration) -> Result<()> {
    let ty = enum_.to_stripper_type();
    let symbols = env.symbols.borrow();

    write_item_doc(w, &ty, |w| {
        if let Some(ref doc) = enum_.doc {
            writeln!(w, "{}", reformat_doc(doc, &symbols))?;
        }
        if let Some(ver) = enum_.deprecated_version {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if enum_.doc_deprecated.is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = enum_.doc_deprecated {
            writeln!(w, "{}", reformat_doc(doc, &symbols))?;
        }
        Ok(())
    })?;

    for member in &enum_.members {
        let mut sub_ty = TypeStruct {
            name: member.name.to_camel(),
            ..member.to_stripper_type()
        };

        if member.doc.is_some() {
            sub_ty.parent = Some(Box::new(ty.clone()));
            write_item_doc(w, &sub_ty, |w| {
                if let Some(ref doc) = member.doc {
                    writeln!(w, "{}", reformat_doc(doc, &symbols))?;
                }
                Ok(())
            })?;
        }
    }

    if let Some(version) = enum_.version {
        if version > env.config.min_cfg_version {
            writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
        }
    }
    Ok(())
}

lazy_static! {
    static ref PARAM_NAME: Regex = Regex::new(r"@(\w+)\b").unwrap();
}

fn fix_param_names<'a>(doc: &'a str, self_name: &Option<String>) -> Cow<'a, str> {
    PARAM_NAME.replace_all(doc, |caps: &Captures| {
        if let Some(ref self_name) = *self_name {
            if &caps[1] == self_name {
                return "@self".into();
            }
        }
        format!("@{}", nameutil::mangle_keywords(&caps[1]))
    })
}

fn create_fn_doc<T>(
    w: &mut Write,
    env: &Env,
    fn_: &T,
    parent: Option<Box<TypeStruct>>,
) -> Result<()>
where
    T: FunctionLikeType + ToStripperType,
{
    if fn_.doc().is_none() && fn_.doc_deprecated().is_none() && fn_.ret().doc.is_none()
        && fn_.parameters().iter().all(|p| p.doc.is_none())
    {
        return Ok(());
    }

    let symbols = env.symbols.borrow();
    let ty = TypeStruct {
        parent,
        ..fn_.to_stripper_type()
    };
    let self_name: Option<String> = fn_.parameters()
        .iter()
        .find(|p| p.instance_parameter)
        .map(|p| p.name.clone());

    write_item_doc(w, &ty, |w| {
        if let Some(ref doc) = *fn_.doc() {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), &symbols)
            )?;
        }
        if let Some(version) = *fn_.version() {
            if version > env.config.min_cfg_version {
                writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
            }
        }
        if let Some(ver) = *fn_.deprecated_version() {
            writeln!(w, "\n# Deprecated since {}\n", ver)?;
        } else if fn_.doc_deprecated().is_some() {
            writeln!(w, "\n# Deprecated\n")?;
        }
        if let Some(ref doc) = *fn_.doc_deprecated() {
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), &symbols)
            )?;
        }

        for parameter in fn_.parameters() {
            if parameter.instance_parameter || parameter.name.is_empty() {
                continue;
            }
            if let Some(ref doc) = parameter.doc {
                writeln!(
                    w,
                    "## `{}`",
                    nameutil::mangle_keywords(&parameter.name[..])
                )?;
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &self_name), &symbols)
                )?;
            }
        }

        if let Some(ref doc) = fn_.ret().doc {
            writeln!(w, "\n# Returns\n")?;
            writeln!(
                w,
                "{}",
                reformat_doc(&fix_param_names(doc, &self_name), &symbols)
            )?;
        }
        Ok(())
    })
}

fn create_property_doc(
    w: &mut Write,
    env: &Env,
    property: &Property,
    parent: Option<Box<TypeStruct>>,
) -> Result<()> {
    if property.doc.is_none() && property.doc_deprecated.is_none()
        && (property.readable || property.writable)
    {
        return Ok(());
    }
    let mut v = Vec::with_capacity(2);

    let symbols = env.symbols.borrow();
    if property.readable {
        v.push(TypeStruct {
            parent: parent.clone(),
            ..TypeStruct::new(SType::Fn, &format!("get_property_{}", property.name))
        });
    }
    if property.writable {
        v.push(TypeStruct {
            parent,
            ..TypeStruct::new(SType::Fn, &format!("set_property_{}", property.name))
        });
    }

    for item in &v {
        write_item_doc(w, item, |w| {
            if let Some(ref doc) = property.doc {
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &None), &symbols)
                )?;
            }
            if let Some(version) = property.version {
                if version > env.config.min_cfg_version {
                    writeln!(w, "\nFeature: `{}`\n", version.to_feature())?;
                }
            }
            if let Some(ver) = property.deprecated_version {
                writeln!(w, "\n# Deprecated since {}\n", ver)?;
            } else if property.doc_deprecated.is_some() {
                writeln!(w, "\n# Deprecated\n")?;
            }
            if let Some(ref doc) = property.doc_deprecated {
                writeln!(
                    w,
                    "{}",
                    reformat_doc(&fix_param_names(doc, &None), &symbols)
                )?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn get_type_trait_for_implements(env: &Env, tid: TypeId) -> String {
    let trait_name = if let Some(&GObject {
        trait_name: Some(ref trait_name),
        ..
    }) = env.config.objects.get(&tid.full_name(&env.library))
    {
        trait_name.clone()
    } else {
        format!("{}Ext", env.library.type_(tid).get_name())
    };
    if tid.ns_id == MAIN_NAMESPACE {
        format!("[`{name}`](trait.{name}.html)", name = trait_name)
    } else if let Some(symbol) = env.symbols.borrow().by_tid(tid) {
        let mut name = symbol.full_rust_name();
        let crate_name = if let Some(crate_name) = symbol.crate_name() {
            if crate_name == "gobject" {
                name = name.replace("gobject", "glib::object");
                "glib/object".to_owned()
            } else {
                crate_name.clone()
            }
        } else {
            error!("Type {} don't have crate", tid.full_name(&env.library));
            "unknown".to_owned()
        };
        format!(
            "[`{}Ext`](../{}/trait.{}.html)",
            name,
            crate_name,
            trait_name
        )
    } else {
        error!("Type {} don't have crate", tid.full_name(&env.library));
        format!("`{}`", trait_name)
    }
}
