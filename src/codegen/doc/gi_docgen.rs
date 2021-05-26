use crate::{config::gobjects::GStatus, nameutil, Env};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GiDocgenError {
    InvalidLinkType(String),
    BrokenLinkType(String),
    InvalidLink,
}

impl Display for GiDocgenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLinkType(e) => f.write_str(&format!("Invalid link type \"{}\"", e)),
            Self::BrokenLinkType(e) => {
                f.write_str(&format!("Broken link syntax for type \"{}\"", e))
            }
            Self::InvalidLink => f.write_str("Invalid link syntax"),
        }
    }
}

impl std::error::Error for GiDocgenError {}

/// Convert a "Namespace.Type" to (Option<Namespace>, Type)
fn namespace_type_from_details(
    link_details: &str,
    link_type: &str,
) -> Result<(Option<String>, String), GiDocgenError> {
    let res: Vec<&str> = link_details.split('.').collect();
    let len = res.len();
    if len == 1 {
        Ok((None, res[0].to_string()))
    } else if len == 2 {
        if res[1].is_empty() {
            Err(GiDocgenError::BrokenLinkType(link_type.to_string()))
        } else {
            Ok((Some(res[0].to_string()), res[1].to_string()))
        }
    } else {
        Err(GiDocgenError::BrokenLinkType(link_type.to_string()))
    }
}

/// Convert a "Namespace.Type.method_name" to (Option<Namespace>, Option<Type>, name)
/// Type is only optional for global functions and the order can be modified the `is_global_func` parameters
fn namespace_type_method_from_details(
    link_details: &str,
    link_type: &str,
    is_global_func: bool,
) -> Result<(Option<String>, Option<String>, String), GiDocgenError> {
    let res: Vec<&str> = link_details.split('.').collect();
    let len = res.len();
    if len == 1 {
        Ok((None, None, res[0].to_string()))
    } else if len == 2 {
        if res[1].is_empty() {
            Err(GiDocgenError::BrokenLinkType(link_type.to_string()))
        } else if is_global_func {
            Ok((Some(res[0].to_string()), None, res[1].to_string()))
        } else {
            Ok((None, Some(res[0].to_string()), res[1].to_string()))
        }
    } else if len == 3 {
        if res[2].is_empty() {
            Err(GiDocgenError::BrokenLinkType(link_type.to_string()))
        } else {
            Ok((
                Some(res[0].to_string()),
                Some(res[1].to_string()),
                res[2].to_string(),
            ))
        }
    } else {
        Err(GiDocgenError::BrokenLinkType(link_type.to_string()))
    }
}

static GI_DOCGEN_SYMBOLS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\[(callback|id|alias|class|const|ctor|enum|error|flags|func|iface|method|property|signal|struct|vfunc)[@](\w+\b)([:.]+[\w-]+\b)?([:.]+[\w-]+\b)?\]?").unwrap()
});

pub(crate) fn replace_c_types(entry: &str, env: &Env, _in_type: &str) -> String {
    GI_DOCGEN_SYMBOLS
        .replace_all(entry, |caps: &Captures<'_>| {
            if let Ok(gi_type) = GiDocgen::from_str(&caps[0]) {
                gi_type.rust_link(env)
            } else {
                // otherwise fallback to the original string
                caps[0].to_string()
            }
        })
        .to_string()
}

/// A representation of the various ways to link items using GI-docgen
///
/// See <https://gnome.pages.gitlab.gnome.org/gi-docgen/linking.html> for details.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GiDocgen {
    // C-identifier
    Id(String),
    // Alias to another type
    Alias(String),
    // Object Class
    Class {
        namespace: Option<String>,
        type_: String,
    },
    Const {
        namespace: Option<String>,
        type_: String,
    },
    Constructor {
        namespace: Option<String>,
        type_: String,
        name: String,
    },
    Callback {
        namespace: Option<String>,
        name: String,
    },
    Enum {
        namespace: Option<String>,
        type_: String,
    },
    Error {
        namespace: Option<String>,
        type_: String,
    },
    Flag {
        namespace: Option<String>,
        type_: String,
    },
    Func {
        namespace: Option<String>,
        type_: Option<String>,
        name: String,
    },
    Interface {
        namespace: Option<String>,
        type_: String,
    },
    Method {
        namespace: Option<String>,
        type_: String,
        name: String,
        is_instance: bool, // Whether `type_` ends with Class
    },
    Property {
        namespace: Option<String>,
        type_: String,
        name: String,
    },
    Signal {
        namespace: Option<String>,
        type_: String,
        name: String,
    },
    Struct {
        namespace: Option<String>,
        type_: String,
    },
    VFunc {
        namespace: Option<String>,
        type_: String,
        name: String,
    },
}

fn ns_type_to_doc(namespace: &Option<String>, type_: &str) -> String {
    if let Some(ns) = namespace {
        format!("{}::{}", ns, type_)
    } else {
        type_.to_string()
    }
}

impl GiDocgen {
    pub fn rust_link(&self, env: &Env) -> String {
        let symbols = env.symbols.borrow();

        match self {
            GiDocgen::Enum { namespace, type_ } | GiDocgen::Error { namespace, type_ } => {
                if let Some(enum_info) = env.analysis.enumerations.iter().find(|e| &e.name == type_)
                {
                    let sym = symbols.by_tid(enum_info.type_id).unwrap();
                    format!(
                        "[{name}](crate::{parent})",
                        name = enum_info.name,
                        parent = sym.parent().trim_end_matches("::")
                    )
                } else {
                    format!("`{}`", ns_type_to_doc(namespace, type_))
                }
            }
            GiDocgen::Class { namespace, type_ } | GiDocgen::Interface { namespace, type_ } => {
                if let Some((_, class_info)) =
                    env.analysis.objects.iter().find(|(_, o)| &o.name == type_)
                {
                    let sym = symbols.by_tid(class_info.type_id).unwrap();
                    format!("[{name}](crate::{name})", name = sym.full_rust_name())
                } else {
                    format!("`{}`", ns_type_to_doc(namespace, type_))
                }
            }
            GiDocgen::Flag { namespace, type_ } => {
                if let Some(flag_info) = env.analysis.flags.iter().find(|e| &e.name == type_) {
                    let sym = symbols.by_tid(flag_info.type_id).unwrap();
                    format!(
                        "[{name}](crate::{parent})",
                        name = flag_info.name,
                        parent = sym.parent().trim_end_matches("::")
                    )
                } else {
                    format!("`{}`", ns_type_to_doc(namespace, type_))
                }
            }
            GiDocgen::Const { namespace, type_ } => {
                if let Some(const_info) = env.analysis.constants.iter().find(|c| &c.name == type_) {
                    let sym = symbols.by_tid(const_info.typ).unwrap();
                    format!("[{name}](crate::{name})", name = sym.full_rust_name())
                } else {
                    format!("`{}`", ns_type_to_doc(namespace, type_))
                }
            }
            GiDocgen::Property {
                namespace,
                type_,
                name,
            } => {
                if let Some((_, class_info)) =
                    env.analysis.objects.iter().find(|(_, o)| &o.name == type_)
                {
                    let sym = symbols.by_tid(class_info.type_id).unwrap();
                    format!("`{}:{}`", sym.full_rust_name(), name)
                } else {
                    format!("`{}:{}`", ns_type_to_doc(namespace, type_), name)
                }
            }
            GiDocgen::Signal {
                namespace,
                type_,
                name,
            } => {
                if let Some((_, class_info)) =
                    env.analysis.objects.iter().find(|(_, o)| &o.name == type_)
                {
                    let sym = symbols.by_tid(class_info.type_id).unwrap();
                    format!("`{}::{}`", sym.full_rust_name(), name)
                } else {
                    format!("`{}::{}`", ns_type_to_doc(namespace, type_), name)
                }
            }
            GiDocgen::Id(c_name) => {
                if let Some(sym) = symbols.by_c_name(c_name) {
                    format!("[{name}](crate::{name})", name = sym.full_rust_name())
                } else {
                    format!("`{}`", c_name)
                }
            }
            GiDocgen::Struct { namespace, type_ } => {
                if let Some((_, record_info)) =
                    env.analysis.records.iter().find(|(_, r)| &r.name == type_)
                {
                    let sym = symbols.by_tid(record_info.type_id).unwrap();
                    format!("[{name}](crate::{name})", name = sym.full_rust_name())
                } else {
                    format!("`{}`", ns_type_to_doc(namespace, type_))
                }
            }
            GiDocgen::Constructor {
                namespace,
                type_,
                name,
            } => {
                if let Some((_, class_info)) =
                    env.analysis.objects.iter().find(|(_, o)| &o.name == type_)
                {
                    let sym = symbols.by_tid(class_info.type_id).unwrap();
                    if let Some(constructor) = class_info
                        .constructors()
                        .iter()
                        .find(|f| f.name == nameutil::mangle_keywords(name))
                    {
                        format!(
                            "[{name}::{fn_name}](crate::{name}::{fn_name})",
                            name = sym.full_rust_name(),
                            fn_name = constructor.codegen_name()
                        )
                    } else {
                        format!("`{}::{}`", sym.full_rust_name(), name)
                    }
                } else {
                    format!("`{}::{}`", ns_type_to_doc(namespace, type_), name)
                }
            }
            GiDocgen::Func {
                namespace: _,
                type_,
                name,
            } => {
                if let Some(ty) = type_ {
                    if let Some(obj_ty) = env
                        .analysis
                        .objects
                        .iter()
                        .find(|(_, o)| &o.name == ty)
                        .map(|(_, info)| info)
                    {
                        let sym = symbols.by_tid(obj_ty.type_id).unwrap();
                        if let Some(fn_info) = obj_ty
                            .functions
                            .iter()
                            .filter(|fn_info| {
                                !fn_info.is_special() && !fn_info.is_async_finish(env)
                            })
                            .find(|f| f.name == nameutil::mangle_keywords(name))
                        {
                            format!(
                                "[{name}::{fn_name}](crate::{name}::{fn_name})",
                                name = sym.full_rust_name(),
                                fn_name = fn_info.codegen_name()
                            )
                        } else {
                            format!("`{}::{}`", sym.full_rust_name(), name)
                        }
                    } else {
                        format!("`{}`", name)
                    }
                } else if let Some(fn_info) = env
                    .analysis
                    .global_functions
                    .as_ref()
                    .unwrap()
                    .functions
                    .iter()
                    .filter(|fn_info| !fn_info.is_special() && !fn_info.is_async_finish(env))
                    .find(|n| &n.name == name)
                {
                    format!(
                        "[{fn_name}()](crate::{fn_name})",
                        fn_name = fn_info.codegen_name()
                    )
                } else {
                    format!("`{}`", name)
                }
            }
            GiDocgen::Alias(alias) => {
                if let Some((_, record_info)) =
                    env.analysis.records.iter().find(|(_, r)| &r.name == alias)
                {
                    let sym = symbols.by_tid(record_info.type_id).unwrap();
                    format!(
                        "{alias} alias [{name}](crate::{name})",
                        alias = alias,
                        name = sym.full_rust_name()
                    )
                } else {
                    format!("`{}`", alias)
                }
            }
            GiDocgen::Method {
                namespace,
                type_,
                name,
                is_instance: _,
            } => {
                if let Some((_, class_info)) =
                    env.analysis.objects.iter().find(|(_, o)| &o.name == type_)
                {
                    let sym = symbols.by_tid(class_info.type_id).unwrap();
                    if let Some(fn_info) = class_info
                        .functions
                        .iter()
                        .filter(|f| f.status != GStatus::Ignore)
                        .filter(|fn_info| !fn_info.is_special() && !fn_info.is_async_finish(env))
                        .find(|f| f.name == nameutil::mangle_keywords(name))
                    {
                        let (type_name, visible_type_name) = if class_info.final_type {
                            (class_info.name.clone(), class_info.name.clone())
                        } else {
                            let type_name = if fn_info.status == GStatus::Generate {
                                class_info.trait_name.clone()
                            } else {
                                format!("{}Manual", class_info.trait_name)
                            };
                            (format!("prelude::{}", type_name), type_name)
                        };
                        format!(
                            "[{visible_type_name}::{fn_name}](crate::{name}::{fn_name})",
                            name = sym.full_rust_name().replace(type_, &type_name),
                            visible_type_name = visible_type_name,
                            fn_name = fn_info.codegen_name()
                        )
                    } else {
                        format!("`{}::{}()`", sym.full_rust_name(), name)
                    }
                } else if let Some((_, record_info)) =
                    env.analysis.records.iter().find(|(_, o)| &o.name == type_)
                {
                    let sym = symbols.by_tid(record_info.type_id).unwrap();
                    if let Some(fn_info) = record_info
                        .functions
                        .iter()
                        .filter(|f| f.status != GStatus::Ignore)
                        .filter(|fn_info| !fn_info.is_special() && !fn_info.is_async_finish(env))
                        .find(|f| f.name == nameutil::mangle_keywords(name))
                    {
                        format!(
                            "[{name}::{fn_name}](crate::{name}::{fn_name})",
                            name = sym.full_rust_name(),
                            fn_name = fn_info.codegen_name()
                        )
                    } else {
                        format!("`{}::{}()`", sym.full_rust_name(), name)
                    }
                } else {
                    format!("`{}::{}()`", ns_type_to_doc(namespace, type_), name)
                }
            }
            GiDocgen::Callback { namespace, name } => {
                format!("`{}`", ns_type_to_doc(namespace, name))
            }
            GiDocgen::VFunc {
                namespace,
                type_,
                name,
            } => {
                format!("`virtual:{}::{}`", ns_type_to_doc(namespace, type_), name)
            }
        }
    }
}

impl FromStr for GiDocgen {
    type Err = GiDocgenError;
    // We assume the string is contained inside a []
    fn from_str(item_link: &str) -> Result<Self, Self::Err> {
        let item_link = item_link.trim_start_matches('[').trim_end_matches(']');
        if let Some((link_type, link_details)) = item_link.split_once('@') {
            match link_type {
                "alias" => Ok(GiDocgen::Alias(link_details.to_string())),
                "class" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "class")?;
                    Ok(GiDocgen::Class { namespace, type_ })
                }
                "const" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "const")?;
                    Ok(GiDocgen::Const { namespace, type_ })
                }
                "ctor" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "ctor", false)?;
                    Ok(GiDocgen::Constructor {
                        namespace,
                        type_: type_
                            .ok_or_else(|| GiDocgenError::BrokenLinkType("ctor".to_string()))?,
                        name,
                    })
                }
                "enum" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "enum")?;
                    Ok(GiDocgen::Enum { namespace, type_ })
                }
                "error" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "error")?;
                    Ok(GiDocgen::Error { namespace, type_ })
                }
                "flags" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "flags")?;
                    Ok(GiDocgen::Flag { namespace, type_ })
                }
                "func" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "func", true)?;
                    Ok(GiDocgen::Func {
                        namespace,
                        type_,
                        name,
                    })
                }
                "iface" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "iface")?;
                    Ok(GiDocgen::Interface { namespace, type_ })
                }
                "callback" => {
                    let (namespace, name) = namespace_type_from_details(link_details, "callback")?;
                    Ok(GiDocgen::Callback { namespace, name })
                }
                "method" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "method", false)?;
                    let type_ =
                        type_.ok_or_else(|| GiDocgenError::BrokenLinkType("method".to_string()))?;
                    Ok(GiDocgen::Method {
                        namespace,
                        is_instance: type_.ends_with("Class"),
                        type_,
                        name,
                    })
                }
                "property" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "property")?;
                    let type_details: Vec<_> = type_.split(':').collect();
                    if type_details.len() < 2 || type_details[1].is_empty() {
                        Err(GiDocgenError::BrokenLinkType("property".to_string()))
                    } else {
                        Ok(GiDocgen::Property {
                            namespace,
                            type_: type_details[0].to_string(),
                            name: type_details[1].to_string(),
                        })
                    }
                }
                "signal" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "signal")?;
                    let type_details: Vec<_> = type_.split("::").collect();
                    if type_details.len() < 2 || type_details[1].is_empty() {
                        Err(GiDocgenError::BrokenLinkType("signal".to_string()))
                    } else {
                        Ok(GiDocgen::Signal {
                            namespace,
                            type_: type_details[0].to_string(),
                            name: type_details[1].to_string(),
                        })
                    }
                }
                "struct" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "struct")?;
                    Ok(GiDocgen::Struct { namespace, type_ })
                }
                "vfunc" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "vfunc", false)?;
                    Ok(GiDocgen::VFunc {
                        namespace,
                        type_: type_
                            .ok_or_else(|| GiDocgenError::BrokenLinkType("vfunc".to_string()))?,
                        name,
                    })
                }
                "id" => Ok(GiDocgen::Id(link_details.to_string())),
                e => Err(GiDocgenError::InvalidLinkType(e.to_string())),
            }
        } else {
            Err(GiDocgenError::InvalidLink)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_link_alias() {
        assert_eq!(
            GiDocgen::from_str("[alias@Allocation]"),
            Ok(GiDocgen::Alias("Allocation".to_string()))
        );
    }

    #[test]
    fn test_link_class() {
        assert_eq!(
            GiDocgen::from_str("[class@Widget]"),
            Ok(GiDocgen::Class {
                namespace: None,
                type_: "Widget".to_string(),
            })
        );
        assert_eq!(
            GiDocgen::from_str("[class@Gdk.Surface]"),
            Ok(GiDocgen::Class {
                namespace: Some("Gdk".to_string()),
                type_: "Surface".to_string(),
            })
        );
        assert_eq!(
            GiDocgen::from_str("[class@Gsk.RenderNode]"),
            Ok(GiDocgen::Class {
                namespace: Some("Gsk".to_string()),
                type_: "RenderNode".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[class@Gsk.RenderNode.test]"),
            Err(GiDocgenError::BrokenLinkType("class".to_string()))
        );

        assert_eq!(
            GiDocgen::from_str("[class@Gsk.]"),
            Err(GiDocgenError::BrokenLinkType("class".to_string()))
        );
    }

    #[test]
    fn test_link_id() {
        assert_eq!(
            GiDocgen::from_str("[id@gtk_widget_show]"),
            Ok(GiDocgen::Id("gtk_widget_show".to_string()))
        );
    }

    #[test]
    fn test_link_const() {
        assert_eq!(
            GiDocgen::from_str("[const@Gdk.KEY_q]"),
            Ok(GiDocgen::Const {
                namespace: Some("Gdk".to_string()),
                type_: "KEY_q".to_string()
            })
        );
    }

    #[test]
    fn test_link_callback() {
        assert_eq!(
            GiDocgen::from_str("[callback@Gtk.MapListModelMapFunc]"),
            Ok(GiDocgen::Callback {
                namespace: Some("Gtk".to_string()),
                name: "MapListModelMapFunc".to_string()
            })
        )
    }

    #[test]
    fn test_link_enum() {
        assert_eq!(
            GiDocgen::from_str("[enum@Orientation]"),
            Ok(GiDocgen::Enum {
                namespace: None,
                type_: "Orientation".to_string()
            })
        );
    }

    #[test]
    fn test_link_error() {
        assert_eq!(
            GiDocgen::from_str("[error@Gtk.BuilderParseError]"),
            Ok(GiDocgen::Error {
                namespace: Some("Gtk".to_string()),
                type_: "BuilderParseError".to_string()
            })
        );
    }

    #[test]
    fn test_link_flags() {
        assert_eq!(
            GiDocgen::from_str("[flags@Gdk.ModifierType]"),
            Ok(GiDocgen::Flag {
                namespace: Some("Gdk".to_string()),
                type_: "ModifierType".to_string()
            })
        );
    }

    #[test]
    fn test_link_iface() {
        assert_eq!(
            GiDocgen::from_str("[iface@Gtk.Buildable]"),
            Ok(GiDocgen::Interface {
                namespace: Some("Gtk".to_string()),
                type_: "Buildable".to_string()
            })
        );
    }

    #[test]
    fn test_link_struct() {
        assert_eq!(
            GiDocgen::from_str("[struct@Gtk.TextIter]"),
            Ok(GiDocgen::Struct {
                namespace: Some("Gtk".to_string()),
                type_: "TextIter".to_string()
            })
        );
    }

    #[test]
    fn test_link_property() {
        assert_eq!(
            GiDocgen::from_str("[property@Gtk.Orientable:orientation]"),
            Ok(GiDocgen::Property {
                namespace: Some("Gtk".to_string()),
                type_: "Orientable".to_string(),
                name: "orientation".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[property@Gtk.Orientable]"),
            Err(GiDocgenError::BrokenLinkType("property".to_string()))
        );

        assert_eq!(
            GiDocgen::from_str("[property@Gtk.Orientable:]"),
            Err(GiDocgenError::BrokenLinkType("property".to_string()))
        );
    }

    #[test]
    fn test_link_signal() {
        assert_eq!(
            GiDocgen::from_str("[signal@Gtk.RecentManager::changed]"),
            Ok(GiDocgen::Signal {
                namespace: Some("Gtk".to_string()),
                type_: "RecentManager".to_string(),
                name: "changed".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[signal@Gtk.RecentManager]"),
            Err(GiDocgenError::BrokenLinkType("signal".to_string()))
        );

        assert_eq!(
            GiDocgen::from_str("[signal@Gtk.RecentManager::]"),
            Err(GiDocgenError::BrokenLinkType("signal".to_string()))
        );

        assert_eq!(
            GiDocgen::from_str("[signal@Gtk.RecentManager:]"),
            Err(GiDocgenError::BrokenLinkType("signal".to_string()))
        );
    }

    #[test]
    fn test_link_vfunc() {
        assert_eq!(
            GiDocgen::from_str("[vfunc@Gtk.Widget.measure]"),
            Ok(GiDocgen::VFunc {
                namespace: Some("Gtk".to_string()),
                type_: "Widget".to_string(),
                name: "measure".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[vfunc@Widget.snapshot]"),
            Ok(GiDocgen::VFunc {
                namespace: None,
                type_: "Widget".to_string(),
                name: "snapshot".to_string(),
            })
        );
    }

    #[test]
    fn test_link_ctor() {
        assert_eq!(
            GiDocgen::from_str("[ctor@Gtk.Box.new]"),
            Ok(GiDocgen::Constructor {
                namespace: Some("Gtk".to_string()),
                type_: "Box".to_string(),
                name: "new".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[ctor@Button.new_with_label]"),
            Ok(GiDocgen::Constructor {
                namespace: None,
                type_: "Button".to_string(),
                name: "new_with_label".to_string(),
            })
        );
    }

    #[test]
    fn test_link_func() {
        assert_eq!(
            GiDocgen::from_str("[func@Gtk.init]"),
            Ok(GiDocgen::Func {
                namespace: Some("Gtk".to_string()),
                type_: None,
                name: "init".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[func@show_uri]"),
            Ok(GiDocgen::Func {
                namespace: None,
                type_: None,
                name: "show_uri".to_string(),
            })
        );

        assert_eq!(
            GiDocgen::from_str("[func@Gtk.Window.list_toplevels]"),
            Ok(GiDocgen::Func {
                namespace: Some("Gtk".to_string()),
                type_: Some("Window".to_string()),
                name: "list_toplevels".to_string(),
            })
        );
    }

    #[test]
    fn test_link_method() {
        assert_eq!(
            GiDocgen::from_str("[method@Gtk.Widget.show]"),
            Ok(GiDocgen::Method {
                namespace: Some("Gtk".to_string()),
                type_: "Widget".to_string(),
                name: "show".to_string(),
                is_instance: false,
            })
        );

        assert_eq!(
            GiDocgen::from_str("[method@WidgetClass.add_binding]"),
            Ok(GiDocgen::Method {
                namespace: None,
                type_: "WidgetClass".to_string(),
                name: "add_binding".to_string(),
                is_instance: true,
            })
        );
    }
}
