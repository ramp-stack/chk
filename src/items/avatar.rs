use super::*;

#[derive(Debug, Clone)]
pub enum AvatarPurpose { None, Custom { icon: Icons, style: AvatarIconStyle } }

impl AvatarPurpose {
    pub fn new(icon: Icons, style: AvatarIconStyle) -> Self {
        AvatarPurpose::Custom { icon, style }
    }
}

#[derive(Clone, Component)]
pub struct AvatarInput {
    layout: Stack,
    avatar: crate::design_system::AvatarView,
    #[skip]
    id: Id,
    #[skip]
    picked: Option<Picked<Image>>,
    #[skip]
    form: Option<Uuid>,
}

impl AvatarInput {
    pub fn new(theme: &Theme, content: AvatarContent, flair: Option<(Icons, AvatarIconStyle)>, action: Option<Action>) -> Self {
        let id = Id::random();
        let on_click = action.map(|action| match action {
            Action::PickPhoto => {
                Box::new(move |ctx: &mut Context, _: &Theme| ctx.emit(PickPhoto(id)))
                    as Box<dyn Callback>
            }
            action => action.get(),
        });
        Self {
            layout: Stack::default(),
            avatar: theme.avatar_input(content, flair, on_click),
            id,
            picked: None,
            form: None,
        }
    }
    pub fn content(&self) -> &AvatarContent { self.avatar.content() }
}

impl std::fmt::Debug for AvatarInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvatarInput").field("avatar", &self.avatar).finish()
    }
}

#[on_event]
impl OnEvent<PickPhoto> for AvatarInput {
    fn on(&mut self, ctx: &mut Context, event: PickPhoto) -> Vec<Box<dyn DynEvent>> {
        if event.0 == self.id { self.picked = Some(ctx.hardware.pick::<Image>()); }
        vec![]
    }
}

#[on_change(async {
    match &mut self.picked {
        Some(picked) => picked.next().await,
        None => std::future::pending().await,
    }
})]
impl OnChange<prism::hardware::PickResult<Image>> for AvatarInput {
    fn on_change(&mut self, ctx: &mut Context, result: prism::hardware::PickResult<Image>) {
        self.picked = None;
        if let Ok(image) = result {
            self.avatar.set_content(AvatarContent::image(image));
            if let Some(form) = self.form { ctx.emit(FormEvent::Refresh(form)); }
        }
    }
}

#[on_event]
impl OnEvent<FormEvent> for AvatarInput {
    fn on(&mut self, _ctx: &mut Context, event: FormEvent) -> Vec<Box<dyn DynEvent>> {
        if let FormEvent::Bind(id) = event { self.form = Some(id); }
        prism::events![event]
    }
}

impl crate::InputControl for AvatarInput {
    fn state(&self) -> State {State::Avatar(self.content().clone())}
    fn validation_value(&self) -> Option<String> {Some(self.content().get_image().unwrap_or_default().to_string())}
}
