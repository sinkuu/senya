extern crate hyper;
extern crate senya;

use hyper::{Method, Response};
use senya::Ctx;
use senya::router::Router;
use senya::serve_static::ServeStatic;
use std::io;
use std::net::ToSocketAddrs;

fn main() {
    let rt = Router::new()
        .route(
            Method::Get,
            // Matches everything under `/`.
            "/:path",
            "hello world!",
        )
        .route(
            Method::Get,
            // Matches `/param/*`.
            // A specific route takes precedence of a generic one.
            "/param/{p}",
            |ctx: Ctx| -> io::Result<Response> {
                Ok(Response::new().with_body(format!("p = {}", ctx.params["p"])))
            },
        )
        .route(Method::Get, "/static/:path", ServeStatic::new("./examples"));
    senya::server::serve(
        ("localhost", 8080)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap(),
        rt,
        Default::default(),
    ).unwrap();
}
