extern crate hyper;
extern crate senya;

use senya::router::Router;
use senya::Ctx;
use hyper::{Method, Response};
use std::net::ToSocketAddrs;
use std::io;

fn main() {
    let rt = Router::builder()
        .route(Method::Get, "/:path", "hello world!")
        .route(
            Method::Get,
            "/param/{p}", // most specific route is chosen
            |ctx: Ctx<(String,)>| -> io::Result<Response> { Ok(Response::new().with_body(ctx.params.0)) },
        )
        .build();
    senya::server::serve(
        ("localhost", 8080)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap(),
        rt,
    ).unwrap();
}
