use std::vec::Vec;

use library::*;

pub type Info = Vec<Parameter>;

pub fn analyze(type_: &Function) -> Info {
    let mut outs = Info::new();
    //Only process out parameters if function returns None
    if type_.ret.typ != Default::default() { return outs; }

    for par in &type_.parameters {
        if par.direction.can_as_return() {
            outs.push(par.clone());
        }
    }
    outs
}
