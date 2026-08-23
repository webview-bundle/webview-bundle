use wvb::util::cancellation::Cancellation;

pub struct WvbCancellation {
  pub(crate) inner: Cancellation,
}

#[unsafe(no_mangle)]
pub extern "C" fn wvb_cancellation_new() -> *mut WvbCancellation {
  Box::into_raw(Box::new(WvbCancellation {
    inner: Cancellation::new(),
  }))
}

/// # Safety
/// `handle` must be null or a valid `WvbCancellation`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_cancellation_cancel(handle: *const WvbCancellation) {
  if let Some(handle) = unsafe { handle.as_ref() } {
    handle.inner.cancel();
  }
}

/// # Safety
/// `handle` must be null or a valid `WvbCancellation`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_cancellation_is_cancelled(handle: *const WvbCancellation) -> u8 {
  match unsafe { handle.as_ref() } {
    Some(handle) if handle.inner.is_cancelled() => 1,
    _ => 0,
  }
}

/// # Safety
/// `handle` must be null or a pointer previously returned by `wvb_cancellation_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wvb_cancellation_free(handle: *mut WvbCancellation) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

/// The token a nullable handle argument stands for: `None` when null, so a caller that passes no
/// cancellation gets the core's own default.
///
/// # Safety
/// `handle` must be null or a valid `WvbCancellation`.
pub(crate) unsafe fn cancellation_of(handle: *const WvbCancellation) -> Option<Cancellation> {
  unsafe { handle.as_ref() }.map(|h| h.inner.clone())
}
