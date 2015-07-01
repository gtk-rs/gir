use env::Env;

mod function;
mod general;
mod return_value;
mod widget;
mod widgets;

pub fn generate(env: &Env) {
    widgets::generate(env);
}
