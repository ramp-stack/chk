use prism::{Context, air::{Contract, Instance}};
use ptsd::Validate;
use std::{rc::Rc, cell::RefCell};
use crate::design_system::{Icons, Theme};
use crate::page::PageType;
use crate::Display;
use crate::form::{FormValues, FormValidState, FormComplete};

pub type NavFnInner = Rc<RefCell<dyn FnMut(&mut Context, &Theme)>>;

#[derive(Clone)]
pub struct NavFn(pub NavFnInner);

impl PartialEq for NavFn {
    fn eq(&self, other: &Self) -> bool {Rc::ptr_eq(&self.0, &other.0)}
}

impl std::ops::Deref for NavFn {
    type Target = NavFnInner;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl std::fmt::Debug for NavFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "NavFN")}
}

pub trait ValidityFn: Validate<String, FormValidState> {}
impl<V: Validate<String, FormValidState>> ValidityFn for V {}
dyn_clone::clone_trait_object!(ValidityFn);

impl PartialEq for dyn ValidityFn {fn eq(&self, _: &Self) -> bool {true}}

impl std::fmt::Debug for dyn ValidityFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Valitidy check...")
    }
}

pub trait ReviewItemGetter: FnMut(&FormValues) -> Vec<Display> + 'static {
    fn clone_box(&self) -> Box<dyn ReviewItemGetter>;
}

impl<F: FnMut(&FormValues) -> Vec<Display> + Clone + 'static> ReviewItemGetter for F {
    fn clone_box(&self) -> Box<dyn ReviewItemGetter> {Box::new(self.clone())}
}

impl Clone for Box<dyn ReviewItemGetter> {
    fn clone(&self) -> Self {self.as_ref().clone_box()}
}

impl std::fmt::Debug for dyn ReviewItemGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReviewItemGetter")
    }
}

pub trait SuccessGetter: FnMut(FormValues) -> (Icons, String) + 'static {
    fn clone_box(&self) -> Box<dyn SuccessGetter>;
}

impl<F: FnMut(FormValues) -> (Icons, String) + Clone + 'static> SuccessGetter for F {
    fn clone_box(&self) -> Box<dyn SuccessGetter> {Box::new(self.clone())}
}

impl Clone for Box<dyn SuccessGetter> {
    fn clone(&self) -> Self {self.as_ref().clone_box()}
}

impl std::fmt::Debug for dyn SuccessGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {write!(f, "SuccessGetter")}
}

pub trait FormSubmit: FnMut(&mut Context, &FormValues) -> FormComplete + 'static {
    fn clone_box(&self) -> Box<dyn FormSubmit>;
}

impl PartialEq for dyn FormSubmit {fn eq(&self, _: &Self) -> bool {true}}

impl<F: FnMut(&mut Context, &FormValues) -> FormComplete + Clone + 'static> FormSubmit for F {
    fn clone_box(&self) -> Box<dyn FormSubmit> {Box::new(self.clone())}
}

impl Clone for Box<dyn FormSubmit> {
    fn clone(&self) -> Self {self.as_ref().clone_box()}
}

impl std::fmt::Debug for dyn FormSubmit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormSubmit Closure")
    }
}

pub trait PageBuilderContractFn<C: Contract>: FnMut(&mut Context, &Theme, Instance<C>) -> PageType + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilderContractFn<C>>;
}

impl<C: Contract, F: FnMut(&mut Context, &Theme, Instance<C>) -> PageType + Clone + 'static> PageBuilderContractFn<C> for F {
    fn clone_box(&self) -> Box<dyn PageBuilderContractFn<C>> {Box::new(self.clone())}
}

impl<C: Contract> Clone for Box<dyn PageBuilderContractFn<C>> {
    fn clone(&self) -> Self {self.as_ref().clone_box()}
}

impl<C: Contract> std::fmt::Debug for dyn PageBuilderContractFn<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageBuilderContractFn")
    }
}

pub trait PageBuilderContractMultiplesFn<C: Contract>: FnMut(&mut Context, &Theme, Vec<Instance<C>>) -> PageType + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilderContractMultiplesFn<C>>;
}

impl<C: Contract, F: FnMut(&mut Context, &Theme, Vec<Instance<C>>) -> PageType + Clone + 'static> PageBuilderContractMultiplesFn<C> for F {
    fn clone_box(&self) -> Box<dyn PageBuilderContractMultiplesFn<C>> {Box::new(self.clone())}
}

impl<C: Contract> Clone for Box<dyn PageBuilderContractMultiplesFn<C>> {
    fn clone(&self) -> Self {self.as_ref().clone_box()}
}

impl<C: Contract> std::fmt::Debug for dyn PageBuilderContractMultiplesFn<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageBuilderContractMultiplesFn")
    }
}
