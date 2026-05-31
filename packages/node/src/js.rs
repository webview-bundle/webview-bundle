/// Source codes are from [rolldown](https://github.com/rolldown/rolldown)
/// See: https://github.com/rolldown/rolldown/blob/fc5ec4dbb8cf7a9bc32f2cba6e0e82eba3ac888d/crates/rolldown_binding/src/types/js_callback.rs#L98
use napi::bindgen_prelude::{FromNapiValue, JsValuesTupleIntoVec};
use napi::threadsafe_function::{
  ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue,
};
use napi::{Either, Error, Status};
use std::future::Future;
use std::sync::Arc;

pub type JsCallback<Args = (), Ret = ()> =
  Arc<ThreadsafeFunction<Args, Either<Ret, UnknownReturnValue>, Args, Status, false, true>>;

pub trait JsCallbackExt<Args, Ret> {
  /// Invoke the JS callback asynchronously and return its return value.
  fn invoke_async(&self, args: Args) -> impl Future<Output = Result<Ret, Error>> + Send;

  /// Fire-and-forget: enqueue the JS callback on the Node.js event loop and return
  /// immediately, without waiting for or retrieving its return value.
  fn fire_and_forgot(&self, args: Args) -> Status;
}

impl<Args, Ret> JsCallbackExt<Args, Ret> for JsCallback<Args, Ret>
where
  Args: 'static + Send + JsValuesTupleIntoVec,
  Ret: 'static + Send + FromNapiValue,
  Either<Ret, UnknownReturnValue>: FromNapiValue,
{
  async fn invoke_async(&self, args: Args) -> Result<Ret, Error> {
    match self.call_async(args).await? {
      Either::A(ret) => Ok(ret),
      Either::B(_) => unknown_return_err::<Ret>(),
    }
  }

  fn fire_and_forgot(&self, args: Args) -> Status {
    self.call(args, ThreadsafeFunctionCallMode::NonBlocking)
  }
}

fn unknown_return_err<Ret>() -> Result<Ret, Error> {
  let js_type = "unknown";
  Err(Error::new(
    Status::InvalidArg,
    format!("UNKNOWN_RETURN_VALUE. Cannot convert {js_type}"),
  ))
}
