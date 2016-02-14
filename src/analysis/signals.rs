use library;
use nameutil;

#[derive(Debug)]
pub struct Info {
    pub connect_name: String,
    pub signal_name: String,
}

pub fn analyze(signals: &[library::Signal]) -> Vec<Info> {
    let mut sns = Vec::new();

    for signal in signals {
        let info = analyze_signal(signal);
        if let Some(info) = info {
            sns.push(info);
        }
    }

    sns
}

fn analyze_signal(signal: &library::Signal) -> Option<Info> {
    let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));

    let info = Info {
        connect_name: connect_name,
        signal_name: signal.name.clone(),
    };
    Some(info)
}
