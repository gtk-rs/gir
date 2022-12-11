use crate::library;

#[derive(PartialEq, Eq)]
pub enum RecordType {
    /// Boxed record that use g_boxed_copy, g_boxed_free.
    /// Must have glib_get_type function
    AutoBoxed,
    /// Boxed record with custom copy/free functions
    Boxed,
    /// Referencecounted record
    Refcounted,
    // TODO: detect and generate direct records
    // Direct,
}

impl RecordType {
    pub fn of(record: &library::Record) -> RecordType {
        if record.has_ref() && record.has_unref() {
            RecordType::Refcounted
        } else if record.has_copy() && record.has_free() {
            RecordType::Boxed
        } else {
            RecordType::AutoBoxed
        }
    }
}
