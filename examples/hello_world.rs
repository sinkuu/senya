extern crate hyper;
extern crate senya;

use hyper::{Method, Response};
use senya::Ctx;
use senya::router::Router;
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
            // A specific route take precedence of a generic one.
            "/param/{p}",
            |ctx: Ctx<(String,)>| -> io::Result<Response> {
                Ok(Response::new().with_body(ctx.params.0))
            },
        );
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
