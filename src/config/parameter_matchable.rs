use super::{ident::Ident, matchable::Matchable};

pub trait Functionlike {
    type Parameter;

    fn parameters(&self) -> &[Self::Parameter];

    // TODO: result
}

pub trait ParameterMatchable {
    type Parameter;

    fn matched_parameters(&self, parameter_name: &str) -> Vec<&Self::Parameter>;
}

impl<'a, U: AsRef<Ident>, T: Functionlike<Parameter = U>> ParameterMatchable for [&'a T] {
    type Parameter = U;

    fn matched_parameters(&self, parameter_name: &str) -> Vec<&Self::Parameter> {
        let mut v = Vec::new();
        for f in self.iter() {
            let pars = f.parameters().matched(parameter_name);
            v.extend_from_slice(&pars);
        }
        v
    }
}
