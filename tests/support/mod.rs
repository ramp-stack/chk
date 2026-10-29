use std::marker::PhantomData;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use prism::{Context, Window, hardware, traits};

#[derive(Clone, Default)]
struct TestKeyboard(Arc<AtomicBool>);

impl traits::Keyboard for TestKeyboard {
    fn show(&self, _: f32) {self.0.store(true, Ordering::Relaxed);}
    fn hide(&self) {self.0.store(false, Ordering::Relaxed);}
    fn visible(&self) -> bool {self.0.load(Ordering::Relaxed)}
}

#[derive(Clone, Default)]
struct TestHardware(TestKeyboard);

struct CancelledPick<T>(PhantomData<T>);
impl<T: Clone + 'static> traits::Picked<T> for CancelledPick<T> {
    async fn get(&mut self) -> hardware::PickResult<T> {Err(hardware::Cancelled)}
}
impl<T: Clone + 'static> traits::Picker<T> for TestHardware {
    type Picked = CancelledPick<T>;
    fn pick(&self) -> Self::Picked {CancelledPick(PhantomData)}
}

impl traits::Hardware for TestHardware {
    fn keyboard(&self) -> hardware::Keyboard {hardware::Keyboard::new(self.0.clone())}
    fn player(&self, _: media::Player) -> Result<hardware::Player, String> {
        panic!("navigation tests must not start media playback")
    }
    fn start_camera(&self) -> Result<hardware::Camera, String> {panic!("unexpected camera")}
    fn start_microphone(&self) -> Result<hardware::Microphone, String> {panic!("unexpected microphone")}
    fn start_capture(&self) -> Result<hardware::Capture, String> {panic!("unexpected capture")}
    fn share(&self, _: &hardware::Data) {panic!("unexpected share")}
    fn set_clipboard(&self, _: &hardware::Data) {panic!("unexpected clipboard write")}
    fn get_clipboard(&self) -> Option<hardware::Data> {None}
    fn set_cache(&self, _: &str, _: &hardware::Data) -> Result<(), String> {Ok(())}
    fn get_cache(&self, _: &str) -> Option<hardware::Data> {None}
    fn trigger_haptic(&self) {}
}

#[derive(Clone)]
struct TestAir;
impl traits::Air for TestAir {
    fn me(&self) -> prism::air::Name {panic!("navigation tests must not access Air")}
    fn instances(&self, _: prism::air::ContractType) -> Box<dyn traits::DynInstances> {
        panic!("navigation tests must not access Air")
    }
    fn start(&self, _: prism::service::ServiceInit) {panic!("unexpected service")}
    fn shutdown(&self, _: prism::service::Key) {panic!("unexpected service")}
}

pub fn context() -> Context {
    Context::new(TestHardware::default(), prism::air::Air::new(TestAir),
        Window {size: (400.0, 800.0), is_mobile: true, ..Window::default()})
}
