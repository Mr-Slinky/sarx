//! Runs a future to completion on the current thread.
//!
//! A future is a value that produces its result later, like a `CompletableFuture` in Java. To poll
//! a future is to ask it to make as much progress as it can. Unlike a Java future, a Rust future
//! does not run until something polls it.
//!
//! Setting up the GPU is asynchronous, meaning `Renderer::new` returns a future. The methods that
//! winit calls on the app are not `async`, and only an `async` function can use `.await` to wait
//! for that future. The app therefore passes the future to [`block_on`], which polls it on the
//! calling thread until it finishes.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

/// Runs a future on the current thread until it finishes, and returns its output.
///
/// `block_on` polls the future in a loop. Each poll returns `Poll::Ready` with the output, or
/// `Poll::Pending` where the future has to wait.
///
/// On `Poll::Pending`, `block_on` parks the thread, which puts the thread to sleep. Once the future
/// can make progress, whatever it is waiting on wakes the thread through a [`ThreadWaker`], and
/// `block_on` then polls the future again.
///
/// # Examples
///
/// The first future in the example is ready on its first poll. The second one waits for the GPU.
///
/// ```ignore
/// // Returns the output at once, since the first poll returns `Poll::Ready`
/// let answer = block_on(async { 6 * 7 });
/// assert_eq!(answer, 42);
///
/// // Polls the renderer's setup until it finishes, parking the thread while it waits
/// let renderer_setup = Renderer::new(Arc::clone(&window), size.width, size.height);
/// let renderer = block_on(renderer_setup)?;
/// ```
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let thread_waker = Arc::new(ThreadWaker(thread::current()));
    let waker = Waker::from(thread_waker);
    let mut context = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

/// Wakes the thread that [`block_on`] parked.
///
/// The waker stores the `Thread` that called `block_on`. A future that returns `Poll::Pending`
/// keeps a copy of the waker. Once the future can be polled again, the code that it is waiting on
/// calls `wake` on that copy.
struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    /// Unparks the stored thread.
    ///
    /// The thread then returns from `thread::park`, and `block_on` polls the future again.
    ///
    /// # Examples
    ///
    /// The example builds a waker for the current thread, then wakes the thread through it.
    ///
    /// ```ignore
    /// // Builds a waker for the current thread, then wakes the thread through it
    /// let thread_waker = Arc::new(ThreadWaker(thread::current()));
    /// let waker = Waker::from(thread_waker);
    /// waker.wake();
    /// ```
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
