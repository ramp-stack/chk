use std::{cell::RefCell, rc::Rc};
use chk::{Context, FormComplete, FormItem, FormValidState, State, Theme};
use chk::page::EditPage;
use pelican_ui::components::TextInput;
use pelican_ui::interface::general::FormEvent;
use prism::drawable::Drawable;
use prism::events::Event;
use ptsd::navigation::{Flow, FlowContainer};

mod support;

struct Editor {
    view: Box<dyn Drawable>,
    ctx: Context,
    saved: Rc<RefCell<Vec<Vec<State>>>>,
}

impl Editor {
    fn new(variant: usize) -> Self {
        let theme = Theme::default();
        let saved = Rc::new(RefCell::new(Vec::new()));
        let output = saved.clone();
        let save = Box::new(move |_: &mut Context, values: &Vec<State>| {
            output.borrow_mut().push(values.clone());
            FormComplete::None
        });
        let item = FormItem::text_with_preset("Name", "Original", None,
            |_: &mut Context, value: String| {
                if value.is_empty() {FormValidState::Invalid} else {FormValidState::Valid}
            });
        let page = match variant {
            0 => EditPage::root(&theme, true, "Edit".into(), None, vec![item], vec![], save),
            1 => EditPage::edit_and_display(&theme, true, "Edit".into(), None, vec![item], vec![], save),
            _ => EditPage::new(&theme, true, "Edit".into(),
                vec![item.build()], vec![], vec![item.validation()], save),
        };
        let view: Box<dyn Drawable> = if variant == 0 {Box::new(page)} else {
            Box::new(chk::FlowWrapper::new(Flow::new(vec![Box::new(page)])))
        };
        Self {view, ctx: support::context(), saved}
    }

    fn page(view: &mut dyn Drawable) -> &mut EditPage {
        if view.as_any().is::<EditPage>() {
            view.downcast_mut::<EditPage>().unwrap()
        } else {
            view.downcast_mut::<chk::FlowWrapper>().unwrap().flow()
                .current.as_mut().unwrap().downcast_mut::<EditPage>().unwrap()
        }
    }

    fn drain(&mut self) {
        let mut pending = self.ctx.events();
        while let Some(event) = pending.pop_front() {
            self.view.request_width();
            self.view.request_height(400.0);
            let layout = self.view.layout((400.0, 800.0));
            self.view.event(&mut self.ctx, &layout, event);
            pending.extend(self.ctx.events());
        }
    }

    fn send(&mut self, event: impl Event) {self.ctx.emit(event); self.drain();}

    fn refresh(&mut self) {
        let id = Self::page(self.view.as_mut()).1.content.id();
        self.send(FormEvent::Refresh(id));
    }

    fn change(&mut self, text: &str) {
        Self::page(self.view.as_mut()).1.content.find::<TextInput>().unwrap()
            .set_value(&mut self.ctx, text);
        self.refresh();
    }

    fn press_save(&mut self) {
        let bumper = Self::page(self.view.as_mut()).1.bumper.as_mut().unwrap();
        bumper.request_width();
        let (_, height) = bumper.request_height(400.0);
        let layout = bumper.layout((400.0, height));
        bumper.event(&mut self.ctx, &layout, Box::new(ptsd::events::Button::Pressed(true)));
        self.drain();
    }
}

#[test]
fn save_requires_valid_changes_and_resets_after_saving() {
    for variant in 0..3 {
        let mut editor = Editor::new(variant);
        editor.press_save();
        assert!(editor.saved.borrow().is_empty(), "disabled before initial validation");
        editor.refresh();
        editor.press_save();
        assert!(editor.saved.borrow().is_empty(), "valid initial values are unchanged");

        editor.change("Changed");
        editor.change("Original");
        editor.press_save();
        assert!(editor.saved.borrow().is_empty(), "reverting edits disables Save");

        editor.change("");
        editor.press_save();
        assert!(editor.saved.borrow().is_empty(), "invalid edits cannot be saved");

        editor.change("Updated");
        editor.press_save();
        assert_eq!(*editor.saved.borrow(), vec![vec![State::Text("Updated".into())]]);
        editor.refresh();
        editor.press_save();
        assert_eq!(editor.saved.borrow().len(), 1, "saved values become the baseline");

        // A later validation result must not re-enable an unchanged form.
        let id = Editor::page(editor.view.as_mut()).1.content.id();
        editor.send(FormEvent::Validated {id, valid: true});
        editor.press_save();
        assert_eq!(editor.saved.borrow().len(), 1);

        editor.change("Another edit");
        editor.press_save();
        assert_eq!(editor.saved.borrow().len(), 2, "further edits can be saved");
    }
}
