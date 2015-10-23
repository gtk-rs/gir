use std::marker::PhantomData;
use std::ops::{Deref, Index, IndexMut};
use std::slice::Iter;

use analysis::namespaces::NsId;

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id {
    ns_id: NsId,
    id: u32,
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

    pub fn ns_iter(&self, ns_id: u16) -> Iter<T> {
        self.data[ns_id as usize].iter()
    }
}

impl<I, T> Index<Id> for NsVec<I, T> 
where I: Deref<Target = Id> {
    type Output = T;

    fn index(&self, index: Id) -> &T {
        &self.data[index.ns_id as usize][index.id as usize]
    }
}

impl<I, T> IndexMut<Id> for NsVec<I, T> 
where I: Deref<Target = Id> {
    fn index_mut(&mut self, index: Id) -> &mut T {
        &mut self.data[index.ns_id as usize][index.id as usize]
    }
}
