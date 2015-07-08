use env::Env;
use config::WorkMode;

mod function;
mod function_body;
mod general;
mod parameter;
mod return_value;
mod sys;
mod translate_from_glib;
mod translate_to_glib;
mod widget;
mod widgets;

pub fn generate(env: &Env) {
    match env.config.work_mode {
        WorkMode::Normal => widgets::generate(env),
        WorkMode::Sys => sys::generate(env),
    }
}
