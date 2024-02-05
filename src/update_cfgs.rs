use log::warn;

pub fn get_cfg_condition(
    object_name: &str,
    cfg_condition: &Option<String>,
    identifier_prefixes: &[String],
) -> Option<String> {
    let sub_object_name = identifier_prefixes
        .iter()
        .filter_map(|prefix| object_name.strip_prefix(prefix))
        .find_map(|name| name.strip_prefix('_'))
        .unwrap_or(object_name);

    if sub_object_name.starts_with("win32_") {
        match cfg_condition.as_deref() {
            Some("windows") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `windows` if name starts with `win32_`");
                Some("windows".to_string())
            }
            None => Some("windows".to_string()),
            Some(cfg) => Some(format!("{cfg},windows")),
        }
    } else if sub_object_name.starts_with("unix_") {
        match cfg_condition.as_deref() {
            Some("unix") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `unix` if name starts with `unix_`");
                Some("unix".to_string())
            }
            None => Some("unix".to_string()),
            Some(cfg) => Some(format!("{cfg},unix")),
        }
    } else {
        cfg_condition.clone()
    }
}

pub fn get_object_cfg_condition(
    object_name: &str,
    cfg_condition: &Option<String>,
    identifier_prefixes: &[String],
) -> Option<String> {
    let sub_object_name = identifier_prefixes
        .iter()
        .filter_map(|prefix| object_name.strip_prefix(prefix))
        .find(|name| {
            name.chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
        })
        .unwrap_or(object_name);

    if sub_object_name.starts_with("Win32") {
        match cfg_condition.as_deref() {
            Some("windows") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `windows` if object name starts with `Win32`");
                Some("windows".to_string())
            }
            None => Some("windows".to_string()),
            Some(cfg) => Some(format!("{cfg},windows")),
        }
    } else if sub_object_name.starts_with("Unix") || sub_object_name.starts_with("UNIX") {
        match cfg_condition.as_deref() {
            Some("unix") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `unix` if object name starts with `Unix`");
                Some("unix".to_string())
            }
            None => Some("unix".to_string()),
            Some(cfg) => Some(format!("{cfg},unix")),
        }
    } else {
        cfg_condition.clone()
    }
}

pub fn get_constant_cfg_condition(
    const_name: &str,
    cfg_condition: &Option<String>,
) -> Option<String> {
    if const_name.starts_with("WIN32_") {
        match cfg_condition.as_deref() {
            Some("windows") => {
                warn!("\"object {const_name}\": No need to set `cfg_condition` to `windows` if name starts with `WIN32_`");
                Some("windows".to_string())
            }
            None => Some("windows".to_string()),
            Some(cfg) => Some(format!("{cfg},windows")),
        }
    } else if const_name.starts_with("UNIX_") {
        match cfg_condition.as_deref() {
            Some("unix") => {
                warn!("\"object {const_name}\": No need to set `cfg_condition` to `unix` if name starts with `UNIX_`");
                Some("unix".to_string())
            }
            None => Some("unix".to_string()),
            Some(cfg) => Some(format!("{cfg},unix")),
        }
    } else {
        cfg_condition.clone()
    }
}
