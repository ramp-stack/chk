use prism::Context;
use ptsd::Validate;

use std::{rc::Rc, cell::RefCell};

use crate::design_system::{Icons, Theme};
use crate::Display;
use crate::form::{FormValues, FormValidState, FormComplete};

pub type NavFnInner = Rc<RefCell<dyn FnMut(&mut Context, &Theme)>>;

pub trait ValidityFn: Validate<String, FormValidState> {}
impl<V: Validate<String, FormValidState>> ValidityFn for V {}
dyn_clone::clone_trait_object!(ValidityFn);

impl std::fmt::Debug for dyn ValidityFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "Validity check...")}
}

pub trait ReviewItemGetter: FnMut(&FormValues) -> Vec<Display> + dyn_clone::DynClone + 'static {}
impl<F: FnMut(&FormValues) -> Vec<Display> + Clone + 'static> ReviewItemGetter for F {}
dyn_clone::clone_trait_object!(ReviewItemGetter);

impl std::fmt::Debug for dyn ReviewItemGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReviewItemGetter")
    }
}

pub trait SuccessGetter: FnMut(FormValues) -> (Icons, String) + dyn_clone::DynClone + 'static {}
impl<F: FnMut(FormValues) -> (Icons, String) + Clone + 'static> SuccessGetter for F {}
dyn_clone::clone_trait_object!(SuccessGetter);

impl std::fmt::Debug for dyn SuccessGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "SuccessGetter")}
}

pub trait FormSubmit:
    FnMut(&mut Context, &FormValues) -> FormComplete + dyn_clone::DynClone + 'static {}
impl<F: FnMut(&mut Context, &FormValues) -> FormComplete + Clone + 'static> FormSubmit for F {}
dyn_clone::clone_trait_object!(FormSubmit);

impl std::fmt::Debug for dyn FormSubmit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormSubmit Closure")
    }
}

#[derive(Clone)]
pub struct NavFn(pub NavFnInner);

impl PartialEq for NavFn {
    fn eq(&self, other: &Self) -> bool {Rc::ptr_eq(&self.0, &other.0)}
}

impl std::ops::Deref for NavFn {
    type Target = NavFnInner;
    fn deref(&self) -> &NavFnInner {&self.0}
}

impl std::fmt::Debug for NavFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "NavFN")}
}
