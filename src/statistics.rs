use std::collections::HashMap;
use std::slice::Iter;

use stopwatch::Stopwatch;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SWType {
    Empty,
    Total,
    Loading,
    Generating,
}

impl SWType {
    pub fn iterator() -> Iter<'static, SWType> {
        use self::SWType::*;
        static ITEMS: [SWType;  3] = [
            Total,
            Loading,
            Generating,
        ];
        ITEMS.into_iter()
    }
}

impl Default for SWType {
    fn default() -> SWType {
        SWType::Empty
    }
}

#[derive(Default)]
pub struct Statistics {
    stopwatches:  HashMap<SWType, Stopwatch>,
}

impl Statistics {
    pub fn new() -> Statistics {
        Default::default()
    }

    pub fn start(&mut self, typ: SWType) {
        let mut sw = self.stopwatches.entry(typ).or_insert(Default::default());
        sw.start();
    }

    pub fn stop(&mut self, typ: SWType) {
        let mut sw = self.stopwatches.entry(typ).or_insert(Default::default());
        sw.stop();
    }

    pub fn print(&mut self) {
        for typ in SWType::iterator() {
            let sw = self.stopwatches.entry(*typ).or_insert(Default::default());
            let elapsed = sw.elapsed();
            let elapsed_ms = sw.elapsed_ms();
            let typ_str = format!("{:?}", typ);
            let elapsed_str = format!("{}", elapsed);
            let elapsed_ms_str = format!("{}", elapsed_ms);
            println!("{:20} {:>20} {:>20}", typ_str, elapsed_str, elapsed_ms_str);
        }
    }
}
