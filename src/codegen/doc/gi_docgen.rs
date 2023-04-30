use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use once_cell::sync::Lazy;
use regex::{Captures, Regex};

use super::format::find_method_or_function;
use crate::{
    analysis::object::LocationInObject,
    codegen::doc::format::{
        gen_alias_doc_link, gen_callback_doc_link, gen_const_doc_link, gen_object_fn_doc_link,
        gen_property_doc_link, gen_signal_doc_link, gen_symbol_doc_link, gen_vfunc_doc_link,
    },
    library::{TypeId, MAIN_NAMESPACE},
    nameutil::mangle_keywords,
    Env,
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
            Self::InvalidLinkType(e) => f.write_str(&format!("Invalid link type \"{e}\"")),
            Self::BrokenLinkType(e) => f.write_str(&format!("Broken link syntax for type \"{e}\"")),
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

/// Convert a "Namespace.Type.method_name" to (Option<Namespace>, Option<Type>,
/// name) Type is only optional for global functions and the order can be
/// modified the `is_global_func` parameters
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

pub(crate) fn replace_c_types(
    entry: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    GI_DOCGEN_SYMBOLS
        .replace_all(entry, |caps: &Captures<'_>| {
            if let Ok(gi_type) = GiDocgen::from_str(&caps[0]) {
                gi_type.rust_link(env, in_type)
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
        is_class_method: bool, // Whether `type_` ends with Class
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
        format!("{ns}::{type_}")
    } else {
        type_.to_string()
    }
}

fn find_virtual_method_by_name(
    type_: Option<&str>,
    namespace: Option<&str>,
    name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> Option<String> {
    find_method_or_function(
        env,
        in_type,
        |f| {
            f.name == mangle_keywords(name)
                && namespace.as_ref().map_or(f.ns_id == MAIN_NAMESPACE, |n| {
                    &env.library.namespaces[f.ns_id as usize].name == n
                })
        },
        |o| {
            type_.map_or(true, |t| {
                o.name == t && is_same_namespace(env, namespace, o.type_id)
            })
        },
        |_| false,
        |_| false,
        |_| false,
        false,
        true,
    )
}

fn find_method_or_function_by_name(
    type_: Option<&str>,
    namespace: Option<&str>,
    name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
    is_class_method: bool,
) -> Option<String> {
    find_method_or_function(
        env,
        in_type,
        |f| {
            f.name == mangle_keywords(name)
                && namespace.as_ref().map_or(f.ns_id == MAIN_NAMESPACE, |n| {
                    &env.library.namespaces[f.ns_id as usize].name == n
                })
        },
        |o| {
            type_.map_or(true, |t| {
                o.name == t && is_same_namespace(env, namespace, o.type_id)
            })
        },
        |r| {
            type_.map_or(true, |t| {
                r.name == t && is_same_namespace(env, namespace, r.type_id)
            })
        },
        |e| {
            type_.map_or(true, |t| {
                e.name == t && is_same_namespace(env, namespace, e.type_id)
            })
        },
        |f| {
            type_.map_or(true, |t| {
                f.name == t && is_same_namespace(env, namespace, f.type_id)
            })
        },
        is_class_method,
        false,
    )
}

fn is_same_namespace(env: &Env, namespace: Option<&str>, type_id: TypeId) -> bool {
    namespace
        .as_ref()
        .map_or(MAIN_NAMESPACE == type_id.ns_id, |n| {
            &env.library.namespaces[type_id.ns_id as usize].name == n
        })
}

impl GiDocgen {
    pub fn rust_link(
        &self,
        env: &Env,
        in_type: Option<(&TypeId, Option<LocationInObject>)>,
    ) -> String {
        let symbols = env.symbols.borrow();
        match self {
            GiDocgen::Enum { type_, namespace } | GiDocgen::Error { type_, namespace } => env
                .analysis
                .enumerations
                .iter()
                .find(|e| &e.name == type_)
                .map_or_else(
                    || format!("`{}`", ns_type_to_doc(namespace, type_)),
                    |info| gen_symbol_doc_link(info.type_id, env),
                ),
            GiDocgen::Class { type_, namespace } | GiDocgen::Interface { type_, namespace } => env
                .analysis
                .objects
                .values()
                .find(|o| {
                    &o.name == type_ && is_same_namespace(env, namespace.as_deref(), o.type_id)
                })
                .map_or_else(
                    || format!("`{}`", ns_type_to_doc(namespace, type_)),
                    |info| gen_symbol_doc_link(info.type_id, env),
                ),
            GiDocgen::Flag { type_, namespace } => env
                .analysis
                .flags
                .iter()
                .find(|e| {
                    &e.name == type_ && is_same_namespace(env, namespace.as_deref(), e.type_id)
                })
                .map_or_else(
                    || format!("`{}`", ns_type_to_doc(namespace, type_)),
                    |info| gen_symbol_doc_link(info.type_id, env),
                ),
            GiDocgen::Const { type_, namespace } => env
                .analysis
                .constants
                .iter()
                .find(|c| &c.name == type_ && is_same_namespace(env, namespace.as_deref(), c.typ))
                .map_or_else(
                    || format!("`{}`", ns_type_to_doc(namespace, type_)),
                    gen_const_doc_link,
                ),
            GiDocgen::Property {
                type_,
                name,
                namespace,
            } => env
                .analysis
                .objects
                .values()
                .find(|o| {
                    &o.name == type_ && is_same_namespace(env, namespace.as_deref(), o.type_id)
                })
                .map_or_else(
                    || gen_property_doc_link(&ns_type_to_doc(namespace, type_), name),
                    |info| {
                        let sym = symbols.by_tid(info.type_id).unwrap();
                        gen_property_doc_link(&sym.full_rust_name(), name)
                    },
                ),
            GiDocgen::Signal {
                type_,
                name,
                namespace,
            } => env
                .analysis
                .objects
                .values()
                .find(|o| {
                    &o.name == type_ && is_same_namespace(env, namespace.as_deref(), o.type_id)
                })
                .map_or_else(
                    || gen_signal_doc_link(&ns_type_to_doc(namespace, type_), name),
                    |info| {
                        let sym = symbols.by_tid(info.type_id).unwrap();
                        gen_signal_doc_link(&sym.full_rust_name(), name)
                    },
                ),
            GiDocgen::Id(c_name) => symbols.by_c_name(c_name).map_or_else(
                || format!("`{c_name}`"),
                |sym| format!("[`{n}`][crate::{n}]", n = sym.full_rust_name()),
            ),
            GiDocgen::Struct { namespace, type_ } => env
                .analysis
                .records
                .values()
                .find(|r| {
                    &r.name == type_ && is_same_namespace(env, namespace.as_deref(), r.type_id)
                })
                .map_or_else(
                    || format!("`{}`", ns_type_to_doc(namespace, type_)),
                    |info| gen_symbol_doc_link(info.type_id, env),
                ),
            GiDocgen::Constructor {
                namespace,
                type_,
                name,
            } => env
                .analysis
                .find_object_by_function(
                    env,
                    |o| &o.name == type_ && is_same_namespace(env, namespace.as_deref(), o.type_id),
                    |f| f.name == mangle_keywords(name),
                )
                .map_or_else(
                    || format!("`{}::{}()`", ns_type_to_doc(namespace, type_), name),
                    |(obj_info, fn_info)| {
                        gen_object_fn_doc_link(obj_info, fn_info, env, in_type, type_)
                    },
                ),
            GiDocgen::Func {
                namespace,
                type_,
                name,
            } => find_method_or_function_by_name(
                type_.as_deref(),
                namespace.as_deref(),
                name,
                env,
                in_type,
                false,
            )
            .unwrap_or_else(|| {
                if let Some(ty) = type_ {
                    format!("`{}::{}()`", ns_type_to_doc(namespace, ty), name)
                } else {
                    format!("`{name}()`")
                }
            }),
            GiDocgen::Alias(alias) => gen_alias_doc_link(alias),
            GiDocgen::Method {
                namespace,
                type_,
                name,
                is_class_method,
            } => find_method_or_function_by_name(
                Some(type_),
                namespace.as_deref(),
                name,
                env,
                in_type,
                *is_class_method,
            )
            .unwrap_or_else(|| format!("`{}::{}()`", ns_type_to_doc(namespace, type_), name)),
            GiDocgen::Callback { namespace, name } => {
                gen_callback_doc_link(&ns_type_to_doc(namespace, name))
            }
            GiDocgen::VFunc {
                namespace,
                type_,
                name,
            } => find_virtual_method_by_name(Some(type_), namespace.as_deref(), name, env, in_type)
                .unwrap_or_else(|| gen_vfunc_doc_link(&ns_type_to_doc(namespace, type_), name)),
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
                "alias" => Ok(Self::Alias(link_details.to_string())),
                "class" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "class")?;
                    Ok(Self::Class { namespace, type_ })
                }
                "const" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "const")?;
                    Ok(Self::Const { namespace, type_ })
                }
                "ctor" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "ctor", false)?;
                    Ok(Self::Constructor {
                        namespace,
                        type_: type_
                            .ok_or_else(|| GiDocgenError::BrokenLinkType("ctor".to_string()))?,
                        name,
                    })
                }
                "enum" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "enum")?;
                    Ok(Self::Enum { namespace, type_ })
                }
                "error" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "error")?;
                    Ok(Self::Error { namespace, type_ })
                }
                "flags" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "flags")?;
                    Ok(Self::Flag { namespace, type_ })
                }
                "func" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "func", true)?;
                    Ok(Self::Func {
                        namespace,
                        type_,
                        name,
                    })
                }
                "iface" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "iface")?;
                    Ok(Self::Interface { namespace, type_ })
                }
                "callback" => {
                    let (namespace, name) = namespace_type_from_details(link_details, "callback")?;
                    Ok(Self::Callback { namespace, name })
                }
                "method" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "method", false)?;
                    let type_ =
                        type_.ok_or_else(|| GiDocgenError::BrokenLinkType("method".to_string()))?;
                    Ok(Self::Method {
                        namespace,
                        is_class_method: type_.ends_with("Class"),
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
                        Ok(Self::Property {
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
                        Ok(Self::Signal {
                            namespace,
                            type_: type_details[0].to_string(),
                            name: type_details[1].to_string(),
                        })
                    }
                }
                "struct" => {
                    let (namespace, type_) = namespace_type_from_details(link_details, "struct")?;
                    Ok(Self::Struct { namespace, type_ })
                }
                "vfunc" => {
                    let (namespace, type_, name) =
                        namespace_type_method_from_details(link_details, "vfunc", false)?;
                    Ok(Self::VFunc {
                        namespace,
                        type_: type_
                            .ok_or_else(|| GiDocgenError::BrokenLinkType("vfunc".to_string()))?,
                        name,
                    })
                }
                "id" => Ok(Self::Id(link_details.to_string())),
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
        );
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
                is_class_method: false,
            })
        );

        assert_eq!(
            GiDocgen::from_str("[method@WidgetClass.add_binding]"),
            Ok(GiDocgen::Method {
                namespace: None,
                type_: "WidgetClass".to_string(),
                name: "add_binding".to_string(),
                is_class_method: true,
            })
        );
    }
}
