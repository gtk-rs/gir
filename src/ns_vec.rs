use std::marker::PhantomData;
use std::ops::{Deref, Index, IndexMut};

use analysis::namespaces::NsId;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id {
    pub ns_id: NsId,
    pub id: u32,
}

pub struct NsIds<I> {
    pos: Id,
    len: u32,
    _dummy: PhantomData<I>,
}

impl<I> Iterator for NsIds<I>
where I: Deref<Target = Id> + From<Id> {
    type Item = I;

    fn next(&mut self) -> Option<I> {
        if self.pos.id < self.len {
            let ret = I::from(self.pos);
            self.pos.id += 1;
            Some(ret)
        }
        else {
            None
        }
    }
}

pub struct NsVec<I, T> {
    data: Vec<Vec<T>>,
    _dummy: PhantomData<I>,
}

impl<I, T> NsVec<I, T>
where I: Deref<Target = Id> + From<Id> {
    pub fn new(ns_count: usize) -> Self {
        let mut data = Vec::with_capacity(ns_count);
        for _ in 0..ns_count {
            data.push(Vec::new());
        }
        NsVec { data: data, _dummy: PhantomData }
    }

    pub fn push(&mut self, ns_id: u16, val: T) -> I {
        let id = self.data[ns_id as usize].len();
        self.data[ns_id as usize].push(val);
        assert!(id < u32::max_value() as usize);
        I::from(Id { ns_id: ns_id, id: id as u32 })
    }

    pub fn ids_by_ns(&self, ns_id: NsId) -> NsIds<I> {
        NsIds {
            pos: Id { ns_id: ns_id, id: 0 },
            len: self.data[ns_id as usize].len() as u32,
            _dummy: PhantomData,
        }
    }
}

impl<I, T> Index<I> for NsVec<I, T>
where I: Deref<Target = Id> {
    type Output = T;

    fn index(&self, index: I) -> &T {
        &self.data[index.ns_id as usize][index.id as usize]
    }
}

impl<I, T> IndexMut<I> for NsVec<I, T>
where I: Deref<Target = Id> {
    fn index_mut(&mut self, index: I) -> &mut T {
        &mut self.data[index.ns_id as usize][index.id as usize]
    }
}
