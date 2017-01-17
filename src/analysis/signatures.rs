use std::collections::HashMap;

use env::Env;
use library;
use version::Version;

#[derive(Debug)]
pub struct Signature(Vec<library::TypeId>, library::TypeId, Option<Version>);

impl Signature {
    pub fn new(func: &library::Function) -> Signature {
        let params = func.parameters.iter().map(|p| p.typ).collect();
        Signature(params, func.ret.typ, func.version)
    }

    pub fn has_in_deps(&self, env: &Env, name: &String, deps: &[library::TypeId]) -> (bool, Option<Version>) {
        for tid in deps {
            let full_name = tid.full_name(&env.library);
            if let Some(info) = env.analysis.objects.get(&full_name) {
                if let Some(params) = info.signatures.get(name) {
                    if params.1 == self.1 && params.0[1..] == self.0[1..] {
                        return (true, params.2);
                    }
                }
            }
        }
        (false, None)
    }
}

pub type Signatures = HashMap<String, Signature>;
