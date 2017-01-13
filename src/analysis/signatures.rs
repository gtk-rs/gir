use std::collections::HashMap;

use env::Env;
use library;

#[derive(Debug)]
pub struct SignatureParams(Vec<library::TypeId>, library::TypeId);

impl SignatureParams {
    pub fn new(func: &library::Function) -> SignatureParams {
        let params = func.parameters.iter().map(|p| p.typ).collect();
        SignatureParams(params, func.ret.typ)
    }

    pub fn has_in_deps(&self, env: &Env, name: &String, deps: &[library::TypeId]) -> bool {
        for tid in deps {
            let full_name = tid.full_name(&env.library);
            if let Some(info) = env.analysis.objects.get(&full_name) {
                if let Some(params) = info.signatures.get(name) {
                    return params.1 == self.1 && params.0[1..] == self.0[1..];
                }
            }
        }
        false
    }
}

pub type Signatures = HashMap<String, SignatureParams>;
