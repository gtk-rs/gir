use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::slice::Iter;

use stopwatch::Stopwatch;

macro_rules! iterated_enum {
    ($name: ident ; $num: expr => [ $($thing: ident),* ] ) => {
        #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($thing),*
        }

        impl $name {
            fn iter() -> Iter<'static, $name> {
                use self::$name::*;
                static THINGS: [$name; $num] = [ $($thing),* ];
                THINGS.into_iter()
            }
        }
    }
}

iterated_enum! {
    SWType ; 3 => [
        Total,
        Loading,
        Generating
    ]
}

pub struct Watcher {
    stopwatch: Rc<RefCell<Stopwatch>>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stopwatch.borrow_mut().stop();
    }
}

#[derive(Default)]
pub struct Statistics {
    stopwatches:  HashMap<SWType, Rc<RefCell<Stopwatch>>>,
}

impl Statistics {
    pub fn new() -> Statistics {
        Default::default()
    }

    pub fn start(&mut self, typ: SWType) -> Watcher {
        let stopwatch = self.stopwatches.entry(typ).or_insert(Default::default());
        stopwatch.borrow_mut().start();
        Watcher{ stopwatch: stopwatch.clone() }
    }

    pub fn print(&self) {
        for typ in SWType::iter() {
            if let Some(sw) = self.stopwatches.get(typ) {
                let sw = sw.borrow();
                let elapsed = sw.elapsed();
                let elapsed_ms = sw.elapsed_ms();
                let typ_str = format!("{:?}", typ);
                let elapsed_str = format!("{}", elapsed);
                let elapsed_ms_str = format!("{}", elapsed_ms);
                println!("{:20} {:>20} {:>20}", typ_str, elapsed_str, elapsed_ms_str);
            }
        }
    }
}
