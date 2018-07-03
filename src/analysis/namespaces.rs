use std::ops::Index;

use library;
use nameutil;
use version::Version;

pub type NsId = u16;
pub const MAIN: NsId = library::MAIN_NAMESPACE;
pub const INTERNAL: NsId = library::INTERNAL_NAMESPACE;

#[derive(Debug)]
pub struct Namespace {
    pub name: String,
    pub crate_name: String,
    pub ffi_crate_name: String,
    pub higher_crate_name: String,
    pub package_name: Option<String>,
    pub shared_libs: Vec<String>,
    pub versions: Vec<Version>,
}

#[derive(Debug)]
pub struct Info {
    namespaces: Vec<Namespace>,
    pub is_glib_crate: bool,
    pub glib_ns_id: NsId,
}

impl Info {
    pub fn main(&self) -> &Namespace {
        &self[MAIN]
    }
}

impl Index<NsId> for Info {
    type Output = Namespace;

    fn index(&self, index: NsId) -> &Namespace {
        &self.namespaces[index as usize]
    }
}

pub fn run(gir: &library::Library) -> Info {
    let mut namespaces = Vec::with_capacity(gir.namespaces.len());
    let mut is_glib_crate = false;
    let mut glib_ns_id = None;
    let mut is_gobject = false;

    for (ns_id, ns) in gir.namespaces.iter().enumerate() {
        let ns_id = ns_id as NsId;
        let crate_name = nameutil::crate_name(&ns.name);
        let mut is_local_ffi = ns_id == MAIN;
        if is_local_ffi && ns.name == "GObject" {
            is_gobject = true;
            is_local_ffi = false;
            is_glib_crate = true;
        } else if is_gobject && ns.name == "GLib" {
            is_local_ffi = true;
        }
        let ffi_crate_name = if is_local_ffi {
            "ffi".to_owned()
        } else {
            format!("{}_ffi", crate_name)
        };
        let higher_crate_name = match &crate_name[..] {
            "gobject" => "glib".to_owned(),
            _ => crate_name.clone(),
        };
        namespaces.push(Namespace {
            name: ns.name.clone(),
            crate_name,
            ffi_crate_name,
            higher_crate_name,
            package_name: ns.package_name.clone(),
            shared_libs: ns.shared_library.clone(),
            versions: ns.versions.iter().cloned().collect(),
        });
        if ns.name == "GLib" {
            glib_ns_id = Some(ns_id);
            if ns_id == MAIN {
                is_glib_crate = true;
            }
        }
    }

    Info {
        namespaces,
        is_glib_crate,
        glib_ns_id: glib_ns_id.expect("Missing `GLib` namespace!"),
    }
}
