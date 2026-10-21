//! Background work for the Elm loop.

use iced::futures::channel::oneshot;
use iced::Task;

/// Runs `work` on its own thread and delivers the result as one message. If the thread cannot
/// start or panics, no message arrives.
pub fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Task<T> {
    let (sender, receiver) = oneshot::channel();
    let _ = std::thread::Builder::new().spawn(move || {
        let _ = sender.send(work());
    });
    // `and_then` skips the `None` of a dropped sender.
    Task::future(async move { receiver.await.ok() }).and_then(Task::done)
}
