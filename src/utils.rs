use std::cell::{Ref, RefMut};

#[allow(clippy::manual_map)]
pub fn ref_opt_to_opt_ref<T>(r: Ref<Option<T>>) -> Option<Ref<T>> {
    match *r {
        Some(_) => Some(Ref::map(r, |o| o.as_ref().unwrap())),
        None => None,
    }
}

#[allow(clippy::manual_map)]
pub fn refmut_opt_to_opt_refmut<T>(r: RefMut<Option<T>>) -> Option<RefMut<T>> {
    match *r {
        Some(_) => Some(RefMut::map(r, |o| o.as_mut().unwrap())),
        None => None,
    }
}
