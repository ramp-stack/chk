use prism::drawable::{Drawable, text::Align};
use ptsd::layout::Offset;
use ptsd::utils::TitleSubtitle;
use uuid::Uuid;
use ::pelican_ui::{Callback as NativeCallback, theme::Theme as NativeTheme};
use ::pelican_ui::components::{self as native, TextInput, RadioSelector, NumericalInput, SelectionInput,
    QRCodeScanner, Keypad, Icon, Alert, AspectRatioImage, DataItem, QRCode};
use ::pelican_ui::components::avatar::{Avatar, AvatarSize};
use ::pelican_ui::components::text::{Text, TextStyle as NativeTextStyle};
use ::pelican_ui::components::button::{ActionData, QuickActions, IconButtonGroup};
use ::pelican_ui::components::list_item::{ListItem as NativeListItem, ListItemInfoLeft, ListItemSection};
use ::pelican_ui::interface::general::{Page as NativePage, Header as NativeHeader, Bumper as NativeFooter, Interface};
use ::pelican_ui::interface::navigation::RootInfo as NativeRoot;

pub use native::{FittedPhoto, AspectFit as Fit, MediaBadge as Badge};

use std::{future::Future, pin::Pin};
use crate::design_system::*;
use crate::{DesignSystem, Control, State};

impl crate::ChkTheme {
    /// Compatibility for existing apps; new apps should use `build`.
    pub fn to_pelican(self, assets: Assets) -> Theme {self.build::<PelicanUi>(assets)}
}

impl Theme {
    pub fn native_pelican(&self) -> &NativeTheme {
        self.downcast_ref::<PelicanUi>().unwrap().native_theme()
    }
}

impl dyn Callback {
    pub fn into_pelican(mut self: Box<Self>, theme: &Theme) -> Box<dyn NativeCallback> {
        let theme = theme.clone();
        Box::new(move |ctx: &mut Context, _: &NativeTheme| self(ctx, &theme))
    }
}

impl crate::ActionPresentation {
    pub fn into_pelican(self, theme: &Theme) -> ActionData {
        ActionData {
            label: self.label, active: self.active, icon: self.icon,
            on_click: self.on_click.into_pelican(theme),
        }
    }
}

impl ProfileView {
    pub fn into_pelican(self) -> native::Profile {
        native::Profile {name: self.name, username: self.username, pfp: self.pfp}
    }
}

impl Header {
    pub fn into_pelican(self, theme: &Theme) -> NativeHeader {
        let native = theme.native_pelican();
        let action = |(icon, action): (_, Box<dyn Callback>)| (icon, action.into_pelican(theme));
        match self {
            Header::Stack {title, action: callback} => NativeHeader::stack(native, &title, callback.map(action)),
            Header::End {title} => NativeHeader::stack_end(native, &title),
            Header::Home {title, action: callback} => NativeHeader::home(native, &title, callback.map(action)),
            Header::Leading {title, action: callback} => NativeHeader::home_leading(native, &title, callback.map(action)),
            Header::Messaging {title, profiles, avatar, steps, info} => NativeHeader::messaging_with_avatar(
                native, title, profiles.into_iter().map(ProfileView::into_pelican).collect(), avatar, steps,
                info.map(|action| action.into_pelican(theme))),
        }
    }
}

impl Footer {
    pub fn into_pelican(self, theme: &Theme) -> NativeFooter {
        let native = theme.native_pelican();
        let action = |(label, action): (_, Box<dyn Callback>)| (label, action.into_pelican(theme));
        match self {
            Footer::Stack {label, action: callback, secondary} => NativeFooter::stack(
                native, label.as_deref(), callback.into_pelican(theme), secondary.map(action)),
            Footer::End {steps} => NativeFooter::stack_end(native, steps),
            Footer::Home {first, second} => NativeFooter::home(native, first.map(action), second.map(action)),
            Footer::Alert {notice} => NativeFooter::alert(native, &notice),
            Footer::Input {placeholder, mut on_send, valid} => NativeFooter::input(native, &placeholder,
                move |ctx: &mut Context, value: &mut String| on_send(ctx, value), NativeValidity(valid)),
        }
    }
}

impl crate::ListItem {
    pub fn to_pelican(&self, theme: &Theme) -> NativeListItem {
        let native = theme.native_pelican();
        NativeListItem::new(native,
            self.avatar.clone().map(|content| Avatar::new(native, content, None, AvatarSize::Md, None)),
            ListItemInfoLeft::new(&self.title, Some(&self.subtitle), None, self.flair).with_badge(self.badge),
            self.secondary.as_ref().map(|value| TitleSubtitle::new(value, Some("Details"))),
            None, self.action.is_some().then_some(Icons::Forward),
            self.action.as_ref().unwrap_or(&crate::Action::None).get().into_pelican(theme))
    }
}

impl dyn PlaybackCallback {
    pub fn into_pelican(mut self: Box<Self>) -> Box<dyn native::PlaybackCallback> {
        Box::new(move |ctx: &mut Context, action: native::PlaybackAction| self(ctx, match action {
            native::PlaybackAction::Toggle => PlaybackAction::Toggle,
            native::PlaybackAction::Scrub(value, phase) => PlaybackAction::Scrub(value, match phase {
                native::ScrubPhase::Start => ScrubPhase::Start, native::ScrubPhase::Move => ScrubPhase::Move,
                native::ScrubPhase::End => ScrubPhase::End,
            }),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct PelicanUi {
    theme: NativeTheme,
}

impl PelicanUi {
    pub fn native_theme(&self) -> &NativeTheme {&self.theme}
}

impl DesignSystem for PelicanUi {
    fn new(config: crate::ChkTheme, assets: Assets) -> Self {
        Self {theme: match config {
            crate::ChkTheme::Dark(color) => NativeTheme::dark(&assets.inner, color),
            crate::ChkTheme::Light(color) => NativeTheme::light(&assets.inner, color),
            crate::ChkTheme::Auto(color) => NativeTheme::from(&assets.inner, color),
        }}
    }
    fn colors(&self) -> &ptsd::ColorResources {self.theme.colors()}
    fn image(&self, name: &str) -> Option<prism::hardware::Image> {self.theme.brand().images.get(name).cloned()}
    fn with_image(&self, name: &str, image: prism::hardware::Image) -> Box<dyn DesignSystem> {
        let mut theme = self.theme.clone();
        theme.brand_mut().images.insert(name.into(), image);
        Box::new(Self {theme})
    }
    fn fonts(&self) -> &ptsd::FontResources {self.theme.fonts()}
    fn configure_gifs(&self, key: &str) {self.theme.gifs().set_api_key(key);}
    fn gif_bytes(&self, gif: &Gif) -> GifBytes {
        let (gifs, gif) = (self.theme.gifs().clone(), gif.clone());
        Box::pin(async move {gifs.bytes(&gif).await})
    }
    fn interface(&self, _: &Theme, ctx: &mut Context, roots: Vec<crate::RootInfo>, keyboard: Option<Box<dyn Drawable>>) -> Box<dyn Drawable> {
        let roots = roots.into_iter().map(|root| match root.icon {
            crate::RootIcon::Icon(icon) => NativeRoot::icon(icon, &root.label, root.page),
            crate::RootIcon::Avatar(avatar) => NativeRoot::avatar(avatar, &root.label, root.page),
        }).collect();
        Box::new(Interface::new(ctx, &self.theme, roots, keyboard))
    }
    fn page(&self, theme: &Theme, header: Header, content: Content, footer: Option<Footer>) -> PageView {
        PageView::new(NativePage::new(header.into_pelican(theme), content, footer.map(|footer| footer.into_pelican(theme))))
    }
    fn slider_input(&self, _: &Theme, start: f32, label: Option<&str>, description: Option<&str>) -> Box<dyn Drawable> {
        Control::boxed(native::Slider::new(&self.theme, start, label, description, |_: &mut Context, _: &NativeTheme, _: f32| {}))
    }
    fn text_input(&self, _: &Theme, id: Uuid, preset: Option<&str>, label: Option<&str>, placeholder: &str) -> Box<dyn Drawable> {
        Control::boxed(TextInput::new(&self.theme, id, preset, label, Some(placeholder), None, None))
    }
    fn choice_input(&self, theme: &Theme, index: usize, items: Vec<(&str, &str, Box<dyn Callback>)>) -> Box<dyn Drawable> {
        Control::boxed(RadioSelector::new(&self.theme, index, items.into_iter().map(|(a,b,action)| (a,b,action.into_pelican(theme))).collect()))
    }
    fn currency_input(&self, _: &Theme, instructions: &str) -> Box<dyn Drawable> {Control::boxed(NumericalInput::numerical(&self.theme, instructions))}
    fn number_input(&self, _: &Theme, instructions: &str) -> Box<dyn Drawable> {Control::boxed(NumericalInput::number(&self.theme, instructions))}
    fn date_input(&self, _: &Theme, instructions: &str) -> Box<dyn Drawable> {Control::boxed(NumericalInput::date(&self.theme, instructions))}
    fn time_input(&self, _: &Theme, instructions: &str) -> Box<dyn Drawable> {Control::boxed(NumericalInput::time(&self.theme, instructions))}
    fn keypad(&self, _: &Theme) -> Box<dyn Drawable> {Box::new(Keypad::new(&self.theme))}
    fn selection_input(&self, theme: &Theme, selector: Selector) -> Box<dyn Drawable> {
        let actions = selector.actions.iter().map(|action| action.get(selector.id).presentation().into_pelican(theme)).collect();
        let selector = native::Selector::from_parts(selector.id, selector.label, selector.items, selector.limit,
            selector.chip_subtitles, selector.sort_by_title);
        Control::boxed(SelectionInput::with_actions(&self.theme, selector, actions, false))
    }
    fn search_input(&self, theme: &Theme, id: Uuid, label: &str, items: Vec<(crate::ListItem, prism::air::Name)>, actions: Vec<crate::ActionPresentation>) -> Box<dyn Drawable> {
        Control::boxed(SelectionInput::legacy(&self.theme, id, label, items, actions.into_iter().map(|action| action.into_pelican(theme)).collect()))
    }
    fn scanner_input(&self, _: &Theme, mut on_find: Box<dyn QrCodeFound>) -> Box<dyn Drawable> {
        Control::boxed(QRCodeScanner::new(&self.theme, Box::new(move |ctx: &mut Context, value: String| on_find(ctx, value))))
    }
    fn text_view(&self, _: &Theme, text: &str, size: TextSize, style: TextStyle, align: Align) -> Box<dyn Drawable> {
        let style = match style {
            TextStyle::Heading => NativeTextStyle::Heading, TextStyle::Primary => NativeTextStyle::Primary,
            TextStyle::Secondary => NativeTextStyle::Secondary, TextStyle::Error => NativeTextStyle::Error,
            TextStyle::Keyboard => NativeTextStyle::Keyboard, TextStyle::Label(color) => NativeTextStyle::Label(color),
        };
        Box::new(Text::new(&self.theme, text, size, style, align, None))
    }
    fn alert_view(&self, _: &Theme, text: &str) -> Box<dyn Drawable> {Box::new(Alert::danger(&self.theme, text))}
    fn icon_view(&self, _: &Theme, icon: Icons, size: f32) -> Box<dyn Drawable> {
        Box::new(Icon::new(&self.theme, icon, Some(self.theme.colors().get(colors::Text::Heading)), size))
    }
    fn image_view(&self, _: &Theme, name: &str, width: f32) -> Box<dyn Drawable> {
        match self.theme.brand().images.get(name) {
            Some(image) => Box::new(AspectRatioImage::new(image.clone(), width)),
            None => Box::new(AspectRatioImage::svg(self.theme.brand().svgs.get(name).unwrap().clone(), width)),
        }
    }
    fn data_text(&self, theme: &Theme, label: &str, data: Option<&str>, instructions: &str, actions: Option<Vec<crate::ActionPresentation>>) -> Box<dyn Drawable> {
        Box::new(DataItem::text(&self.theme, label, data, instructions, actions.map(|items| items.into_iter().map(|action| action.into_pelican(theme)).collect())))
    }
    fn data_table(&self, theme: &Theme, label: &str, items: Vec<(String, String)>, actions: Option<Vec<crate::ActionPresentation>>) -> Box<dyn Drawable> {
        Box::new(DataItem::table(&self.theme, label, items, actions.map(|items| items.into_iter().map(|action| action.into_pelican(theme)).collect())))
    }
    fn data_list(&self, theme: &Theme, label: &str, instructions: Option<&str>, items: Vec<crate::ListItem>, actions: Option<Vec<crate::ActionPresentation>>) -> Box<dyn Drawable> {
        Box::new(DataItem::list(&self.theme, label, instructions, items.iter().map(|item| item.to_pelican(theme)).collect(),
            actions.map(|items| items.into_iter().map(|action| action.into_pelican(theme)).collect())))
    }
    fn currency_view(&self, _: &Theme, amount: Number, instructions: &str) -> Box<dyn Drawable> {Box::new(NumericalInput::display_currency(&self.theme, amount, instructions))}
    fn number_view(&self, _: &Theme, number: Number, instructions: &str) -> Box<dyn Drawable> {Box::new(NumericalInput::display_number(&self.theme, number, instructions))}
    fn list_section(&self, theme: &Theme, label: Option<String>, instructions: Option<String>, items: Vec<crate::ListItem>) -> Box<dyn Drawable> {
        Box::new(ListItemSection::new(&self.theme, label, instructions, items.iter().map(|item| item.to_pelican(theme)).collect()))
    }
    fn qr_code(&self, _: &Theme, data: &str) -> Box<dyn Drawable> {Box::new(QRCode::new(&self.theme, data))}
    fn avatar_display(&self, _: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>) -> Box<dyn Drawable> {
        Box::new(Avatar::new(&self.theme, content, flair, AvatarSize::Xxl, None))
    }
    fn quick_actions(&self, theme: &Theme, offset: Offset, actions: Vec<crate::ActionPresentation>) -> Box<dyn Drawable> {
        Box::new(QuickActions::new(&self.theme, offset, actions.into_iter().map(|action| action.into_pelican(theme)).collect()))
    }
    fn icon_actions(&self, theme: &Theme, actions: Vec<(Icons, Box<dyn Callback>)>) -> Box<dyn Drawable> {
        Box::new(IconButtonGroup::new(&self.theme, actions.into_iter().map(|(icon,action)| (icon,action.into_pelican(theme))).collect()))
    }
    fn list_item(&self, theme: &Theme, item: &crate::ListItem) -> ListItemView {Box::new(item.to_pelican(theme))}
    fn avatar_input(&self, theme: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Box<dyn Callback>>) -> AvatarView {
        AvatarView::new(Avatar::new(&self.theme, content, flair, AvatarSize::Xxl, action.map(|action| action.into_pelican(theme))))
    }
    fn photo_page(&self, theme: &Theme, photo: Photo, header: Option<(Icons, Box<dyn Callback>)>) -> PhotoPageView {
        Box::new(native::PhotoPageView::new(&self.theme, photo, header.map(|(icon,action)| (icon,action.into_pelican(theme)))))
    }
    fn video_poster(&self, theme: &Theme, photo: &Photo, header: Option<(Icons, Box<dyn Callback>)>, action: Box<dyn PlaybackCallback>) -> VideoView {
        VideoView::new(native::PlaybackView::poster(&self.theme, photo,
            header.map(|(icon,action)| (icon,action.into_pelican(theme))), action.into_pelican()))
    }
    fn video_player(&self, _: &Theme, title: &str, player: prism::hardware::Player, size: (u32,u32), action: Box<dyn PlaybackCallback>) -> VideoView {
        VideoView::new(native::PlaybackView::player(&self.theme, title, player, size, action.into_pelican()))
    }
    fn camera_view(&self, theme: &Theme, capture: Box<dyn Callback>) -> CameraView {
        CameraView::new(native::CameraView::new(&self.theme,capture.into_pelican(theme)))
    }
    fn message_group(&self, theme: &Theme, ctx: &mut Context, messages: Vec<MessageView>, group: bool) -> MessageGroups {
        let messages = messages.into_iter().map(|message| native::Message {body:message.body,timestamp:message.timestamp,
            author:message.author.into_pelican(),on_click:message.on_click.into_pelican(theme)}).collect();
        Box::new(native::MessageGroups::new(ctx,&self.theme,messages,group,false))
    }
    fn typing_indicator(&self, _: &Theme, name: Option<&str>) -> TypingIndicator {Box::new(native::TypingIndicator::new(&self.theme,name))}
}

impl crate::PageSurface for NativePage {
    fn content(&self) -> &Content {&self.content}
    fn content_mut(&mut self) -> &mut Content {&mut self.content}
    fn replace_content(&mut self, content: Content) {self.content = content;}
    fn disable_actions(&mut self, disabled: bool) {if let Some(footer) = &mut self.bumper {footer.disable(disabled);}}
    fn on_primary(&mut self, action: Box<dyn ptsd::utils::Callback>) {if let Some(footer) = &mut self.bumper {*footer.on_click()[0] = action;}}
    fn composer_state(&mut self) -> Option<(Uuid, bool)> {
        let field = self.bumper.as_mut()?.message_bar()?.field(); Some((field.id(), field.has_focus()))
    }
    fn take_footer_from(&mut self, old: &mut dyn crate::PageSurface) {
        if let Some(old) = old.as_any_mut().downcast_mut::<Self>() && let Some(footer) = old.bumper.take() {self.bumper=Some(footer);}
    }
}
impl crate::AvatarSurface for Avatar {
    fn content(&self) -> &AvatarContent {Avatar::content(self)}
    fn set_content(&mut self, content: AvatarContent) {Avatar::set_content(self,content);}
}
impl crate::CameraSurface for native::CameraView {
    fn reset(&mut self, theme: &Theme) {native::CameraView::reset(self,theme.native_pelican());}
    fn error(&mut self, theme: &Theme, message: &str, disabled: bool) {native::CameraView::error(self,theme.native_pelican(),message,disabled);}
    fn disable(&mut self, disabled: bool) {native::CameraView::disable(self,disabled);}
    fn preview(&mut self, image: prism::hardware::Image) {native::CameraView::preview(self,image);}
}
impl crate::PlaybackSurface for native::PlaybackView {
    fn set_player(&mut self, player:prism::hardware::Player,size:(u32,u32)) {native::PlaybackView::set_player(self,player,size);}
    fn progress(&mut self, value:f32) {native::PlaybackView::progress(self,value);}
    fn badge(&mut self, theme:&Theme,badge:PlaybackBadge) {
        native::PlaybackView::badge(self,theme.native_pelican(),match badge {
            PlaybackBadge::Loading=>native::PlaybackBadge::Loading, PlaybackBadge::Unplayable=>native::PlaybackBadge::Unplayable,
            PlaybackBadge::Play=>native::PlaybackBadge::Play, PlaybackBadge::None=>native::PlaybackBadge::None,
        });
    }
    fn playing(&mut self,theme:&Theme,playing:bool) {native::PlaybackView::playing(self,theme.native_pelican(),playing);}
    fn position(&mut self,theme:&Theme,seconds:u64) {native::PlaybackView::position(self,theme.native_pelican(),seconds);}
    fn duration(&mut self,theme:&Theme,seconds:u64) {native::PlaybackView::duration(self,theme.native_pelican(),seconds);}
}
impl crate::Slider for native::Slider {fn value(&self)->f32 {self.current_value()}}
impl crate::InputControl for native::Slider {fn state(&self)->State {State::Slider(crate::Slider::value(self))}}
impl crate::TextField for TextInput {
    fn value(&self)->String {TextInput::value(self)}
    fn id(&self)->Uuid {TextInput::id(self)}
    fn focused(&self)->bool {self.has_focus()}
}
impl crate::InputControl for TextInput {
    fn state(&self)->State {State::Text(crate::TextField::value(self))}
    fn validation_value(&self)->Option<String> {Some(crate::TextField::value(self))}
    fn feedback(&mut self,result:&crate::FormValidState) {self.error(result.eval().map(|_|()))}
}
impl crate::NumberField for NumericalInput {fn value(&self)->String {NumericalInput::value(self)}}
impl crate::InputControl for NumericalInput {
    fn state(&self)->State {State::Number(crate::NumberField::value(self))}
    fn validation_value(&self)->Option<String> {Some(crate::NumberField::value(self))}
    fn feedback(&mut self,result:&crate::FormValidState) {self.error(result.eval())}
}
impl crate::ChoiceField for RadioSelector {fn value(&self)->usize {RadioSelector::value(self)}}
impl crate::InputControl for RadioSelector {fn state(&self)->State {State::Enumerator(crate::ChoiceField::value(self))}}
impl crate::SelectionField for SelectionInput {fn value(&self)->Vec<String> {self.results()}}
impl crate::InputControl for SelectionInput {
    fn state(&self)->State {
        let keys=crate::SelectionField::value(self);
        if self.uses_names() {State::Search(keys.iter().filter_map(|key|key.parse().ok()).collect())} else {State::Selection(keys)}
    }
}
impl crate::Scanner for QRCodeScanner {fn value(&self)->Option<String> {self.found()}}
impl crate::InputControl for QRCodeScanner {fn state(&self)->State {State::ScanCode(crate::Scanner::value(self))}}

#[derive(Clone)]
struct NativeValidity(Box<dyn ValidFn>);
impl ptsd::Validate<&mut String, bool> for NativeValidity {
    fn validate(&mut self, ctx: &mut Context, value: &mut String) -> bool {self.0.validate(ctx,value)}
    fn change(&mut self) -> Pin<Box<dyn Future<Output=Option<()>>+'_>> {self.0.change()}
}
