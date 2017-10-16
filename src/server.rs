use futures::Future;
use hyper::{self, Request, Response};
use hyper::server::{Http, NewService, Service};
use router::Router;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

pub fn serve(addr: SocketAddr, router: Router) -> hyper::Result<()> {
    let router = Arc::new(router);
    let newsvc = HyperNewService(router);
    let server = Http::new().bind(&addr, newsvc).unwrap();
    server.run()
}

struct HyperNewService(Arc<Router>);

impl NewService for HyperNewService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Instance = HyperService;

    fn new_service(&self) -> io::Result<Self::Instance> {
        Ok(HyperService(Arc::clone(&self.0)))
    }
}

struct HyperService(Arc<Router>);

impl Service for HyperService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        // println!(
        //     "{} {}",
        //     req.path(),
        //     self.0.is_match(req.method(), req.path())
        // );
        let h = self.0.handler(req.method(), req.path()).unwrap();
        Box::new(h(req).map_err(|_| unimplemented!()))
    }
}
