//! Pelican's CHK adapter, using only the public APIs of both crates.
//! This file can also live in an app or adapter crate: select its `PelicanUi` as
//! `App::DesignSystem` and depend on CHK without its built-in `pelican_ui` feature.

use chk::design_system::*;
use chk::{DesignSystem, Control, State};
use prism::drawable::{Drawable, LayoutTree, Instruction, Rect, text::Align};
use prism::events::{DynEvent, ListenFuture, ListenCallback};
use ptsd::layout::Offset;
use ptsd::utils::TitleSubtitle;
use uuid::Uuid;
use ::pelican_ui::{Callback as NativeCallback, theme::Theme as NativeTheme};
use ::pelican_ui::components::{
    self as native, TextInput, RadioSelector, NumericalInput, SelectionInput,
    QRCodeScanner, Keypad, Icon, Alert, AspectRatioImage, DataItem, QRCode,
};
use ::pelican_ui::components::avatar::{Avatar, AvatarSize};
use ::pelican_ui::components::text::{Text, TextStyle as NativeTextStyle};
use ::pelican_ui::components::button::{ActionData, QuickActions, IconButtonGroup};
use ::pelican_ui::components::list_item::{ListItem as NativeListItem, ListItemInfoLeft, ListItemSection};
use ::pelican_ui::interface::general::{
    Page as NativePage, Header as NativeHeader, Bumper as NativeFooter, FullScreen, Interface,
};
use ::pelican_ui::interface::navigation::RootInfo as NativeRoot;

use std::{any::Any, future::Future, pin::Pin};

/// Owns the native theme or widget so its CHK trait implementations also work in external crates.
#[derive(Debug, Clone)]
pub struct PelicanUi<T = NativeTheme> {
    inner: T,
}

impl PelicanUi {
    pub fn native_theme(&self) -> &NativeTheme {&self.inner}

    pub fn callback(mut action: Box<dyn Callback>, theme: &Theme) -> Box<dyn NativeCallback> {
        let theme = theme.clone();
        Box::new(move |ctx: &mut Context, _: &NativeTheme| action(ctx, &theme))
    }

    pub fn action(action: chk::ActionPresentation, theme: &Theme) -> ActionData {
        ActionData {
            label: action.label, active: action.active, icon: action.icon,
            on_click: PelicanUi::callback(action.on_click, theme),
        }
    }

    pub fn profile(profile: ProfileView) -> native::Profile {
        native::Profile {name: profile.name, username: profile.username, pfp: profile.pfp}
    }

    pub fn header(&self, header: Header, theme: &Theme) -> NativeHeader {
        let native = &self.inner;
        let action = |(icon, action): (_, Box<dyn Callback>)| (icon, PelicanUi::callback(action, theme));
        match header {
            Header::Stack {title, action: callback} => {
                NativeHeader::stack(native, &title, callback.map(action))
            }
            Header::End {title} => NativeHeader::stack_end(native, &title),
            Header::Home {title, action: callback} => {
                NativeHeader::home(native, &title, callback.map(action))
            }
            Header::Leading {title, action: callback} => {
                NativeHeader::home_leading(native, &title, callback.map(action))
            }
            Header::Messaging {title, profiles, avatar, steps, info} => {
                NativeHeader::messaging_with_avatar(
                    native, title, profiles.into_iter().map(PelicanUi::profile).collect(), avatar, steps,
                    info.map(|action| PelicanUi::callback(action, theme)),
                )
            }
        }
    }

    pub fn footer(&self, footer: Footer, theme: &Theme) -> NativeFooter {
        let native = &self.inner;
        let action = |(label, action): (_, Box<dyn Callback>)| {
            (label, PelicanUi::callback(action, theme))
        };
        match footer {
            Footer::Stack {label, action: callback, secondary} => {
                NativeFooter::stack(
                    native, label.as_deref(), PelicanUi::callback(callback, theme),
                    secondary.map(action),
                )
            }
            Footer::End {steps} => NativeFooter::stack_end(native, steps),
            Footer::Home {first, second} => {
                NativeFooter::home(native, first.map(action), second.map(action))
            }
            Footer::Alert {notice} => NativeFooter::alert(native, &notice),
            Footer::Input {placeholder, mut on_send, valid} => {
                NativeFooter::input(
                    native, &placeholder,
                    move |ctx: &mut Context, value: &mut String| on_send(ctx, value),
                    PelicanUi {inner: valid},
                )
            }
        }
    }

    pub fn native_list_item(&self, item: &chk::ListItem, theme: &Theme) -> NativeListItem {
        let native = &self.inner;
        NativeListItem::new(
            native,
            item.avatar.clone().map(|content| Avatar::new(native, content, None, AvatarSize::Md, None)),
            ListItemInfoLeft {
                title: TitleSubtitle::new(&item.title, Some(&item.subtitle)),
                description: None, flair: item.flair, badge: item.badge,
            },
            item.secondary.as_ref().map(|value| TitleSubtitle::new(value, Some("Details"))),
            None, item.action.is_some().then_some(Icons::Forward),
            PelicanUi::callback(item.action.as_ref().unwrap_or(&chk::Action::None).get(), theme),
        )
    }

    pub fn playback_callback(mut callback: Box<dyn PlaybackCallback>) -> Box<dyn native::PlaybackCallback> {
        Box::new(move |ctx: &mut Context, action: native::PlaybackAction| callback(ctx, match action {
            native::PlaybackAction::Toggle => PlaybackAction::Toggle,
            native::PlaybackAction::Scrub(value, phase) => PlaybackAction::Scrub(value, match phase {
                native::ScrubPhase::Start => ScrubPhase::Start,
                native::ScrubPhase::Move => ScrubPhase::Move,
                native::ScrubPhase::End => ScrubPhase::End,
            }),
        }))
    }
}

impl<T: Drawable + Clone> Drawable for PelicanUi<T> {
    fn request_width(&mut self) -> (f32, f32) {self.inner.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.inner.request_height(width)}
    fn layout(&mut self, size: (f32, f32)) -> LayoutTree {self.inner.layout(size)}
    fn draw(&self, layout: &LayoutTree, offset: (f32, f32), bound: Rect) -> Vec<Instruction> {
        self.inner.draw(layout, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, layout: &LayoutTree, event: Box<dyn DynEvent>) {
        self.inner.event(ctx, layout, event);
    }
    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let callback = self.inner.changed().await;
            Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                let target = target.downcast_mut::<PelicanUi<T>>().unwrap();
                callback(&mut target.inner, ctx);
            }) as ListenCallback
        })
    }
}

impl DesignSystem for PelicanUi {
    fn new(config: chk::ChkTheme, assets: Assets) -> Self {
        Self {
            inner: match config {
                chk::ChkTheme::Dark(color) => NativeTheme::dark(&assets.inner, color),
                chk::ChkTheme::Light(color) => NativeTheme::light(&assets.inner, color),
                chk::ChkTheme::Auto(color) => NativeTheme::from(&assets.inner, color),
            },
        }
    }
    fn colors(&self) -> &ptsd::ColorResources {self.inner.colors()}
    fn image(&self, name: &str) -> Option<prism::hardware::Image> {
        self.inner.brand().images.get(name).cloned()
    }
    fn with_image(&self, name: &str, image: prism::hardware::Image) -> Box<dyn DesignSystem> {
        let mut theme = self.inner.clone();
        theme.brand_mut().images.insert(name.into(), image);
        Box::new(Self {inner: theme})
    }
    fn fonts(&self) -> &ptsd::FontResources {self.inner.fonts()}
    fn configure_gifs(&self, key: &str) {self.inner.gifs().set_api_key(key);}
    fn gif_bytes(&self, gif: &Gif) -> GifBytes {
        let (gifs, gif) = (self.inner.gifs().clone(), gif.clone());
        Box::pin(async move {gifs.bytes(&gif).await})
    }
    fn page(&self, theme: &Theme, header: Header, content: Content, footer: Option<Footer>) -> Box<dyn chk::PageSurface> {
        Box::new(PelicanUi {
            inner: NativePage::new(
                self.header(header, theme), content,
                footer.map(|footer| self.footer(footer, theme)),
            ),
        })
    }
    fn interface(&self, _: &Theme, ctx: &mut Context, roots: Vec<chk::RootInfo>, keyboard: Option<Box<dyn Drawable>>) -> Box<dyn Drawable> {
        let roots = roots.into_iter().map(|root| match root.icon {
            chk::RootIcon::Icon(icon) => NativeRoot::icon(icon, &root.label, root.page),
            chk::RootIcon::Avatar(avatar) => NativeRoot::avatar(avatar, &root.label, root.page),
        }).collect();
        Box::new(Interface::new(ctx, &self.inner, roots, keyboard))
    }
    fn edge_to_edge(&self, theme: &Theme, header: Header, content: Box<dyn Drawable>) -> Box<dyn Drawable> {
        Box::new(FullScreen::without_footer(&self.inner, self.header(header, theme), content))
    }
    fn slider_input(&self, _: &Theme, start: f32, label: Option<&str>, description: Option<&str>) -> Control {
        Control::new(PelicanUi {
            inner: native::Slider::new(
                &self.inner, start, label, description, |_: &mut Context, _: &NativeTheme, _: f32| {},
            ),
        })
    }
    fn text_input(&self, _: &Theme, id: Uuid, preset: Option<&str>, label: Option<&str>, placeholder: &str) -> Control {
        Control::new(PelicanUi {
            inner: TextInput::new(&self.inner, id, preset, label, Some(placeholder), None, None),
        })
    }
    fn choice_input(&self, theme: &Theme, index: usize, items: Vec<(&str, &str, Box<dyn Callback>)>) -> Control {
        let items = items.into_iter()
            .map(|(label, description, action)| (label, description, PelicanUi::callback(action, theme)))
            .collect();
        Control::new(PelicanUi {inner: RadioSelector::new(&self.inner, index, items)})
    }
    fn currency_input(&self, _: &Theme, instructions: &str) -> Control {
        Control::new(PelicanUi {inner: NumericalInput::numerical(&self.inner, instructions)})
    }
    fn number_input(&self, _: &Theme, instructions: &str) -> Control {
        Control::new(PelicanUi {inner: NumericalInput::number(&self.inner, instructions)})
    }
    fn date_input(&self, _: &Theme, instructions: &str) -> Control {
        Control::new(PelicanUi {inner: NumericalInput::date(&self.inner, instructions)})
    }
    fn time_input(&self, _: &Theme, instructions: &str) -> Control {
        Control::new(PelicanUi {inner: NumericalInput::time(&self.inner, instructions)})
    }
    fn keypad(&self, _: &Theme) -> Box<dyn Drawable> {Box::new(Keypad::new(&self.inner))}
    fn selection_input(&self, theme: &Theme, selector: Selector) -> Control {
        let actions = selector.actions.iter()
            .map(|action| PelicanUi::action(action.get(selector.id).presentation(), theme)).collect();
        let selector = native::Selector::from_parts(
            selector.id, selector.label, selector.items, selector.limit,
            selector.chip_subtitles, selector.sort_by_title,
        );
        Control::new(PelicanUi {inner: SelectionInput::with_actions(&self.inner, selector, actions)})
    }
    fn scanner_input(&self, _: &Theme, mut on_find: Box<dyn QrCodeFound>) -> Control {
        Control::new(PelicanUi {
            inner: QRCodeScanner::new(
                &self.inner, Box::new(move |ctx: &mut Context, value: String| on_find(ctx, value)),
            ),
        })
    }
    fn text_view(&self, _: &Theme, text: &str, size: TextSize, style: TextStyle, align: Align) -> Box<dyn Drawable> {
        let style = match style {
            TextStyle::Heading => NativeTextStyle::Heading,
            TextStyle::Primary => NativeTextStyle::Primary,
            TextStyle::Secondary => NativeTextStyle::Secondary,
            TextStyle::Error => NativeTextStyle::Error,
            TextStyle::Keyboard => NativeTextStyle::Keyboard,
            TextStyle::Label(color) => NativeTextStyle::Label(color),
        };
        Box::new(Text::new(&self.inner, text, size, style, align, None))
    }
    fn alert_view(&self, _: &Theme, text: &str) -> Box<dyn Drawable> {
        Box::new(Alert::danger(&self.inner, text))
    }
    fn icon_view(&self, _: &Theme, icon: Icons, size: f32) -> Box<dyn Drawable> {
        Box::new(Icon::new(
            &self.inner, icon, Some(self.inner.colors().get(colors::Text::Heading)), size,
        ))
    }
    fn image_view(&self, _: &Theme, name: &str, width: f32) -> Box<dyn Drawable> {
        match self.inner.brand().images.get(name) {
            Some(image) => Box::new(AspectRatioImage::new(image.clone(), width)),
            None => Box::new(AspectRatioImage::svg(
                self.inner.brand().svgs.get(name).unwrap().clone(), width,
            )),
        }
    }
    fn data_text(
        &self, theme: &Theme, label: &str, data: Option<&str>, instructions: &str,
        actions: Option<Vec<chk::ActionPresentation>>,
    ) -> Box<dyn Drawable> {
        let actions = actions.map(|items| items.into_iter()
            .map(|action| PelicanUi::action(action, theme)).collect());
        Box::new(DataItem::text(&self.inner, label, data, instructions, actions))
    }
    fn data_table(&self, theme: &Theme, label: &str, items: Vec<(String, String)>, actions: Option<Vec<chk::ActionPresentation>>) -> Box<dyn Drawable> {
        let actions = actions.map(|items| items.into_iter()
            .map(|action| PelicanUi::action(action, theme)).collect());
        Box::new(DataItem::table(&self.inner, label, items, actions))
    }
    fn data_list(
        &self, theme: &Theme, label: &str, instructions: Option<&str>,
        items: Vec<chk::ListItem>, actions: Option<Vec<chk::ActionPresentation>>,
    ) -> Box<dyn Drawable> {
        let items = items.iter().map(|item| self.native_list_item(item, theme)).collect();
        let actions = actions.map(|items| items.into_iter()
            .map(|action| PelicanUi::action(action, theme)).collect());
        Box::new(DataItem::list(&self.inner, label, instructions, items, actions))
    }
    fn currency_view(&self, _: &Theme, amount: Number, instructions: &str) -> Box<dyn Drawable> {
        Box::new(NumericalInput::display_currency(&self.inner, amount, instructions))
    }
    fn number_view(&self, _: &Theme, number: Number, instructions: &str) -> Box<dyn Drawable> {
        Box::new(NumericalInput::display_number(&self.inner, number, instructions))
    }
    fn list_section(&self, theme: &Theme, label: Option<String>, instructions: Option<String>, items: Vec<chk::ListItem>) -> Box<dyn Drawable> {
        let items = items.iter().map(|item| self.native_list_item(item, theme)).collect();
        Box::new(ListItemSection::new(&self.inner, label, instructions, items))
    }
    fn qr_code(&self, _: &Theme, data: &str) -> Box<dyn Drawable> {
        Box::new(QRCode::new(&self.inner, data))
    }
    fn avatar_display(&self, _: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>) -> Box<dyn Drawable> {
        Box::new(Avatar::new(&self.inner, content, flair, AvatarSize::Xxl, None))
    }
    fn quick_actions(&self, theme: &Theme, offset: Offset, actions: Vec<chk::ActionPresentation>) -> Box<dyn Drawable> {
        let actions = actions.into_iter().map(|action| PelicanUi::action(action, theme)).collect();
        Box::new(QuickActions::new(&self.inner, offset, actions))
    }
    fn icon_actions(&self, theme: &Theme, actions: Vec<(Icons, Box<dyn Callback>)>) -> Box<dyn Drawable> {
        let actions = actions.into_iter()
            .map(|(icon, action)| (icon, PelicanUi::callback(action, theme))).collect();
        Box::new(IconButtonGroup::new(&self.inner, actions))
    }
    fn list_item(&self, theme: &Theme, item: &chk::ListItem) -> ListItemView {
        Box::new(self.native_list_item(item, theme))
    }
    fn avatar_input(
        &self, theme: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>,
        action: Option<Box<dyn Callback>>,
    ) -> AvatarView {
        AvatarView::new(PelicanUi {
            inner: Avatar::new(
                &self.inner, content, flair, AvatarSize::Xxl,
                action.map(|action| PelicanUi::callback(action, theme)),
            ),
        })
    }
    fn photo_view(&self, _: &Theme, photo: &Photo) -> Box<dyn Drawable> {
        Box::new(native::FittedPhoto::new(photo))
    }
    fn video_poster(&self, _: &Theme, photo: &Photo, action: Box<dyn PlaybackCallback>) -> VideoView {
        VideoView::new(PelicanUi {
            inner: native::PlaybackView::poster(
                &self.inner, photo, PelicanUi::playback_callback(action),
            ),
        })
    }
    fn video_player(&self, _: &Theme, player: prism::hardware::Player, size: (u32, u32), action: Box<dyn PlaybackCallback>) -> VideoView {
        VideoView::new(PelicanUi {
            inner: native::PlaybackView::player(
                &self.inner, player, size, PelicanUi::playback_callback(action),
            ),
        })
    }
    fn camera_view(&self, theme: &Theme, disabled: bool, capture: Box<dyn Callback>) -> CameraView {
        CameraView::new(PelicanUi {
            inner: native::CameraView::new(&self.inner, disabled, PelicanUi::callback(capture, theme)),
        })
    }
    fn message_group(&self, theme: &Theme, ctx: &mut Context, messages: Vec<MessageView>, group: bool) -> MessageGroups {
        let messages = messages.into_iter().map(|message| native::Message {
            body: message.body, timestamp: message.timestamp, author: PelicanUi::profile(message.author),
            on_click: PelicanUi::callback(message.on_click, theme),
        }).collect();
        Box::new(native::MessageGroups::new(ctx, &self.inner, messages, group, false))
    }
    fn typing_indicator(&self, _: &Theme, name: Option<&str>) -> TypingIndicator {
        Box::new(native::TypingIndicator::new(&self.inner, name))
    }
}

impl chk::PageSurface for PelicanUi<NativePage> {
    fn content(&self) -> &Content {&self.inner.content}
    fn content_mut(&mut self) -> &mut Content {&mut self.inner.content}
    fn disable_actions(&mut self, disabled: bool) {
        if let Some(footer) = &mut self.inner.bumper {footer.disable(disabled);}
    }
    fn composer_state(&mut self) -> Option<(Uuid, bool)> {
        let field = self.inner.bumper.as_mut()?.message_bar()?.field();
        Some((field.id(), field.has_focus()))
    }
    fn take_footer_from(&mut self, old: &mut dyn chk::PageSurface) {
        if let Some(old) = old.as_any_mut().downcast_mut::<PelicanUi<NativePage>>()
            && let Some(footer) = old.inner.bumper.take() {
            self.inner.bumper = Some(footer);
        }
    }
}
impl chk::AvatarSurface for PelicanUi<Avatar> {
    fn content(&self) -> &AvatarContent {self.inner.content()}
    fn set_content(&mut self, content: AvatarContent) {self.inner.set_content(content);}
}
impl chk::CameraSurface for PelicanUi<native::CameraView> {
    fn reset(&mut self, theme: &Theme) {
        self.inner.reset(theme.downcast_ref::<PelicanUi>().unwrap().native_theme());
    }
    fn error(&mut self, theme: &Theme, message: &str, disabled: bool) {
        let native = theme.downcast_ref::<PelicanUi>().unwrap().native_theme();
        self.inner.error(native, message, disabled);
    }
    fn disable(&mut self, disabled: bool) {self.inner.disable(disabled);}
    fn preview(&mut self, image: prism::hardware::Image) {self.inner.preview(image);}
}
impl chk::PlaybackSurface for PelicanUi<native::PlaybackView> {
    fn set_player(&mut self, player: prism::hardware::Player, size: (u32, u32)) {
        self.inner.set_player(player, size);
    }
    fn progress(&mut self, value: f32) {self.inner.progress(value);}
    fn badge(&mut self, theme: &Theme, badge: PlaybackBadge) {
        let native = theme.downcast_ref::<PelicanUi>().unwrap().native_theme();
        self.inner.badge(native, match badge {
            PlaybackBadge::Loading => native::PlaybackBadge::Loading,
            PlaybackBadge::Unplayable => native::PlaybackBadge::Unplayable,
            PlaybackBadge::Play => native::PlaybackBadge::Play,
            PlaybackBadge::None => native::PlaybackBadge::None,
        });
    }
    fn playing(&mut self, theme: &Theme, playing: bool) {
        self.inner.playing(theme.downcast_ref::<PelicanUi>().unwrap().native_theme(), playing);
    }
    fn position(&mut self, theme: &Theme, seconds: u64) {
        self.inner.position(theme.downcast_ref::<PelicanUi>().unwrap().native_theme(), seconds);
    }
    fn duration(&mut self, theme: &Theme, seconds: u64) {
        self.inner.duration(theme.downcast_ref::<PelicanUi>().unwrap().native_theme(), seconds);
    }
}
impl chk::InputControl for PelicanUi<native::Slider> {
    fn state(&self) -> State {State::Slider(self.inner.current_value())}
}
impl chk::InputControl for PelicanUi<TextInput> {
    fn state(&self) -> State {State::Text(self.inner.value())}
    fn feedback(&mut self, result: &chk::FormValidState) {self.inner.error(result.eval().map(|_|()))}
}
impl chk::InputControl for PelicanUi<NumericalInput> {
    fn state(&self) -> State {State::Number(self.inner.value())}
    fn feedback(&mut self, result: &chk::FormValidState) {self.inner.error(result.eval())}
}
impl chk::InputControl for PelicanUi<RadioSelector> {
    fn state(&self) -> State {State::Enumerator(self.inner.value())}
}
impl chk::InputControl for PelicanUi<SelectionInput> {
    fn state(&self) -> State {State::Selection(self.inner.results())}
}
impl chk::InputControl for PelicanUi<QRCodeScanner> {
    fn state(&self) -> State {State::ScanCode(self.inner.found())}
}

impl ptsd::Validate<&mut String, bool> for PelicanUi<Box<dyn ValidFn>> {
    fn validate(&mut self, ctx: &mut Context, value: &mut String) -> bool {
        self.inner.validate(ctx, value)
    }
    fn change(&mut self) -> Pin<Box<dyn Future<Output = Option<()>> + '_>> {self.inner.change()}
}
