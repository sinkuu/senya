use futures::Future;
use hyper::{self, Request, Response};
use hyper::server::{Http, NewService, Service};
use router::{CompiledRouter, Router};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

pub fn serve(addr: SocketAddr, router: Router) -> hyper::Result<()> {
    let router = Arc::new(router.compile());
    let newsvc = HyperNewService(router);
    let server = Http::new().bind(&addr, newsvc).unwrap();
    server.run()
}

struct HyperNewService(Arc<CompiledRouter>);

impl NewService for HyperNewService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Instance = HyperService;

    #[inline]
    fn new_service(&self) -> io::Result<Self::Instance> {
        Ok(HyperService(Arc::clone(&self.0)))
    }
}

struct HyperService(Arc<CompiledRouter>);

impl Service for HyperService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    #[inline]
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
