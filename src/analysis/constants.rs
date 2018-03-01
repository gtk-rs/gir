use std::borrow::Borrow;

use config;
use env::Env;
use library;
use traits::*;
use nameutil;
use version::Version;

#[derive(Debug)]
pub struct Info {
    pub name: String,
    pub glib_name: String,
    pub typ: library::TypeId,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub cfg_condition: Option<String>,
}

pub fn analyze<F: Borrow<library::Constant>>(
    env: &Env,
    constants: &[F],
    obj: &config::gobjects::GObject,
) -> Vec<Info> {
    let mut consts = Vec::new();

    for constant in constants {
        let constant = constant.borrow();
        let configured_constants = obj.constants.matched(&constant.name);

        if configured_constants.iter().any(|c| c.ignore) {
            continue;
        }

        if env.is_totally_deprecated(constant.deprecated_version) {
            continue;
        }

        match *env.type_(constant.typ) {
            library::Type::Fundamental(library::Fundamental::Utf8) => (),
            _ => continue,
        }

        let version = configured_constants
            .iter()
            .filter_map(|c| c.version)
            .min()
            .or(constant.version);
        let version = env.config.filter_version(version);
        let deprecated_version = constant.deprecated_version;
        let cfg_condition = configured_constants
            .iter()
            .filter_map(|c| c.cfg_condition.clone())
            .next();

        let name = nameutil::mangle_keywords(&*constant.name).into_owned();

        consts.push(Info {
            name: name,
            glib_name: constant.c_identifier.clone(),
            typ: constant.typ,
            version: version,
            deprecated_version: deprecated_version,
            cfg_condition: cfg_condition,
        });
    }

    consts
}
