use futures::Future;
use hyper::{self, Request, Response};
use hyper::server::{Http, NewService, Service};
use router::{CompiledRouter, Router};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use anymap::AnyMap;

// TODO: builder FTW

pub fn serve(addr: SocketAddr, router: Router, data: AnyMap) -> hyper::Result<()> {
    let router = Arc::new(router.compile());
    let data = Arc::new(data);
    let newsvc = HyperNewService(router, data);
    let server = Http::new().bind(&addr, newsvc).unwrap();
    server.run()
}

struct HyperNewService(Arc<CompiledRouter>, Arc<AnyMap>);

impl NewService for HyperNewService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Instance = HyperService;

    #[inline]
    fn new_service(&self) -> io::Result<Self::Instance> {
        Ok(HyperService(Arc::clone(&self.0), Arc::clone(&self.1)))
    }
}

struct HyperService(Arc<CompiledRouter>, Arc<AnyMap>);

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
        Box::new(h(req, Arc::clone(&self.1)).map_err(|e| unimplemented!("error occured: {}", e)))
    }
}
