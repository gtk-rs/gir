pub const TAB: &str = "    ";
pub const TAB_SIZE: usize = 4;
pub const MAX_TEXT_WIDTH: usize = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs() {
        assert_eq!(TAB_SIZE, TAB.len());
    }
}
