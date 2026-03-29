use std::{ops::Deref, rc::Rc, sync::Arc};

pub trait CloneDeref: Clone + Deref{}

impl<T> CloneDeref for Rc<T> {}
impl<T> CloneDeref for Arc<T> {}