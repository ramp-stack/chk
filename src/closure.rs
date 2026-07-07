#![allow(clippy::type_complexity)]
use pelican_ui::{Context, theme::{Icons}};
use crate::page::{PageType};
use crate::{ListItem, FormStorage, Display};
use crate::flow::Flow;
use crate::form::{State, FormValidState, FormComplete};
use pelican_ui::theme::Theme;
use pelican_ui::navigation::AppPage;

use air::{Contract, Instance};

use std::rc::Rc;
use std::cell::RefCell;

pub trait FnMutClone: FnMut(&mut Context) + 'static {
    fn clone_box(&self) -> Box<dyn FnMutClone>;
}

impl PartialEq for dyn FnMutClone{fn eq(&self, _: &Self) -> bool {true}}

impl<F> FnMutClone for F where F: FnMut(&mut Context) + Clone + 'static {
    fn clone_box(&self) -> Box<dyn FnMutClone> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FnMutClone> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn FnMutClone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Clonable Closure")
    }
}

pub trait ValidityFn: FnMut(&mut Context, String) -> FormValidState + 'static {
    fn clone_box(&self) -> Box<dyn ValidityFn>;
}

impl<F> ValidityFn for F where F: FnMut(&mut Context, String) -> FormValidState + Clone + 'static {
    fn clone_box(&self) -> Box<dyn ValidityFn> {
        Box::new(self.clone())
    }
}

impl PartialEq for dyn ValidityFn{fn eq(&self, _: &Self) -> bool {true}}

impl Clone for Box<dyn ValidityFn> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn ValidityFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Valitidy check...")
    }
}

pub trait EditedFn: FnMut(&mut Context, &mut String) + 'static {
    fn clone_box(&self) -> Box<dyn EditedFn>;
}

impl PartialEq for dyn EditedFn{fn eq(&self, _: &Self) -> bool {true}}

impl<F> EditedFn for F where F: FnMut(&mut Context, &mut String) + Clone + 'static {
    fn clone_box(&self) -> Box<dyn EditedFn> { Box::new(self.clone()) }
}

impl Clone for Box<dyn EditedFn> { fn clone(&self) -> Self { self.as_ref().clone_box() } }

impl std::fmt::Debug for dyn EditedFn { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "EditedFn") } }

pub type NavFnInner = Rc<RefCell<dyn FnMut(&mut Context, &Theme)>>;

#[derive(Clone)]
pub struct NavFn(pub NavFnInner);

impl PartialEq for NavFn {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for NavFn {
    type Target = NavFnInner;
    fn deref(&self) -> &Self::Target { &self.0 }
}


impl std::fmt::Debug for NavFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NavFN")
    }
}

// pub trait PageBuilder: FnMut() -> PageType + 'static {
//     fn clone_box(&self) -> Box<dyn PageBuilder>;
// }

// impl<F> PageBuilder for F where F: FnMut() -> PageType + Clone + 'static {
//     fn clone_box(&self) -> Box<dyn PageBuilder> {
//         Box::new(self.clone())
//     }
// }

// impl Clone for Box<dyn PageBuilder> {
//     fn clone(&self) -> Self {
//         self.as_ref().clone_box()
//     }
// }

// impl std::fmt::Debug for dyn PageBuilder {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "PageBuilder")
//     }
// }


// pub trait ScreenBuilder: FnMut(&mut Context) -> Screen + 'static {
//     fn clone_box(&self) -> Box<dyn ScreenBuilder>;
// }

// impl<F> ScreenBuilder for F where F: FnMut(&mut Context) -> Screen + Clone + 'static {
//     fn clone_box(&self) -> Box<dyn ScreenBuilder> {
//         Box::new(self.clone())
//     }
// }

// impl Clone for Box<dyn ScreenBuilder> {
//     fn clone(&self) -> Self {
//         self.as_ref().clone_box()
//     }
// }

// impl std::fmt::Debug for dyn ScreenBuilder {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "ScreenBuilder")
//     }
// }

pub trait SuccessClosure: FnMut(&mut Context) -> [String; 3] + 'static {
    fn clone_box(&self) -> Box<dyn SuccessClosure>;
}

impl<F> SuccessClosure for F where F: FnMut(&mut Context) -> [String; 3] + Clone + 'static {
    fn clone_box(&self) -> Box<dyn SuccessClosure> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn SuccessClosure> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn SuccessClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuccessClosure")
    }
}


pub trait MutString: FnMut(&mut Context) -> &mut String + 'static {
    fn clone_box(&self) -> Box<dyn MutString>;
}

impl<F> MutString for F where F: FnMut(&mut Context) -> &mut String + Clone + 'static {
    fn clone_box(&self) -> Box<dyn MutString> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn MutString> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn MutString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MutString")
    }
}

pub trait FormClosure: FnMut(&mut FormStorage, String) + 'static {
    fn clone_box(&self) -> Box<dyn FormClosure>;
}

impl<F> FormClosure for F where F: FnMut(&mut FormStorage, String) + Clone + 'static {
    fn clone_box(&self) -> Box<dyn FormClosure> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FormClosure> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn FormClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormClosure")
    }
}

pub trait ReviewItemGetter: FnMut(&Vec<State>) -> Vec<Display> + 'static {
    fn clone_box(&self) -> Box<dyn ReviewItemGetter>;
}

impl<F> ReviewItemGetter for F where F: FnMut(&Vec<State>) -> Vec<Display> + Clone + 'static {
    fn clone_box(&self) -> Box<dyn ReviewItemGetter> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn ReviewItemGetter> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn ReviewItemGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReviewItemGetter")
    }
}

pub trait SuccessGetter: FnMut(Vec<State>) -> (Icons, String) + 'static {
    fn clone_box(&self) -> Box<dyn SuccessGetter>;
}

impl<F> SuccessGetter for F where F: FnMut(Vec<State>) -> (Icons, String) + Clone + 'static {
    fn clone_box(&self) -> Box<dyn SuccessGetter> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn SuccessGetter> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn SuccessGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuccessGetter")
    }
}


pub trait FormSubmit: FnMut(&mut Context, &Vec<State>) -> FormComplete + 'static {
    fn clone_box(&self) -> Box<dyn FormSubmit>;
}

impl PartialEq for dyn FormSubmit{fn eq(&self, _: &Self) -> bool {true}}

impl<F> FormSubmit for F where F: FnMut(&mut Context, &Vec<State>) -> FormComplete + Clone + 'static {
    fn clone_box(&self) -> Box<dyn FormSubmit> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FormSubmit> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn FormSubmit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormSubmit Closure")
    }
}

pub trait ListItemGetter:
    Fn(&mut Context) -> Vec<ListItem> + Send + Sync + 'static
{
}

impl<T> ListItemGetter for T
where
    T: Fn(&mut Context) -> Vec<ListItem> + Send + Sync + 'static
{
}


impl std::fmt::Debug for dyn ListItemGetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ListItemGetter Closure")
    }
}


pub trait FlowBuilder: FnMut(&mut Context, &Theme) -> Flow + 'static {
    fn clone_box(&self) -> Box<dyn FlowBuilder>;
}

impl PartialEq for dyn FlowBuilder{fn eq(&self, _: &Self) -> bool {true}}

impl<F> FlowBuilder for F where F: FnMut(&mut Context, &Theme) -> Flow + Clone + 'static {
    fn clone_box(&self) -> Box<dyn FlowBuilder> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FlowBuilder> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn FlowBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Flow Builder Closure")
    }
}

pub trait PageTypeToAppPageFn: FnMut(&mut Context, &Theme, PageType) -> Box<dyn AppPage> + 'static {
    fn clone_box(&self) -> Box<dyn PageTypeToAppPageFn>;
}

impl PartialEq for dyn PageTypeToAppPageFn{fn eq(&self, _: &Self) -> bool {true}}

impl<F> PageTypeToAppPageFn for F where F: FnMut(&mut Context, &Theme, PageType) -> Box<dyn AppPage> + Clone + 'static {
    fn clone_box(&self) -> Box<dyn PageTypeToAppPageFn> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn PageTypeToAppPageFn> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl std::fmt::Debug for dyn PageTypeToAppPageFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageTypeToAppPageFn Closure")
    }
}

pub trait PageBuilderContractFn<C: Contract>: FnMut(&mut Context, &Theme, Instance<C>) -> PageType + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilderContractFn<C>>;
}

impl<F, C: Contract> PageBuilderContractFn<C> for F where F: FnMut(&mut Context, &Theme, Instance<C>) -> PageType + Clone + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilderContractFn<C>> {
        Box::new(self.clone())
    }
}

impl<C: Contract> Clone for Box<dyn PageBuilderContractFn<C>> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl<C: Contract> std::fmt::Debug for dyn PageBuilderContractFn<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageBuilderContractFn")
    }
}

pub trait PageBuilderContractMultiplesFn<C: Contract>: FnMut(&mut Context, &Theme, Vec<Instance<C>>) -> PageType + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilderContractMultiplesFn<C>>;
}

impl<F, C: Contract> PageBuilderContractMultiplesFn<C> for F where F: FnMut(&mut Context, &Theme, Vec<Instance<C>>) -> PageType + Clone + 'static {
    fn clone_box(&self) -> Box<dyn PageBuilderContractMultiplesFn<C>> {
        Box::new(self.clone())
    }
}

impl<C: Contract> Clone for Box<dyn PageBuilderContractMultiplesFn<C>> {
    fn clone(&self) -> Self {
        self.as_ref().clone_box()
    }
}

impl<C: Contract> std::fmt::Debug for dyn PageBuilderContractMultiplesFn<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageBuilderContractMultiplesFn")
    }
}
