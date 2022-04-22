use crate::analysis::ref_mode::RefMode;

impl RefMode {
    pub(crate) fn for_rust_type(self) -> &'static str {
        match self {
            RefMode::None | RefMode::ByRefFake => "",
            RefMode::ByRef | RefMode::ByRefImmut | RefMode::ByRefConst => "&",
            RefMode::ByRefMut => "&mut ",
        }
    }
}
