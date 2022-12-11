use std::collections::HashMap;

use crate::{env::Env, library, version::Version};

#[derive(Debug)]
pub struct Signature(Vec<library::TypeId>, library::TypeId, Option<Version>);

impl Signature {
    pub fn new(func: &library::Function) -> Self {
        let params = func.parameters.iter().map(|p| p.typ).collect();
        Self(params, func.ret.typ, func.version)
    }

    fn from_property(is_get: bool, typ: library::TypeId) -> Self {
        if is_get {
            Self(vec![Default::default()], typ, None)
        } else {
            Self(vec![Default::default(), typ], Default::default(), None)
        }
    }

    pub fn has_in_deps(
        &self,
        env: &Env,
        name: &str,
        deps: &[library::TypeId],
    ) -> (bool, Option<Version>) {
        for tid in deps {
            let full_name = tid.full_name(&env.library);
            if let Some(info) = env.analysis.objects.get(&full_name) {
                if let Some(signature) = info.signatures.get(name) {
                    if self.eq(signature) {
                        return (true, signature.2);
                    }
                }
            }
        }
        (false, None)
    }

    pub fn has_for_property(
        env: &Env,
        name: &str,
        is_get: bool,
        typ: library::TypeId,
        signatures: &Signatures,
        deps: &[library::TypeId],
    ) -> (bool, Option<Version>) {
        if let Some(params) = signatures.get(name) {
            return (true, params.2);
        }
        let this = Signature::from_property(is_get, typ);
        for tid in deps {
            let full_name = tid.full_name(&env.library);
            if let Some(info) = env.analysis.objects.get(&full_name) {
                if let Some(signature) = info.signatures.get(name) {
                    if this.property_eq(signature, is_get) {
                        return (true, signature.2);
                    }
                }
            }
        }
        (false, None)
    }

    fn eq(&self, other: &Signature) -> bool {
        other.1 == self.1 && other.0[1..] == self.0[1..]
    }

    fn property_eq(&self, other: &Signature, is_get: bool) -> bool {
        if self.eq(other) {
            true
        } else {
            // For getters for types like GdkRGBA
            is_get && other.0.len() == 2 && other.0[1] == self.1
        }
    }
}

pub type Signatures = HashMap<String, Signature>;
