use ramp::prism;
use pelican_ui::event::{OnEvent, Event, TickEvent};
use pelican_ui::drawable::{Component, Drawable, SizedTree};
use pelican_ui::{Request, drawables, Context, Callback};
use pelican_ui::layout::{Stack, Offset};
use pelican_ui::canvas::Align;
use pelican_ui::components::TextInput;
use pelican_ui::components::avatar::{Avatar, AvatarSize, AvatarContent, AvatarIconStyle};
use pelican_ui::components::list_item::{ListItemGroup, ListItem, ListItemInfoLeft};
use pelican_ui::navigation::NavigationEvent;
use pelican_ui::interface::general::{Header, Content, Bumper as PelicanBumper, Page as PelicanPage};
use pelican_ui::navigation::{AppPage, Flow as PelicanFlow};
use pelican_ui::components::text::{ExpandableText, TextSize, TextStyle};
use pelican_ui::theme::{Theme, Icons};
use pelican_ui::components::MessageGroups;
use std::fmt::Debug;
use pelican_ui::utils::{ValidationFn, Timestamp};

use crate::{FlowWrapper, PageType, FormItem, EditPage, Bumper};
use crate::flow::{Flow, State};
use crate::items::{Action, Input, Display};
use crate::closure::{FormSubmit, NavFn, ScreenBuilder, PageBuilder, ReviewItemGetter, SuccessGetter};

use air::names::{Secret, Id, Name};
use air::contract::{Contracts, Contract, Substance, Reactants, Reactant, Beaker};

use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use std::convert::Infallible;
use std::str::FromStr;

use std::sync::Arc;
use rand::{seq::SliceRandom, Rng};
use std::fs;

use serde::{Serialize, Deserialize};

#[derive(Debug, Component, Clone)]
pub struct EditPage(Stack, pub PelicanPage, #[skip] Theme, #[skip] Box<dyn FormSubmit>, #[skip] Vec<State>);
impl OnEvent for EditPage {}
impl AppPage for EditPage {}
impl EditPage {
    pub fn new(theme: &Theme, title: String, input: Vec<Input>, display: Vec<Display>, validations: Vec<Box<dyn ValidationFn>>, on_save: Box<dyn FormSubmit>) -> Self {
        let header = Header::stack(theme, &title, None);
        let mut content = input.into_iter().flat_map(|i| i.build(theme)).flatten().collect::<Vec<_>>();
        display.into_iter().for_each(|mut d| if let Some(r) = d.build(theme) {content.extend(r)});

        let validation = Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
            validations.clone().into_iter().enumerate().any(|(i, mut validation)| {
                let child = vec![&mut *children[i]];
                !(validation)(ctx, child)
            })
        }) as Box<dyn ValidationFn>;
        
        let bumper = PelicanBumper::stack(theme, Some("Save"), Box::new(|_: &mut Context, _: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validation), Some(bumper));

        EditPage(Stack::default(), page, theme.clone(), on_save.clone(), vec![])
    }

    pub fn edit_and_display(theme: &Theme, title: String, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        let header = Header::stack(theme, &title, None);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let mut content = inputs.into_iter().flat_map(|i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        display.into_iter().for_each(|mut d| if let Some(r) = d.build(theme) {content.extend(r)});


        let validation = Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
            validations.clone().into_iter().enumerate().any(|(i, mut validation)| {
                let child = vec![&mut *children[i]];
                !(validation)(ctx, child)
            })
        }) as Box<dyn ValidationFn>;
        
        let bumper = PelicanBumper::stack(theme, Some("Save"), Box::new(|_: &mut Context, _: &Theme| {}), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validation), Some(bumper));

        EditPage(Stack::default(), page, theme.clone(), on_save.clone(), vec![])
    }

    pub fn root(theme: &Theme, title: String, items: Vec<FormItem>, display: Vec<Display>, on_save: Box<dyn FormSubmit>) -> Self {
        let header = Header::home(theme, &title, None);
        let validations = items.iter().map(|i| i.validation()).collect::<Vec<_>>();
        let inputs = items.into_iter().map(|i| i.build()).collect::<Vec<Input>>();
        let mut content = inputs.into_iter().flat_map(|i| i.build(theme)).flatten().collect::<Vec<Box<dyn Drawable>>>();
        display.into_iter().for_each(|mut d| if let Some(r) = d.build(theme) {content.extend(r)});

        let validation = Box::new(move |ctx: &mut Context, mut children: Vec<&mut Box<dyn Drawable>>| {
            validations.clone().into_iter().enumerate().any(|(i, mut validation)| {
                let child = vec![&mut *children[i]];
                !(validation)(ctx, child)
            })
        }) as Box<dyn ValidationFn>;
        
        let bumper = PelicanBumper::home(theme, Some(("Save".to_string(), Box::new(|_: &mut Context, _: &Theme| {}))), None);
        let page = PelicanPage::new(header, Content::new(Offset::Start, content, validation), Some(bumper));

        EditPage(Stack::default(), page, theme.clone(), on_save.clone(), vec![])
    }

    pub fn on_change(&mut self, new: Vec<State>) {
        println!("On change");
        if new != self.4 {
            self.4 = new.clone();
            let theme = &self.2;
            let mut on_save = self.3.clone();
            let closure = Box::new(move |ctx: &mut Context, _theme: &Theme| {(on_save)(ctx, &new);});
            self.1.bumper = Some(PelicanBumper::stack(theme, Some("Save"), closure, None));
        }
    }
}
