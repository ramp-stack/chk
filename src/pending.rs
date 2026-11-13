use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Wake, Waker};

/// Shares one local future and its result between independently cancelable waiters.
pub(crate) struct Pending<T: Clone> {
    state: Rc<RefCell<PendingState<T>>>,
    wake: Arc<PendingWake>,
    waiter: Option<usize>,
}
struct PendingState<T> {
    future: Option<Pin<Box<dyn Future<Output = T>>>>,
    result: Option<T>,
}
struct PendingWake(Mutex<Vec<Option<Waker>>>);
impl Wake for PendingWake {
    fn wake(self: Arc<Self>) {self.wake_by_ref();}
    fn wake_by_ref(self: &Arc<Self>) {
        let waiters = self.0.lock().unwrap().clone();
        for waker in waiters.into_iter().flatten() {waker.wake();}
    }
}
impl<T: Clone> Pending<T> {
    pub(crate) fn new(future: impl Future<Output = T> + 'static) -> Self {
        Self {
            state: Rc::new(RefCell::new(PendingState {future: Some(Box::pin(future)), result: None})),
            wake: Arc::new(PendingWake(Mutex::new(Vec::new()))),
            waiter: None,
        }
    }
}
impl<T: Clone> Clone for Pending<T> {
    fn clone(&self) -> Self {
        Self {state: self.state.clone(), wake: self.wake.clone(), waiter: None}
    }
}
impl<T: Clone> Drop for Pending<T> {
    fn drop(&mut self) {
        let waker = self.waiter.and_then(|waiter| self.wake.0.lock().unwrap().get_mut(waiter).and_then(Option::take));
        drop(waker);
    }
}
impl<T: Clone> Future for Pending<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut state = this.state.borrow_mut();
        if let Some(result) = &state.result {return Poll::Ready(result.clone());}
        let caller = cx.waker().clone();
        let previous = {
            let mut waiters = this.wake.0.lock().unwrap();
            let waiter = *this.waiter.get_or_insert_with(|| {
                waiters.iter().position(Option::is_none).unwrap_or_else(|| {
                    waiters.push(None);
                    waiters.len() - 1
                })
            });
            waiters[waiter].replace(caller)
        };
        drop(previous);
        let Some(mut future) = state.future.take() else {return Poll::Pending;};
        drop(state);
        let waker = Waker::from(this.wake.clone());
        match future.as_mut().poll(&mut TaskContext::from_waker(&waker)) {
            Poll::Pending => {
                this.state.borrow_mut().future = Some(future);
                Poll::Pending
            }
            Poll::Ready(result) => {
                this.state.borrow_mut().result = Some(result.clone());
                let waiters = std::mem::take(&mut *this.wake.0.lock().unwrap());
                for waker in waiters.into_iter().flatten() {waker.wake();}
                Poll::Ready(result)
            }
        }
    }
}
