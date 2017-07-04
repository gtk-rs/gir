use library;

pub enum RecordType {
    Direct,
    Boxed,
    Refcounted,
}

impl RecordType {
    pub fn of(record: &library::Record) -> RecordType {
        let mut has_copy = false;
        let mut has_free = false;
        let mut has_ref = false;
        let mut has_unref = false;
        let mut has_destroy = false;
        for func in &record.functions {
            match &func.name[..] {
                "copy" => has_copy = true,
                "free" => has_free = true,
                "destroy" => has_destroy = true,
                "ref" => has_ref = true,
                "unref" => has_unref = true,
                _ => (),
            }
        }

        if has_destroy && has_copy {
            has_free = true;
        }

        if has_copy && has_free {
            RecordType::Boxed
        } else if has_ref && has_unref {
            RecordType::Refcounted
        } else {
            RecordType::Direct
        }
    }
}
