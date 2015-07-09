use env::Env;

mod functions;
mod statics;

pub fn generate(env: &Env) {
    functions::generate(env);
}
