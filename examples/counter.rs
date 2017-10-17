extern crate anymap;
extern crate hyper;
extern crate senya;

use anymap::AnyMap;
use hyper::{Method, Response};
use senya::Ctx;
use senya::router::Router;
use std::io;
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicIsize, Ordering};

fn main() {
    let mut data = AnyMap::new();
    data.insert(AtomicIsize::new(0));
    let rt = Router::new()
        .route(Method::Get, "/", |ctx: Ctx| -> io::Result<Response> {
            Ok(
                Response::new().with_body(
                    ctx.data
                        .get::<AtomicIsize>()
                        .unwrap()
                        .load(Ordering::Relaxed)
                        .to_string(),
                ),
            )
        })
        .route(
            Method::Get,
            "/incr",
            |ctx: Ctx| -> io::Result<Response> {
                let n = ctx.data
                    .get::<AtomicIsize>()
                    .unwrap()
                    .fetch_add(1, Ordering::Relaxed) + 1;
                Ok(Response::new().with_body(n.to_string()))
            },
        );
    senya::server::serve(
        ("localhost", 8080)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap(),
        rt,
        data,
    ).unwrap();
}
