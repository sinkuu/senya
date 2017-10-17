//! Sen'ya micro web-framework.

extern crate anymap;
extern crate futures;
extern crate fxhash;
extern crate hyper;
extern crate itertools;
#[macro_use]
extern crate matches;
extern crate regex;
extern crate vec_map;

use anymap::AnyMap;
use futures::IntoFuture;
use hyper::{Request, Response};
use std::collections::HashMap;
use std::error::Error;
use std::ops::Deref;
use std::sync::Arc;

pub(crate) mod pattern;
pub mod param;
pub mod router;
pub mod server;
pub(crate) mod util;

pub struct Ctx<P = HashMap<String, String>> {
    pub params: P,
    pub data: Arc<AnyMap>,
    pub request: Request,
}

impl<P> Deref for Ctx<P> {
    type Target = Request;

    fn deref(&self) -> &Request {
        &self.request
    }
}

pub trait Handler<P>: 'static {
    type Result: IntoFuture<Item = Response, Error = Self::Error> + 'static;
    type Error: Error + Send + 'static;

    fn call(&self, ctx: Ctx<P>) -> Self::Result;
}

impl<P, F, R, E> Handler<P> for F
where
    F: 'static + Fn(Ctx<P>) -> R,
    R: IntoFuture<Item = Response, Error = E> + 'static,
    E: Error + Send + 'static,
{
    type Result = R;
    type Error = E;

    fn call(&self, ctx: Ctx<P>) -> R {
        (self)(ctx)
    }
}

impl Handler<()> for &'static str {
    type Result = Result<Response, hyper::Error>;
    type Error = hyper::Error;

    #[inline]
    fn call(&self, _: Ctx<()>) -> Self::Result {
        Ok(Response::new().with_body(self.to_string()))
    }
}
