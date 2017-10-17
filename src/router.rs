use {Ctx, Handler};
use futures::{Future, IntoFuture};
use hyper::{Method, Request, Response};
use param::FromParameters;
use pattern::{CompiledPatternSet, Pattern, PatternSet};
use std::error::Error;
use std::sync::Arc;
use util::{Control, HttpMethodMap};
use vec_map::VecMap;

macro_rules! check_path {
    ($path:expr) => {
        debug_assert!($path.starts_with('/'), "paths must start with '/'");
    };
}

pub type RouteHandler = Arc<
    Fn(Request)
        -> Box<Future<Item = Response, Error = Box<Error + Send>>>,
>;

pub struct Router {
    routes: HttpMethodMap<PathRouter>,
    // err_routes: UncompiledPathRouter,
}

impl Router {
    pub fn new() -> Self {
        Router {
            routes: HttpMethodMap::new(),
            // err_routes: HttpMethodMap::new(),
        }
    }

    pub fn route<H: Handler<P> + 'static, P: FromParameters>(
        mut self,
        method: Method,
        pattern: &str,
        handler: H,
    ) -> Self {
        let pattern: Pattern = pattern.parse().expect("failed to parse pattern");
        let cpat = pattern.compile();
        let f = move |req: Request| -> Box<Future<Item = Response, Error = Box<Error + Send>>> {
            // println!("{:?} {:?}", re, pn);
            let params = cpat.path_to_parameters(req.path()).unwrap();

            let fut = handler
                .call(Ctx {
                    params,
                    request: req,
                })
                .into_future()
                .map_err(|e| Box::new(e) as Box<Error + Send>);
            Box::new(fut)
        };
        let f = Arc::new(f) as RouteHandler;

        if !self.routes.contains_key(&method) {
            let mut pr = PathRouter::new();
            pr.route(pattern, f);
            self.routes.insert(method, pr);
        } else {
            self.routes
                .get_mut(&method)
                .expect("this must not happen")
                .route(pattern, f);
        }

        self
    }

    pub fn mount(mut self, pattern: &str, b: Router) -> Self {
        let pattern = pattern.parse().expect("failed to parse pattern");
        b.routes.into_each(|k, v| -> Control<()> {
            let new = v.0.prefix(&pattern);
            // nll
            if self.routes.contains_key(&k) {
                self.routes
                    .get_mut(&k)
                    .unwrap()
                    .merge(PathRouter(new, v.1));
            } else {
                self.routes.insert(k.clone(), PathRouter(new, v.1));
            }
            Default::default()
        });
        self
    }

    pub fn compile(self) -> CompiledRouter {
        CompiledRouter {
            routes: self.routes.map(|_, value| value.compile()),
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CompiledRouter {
    routes: HttpMethodMap<CompiledPathRouter>,
}

impl CompiledRouter {
    #[inline]
    pub fn is_match(&self, method: &Method, path: &str) -> bool {
        check_path!(path);

        if let Some(pr) = self.routes.get(method) {
            pr.0.is_match(path)
        } else {
            false
        }
    }

    #[inline]
    pub fn handler(&self, method: &Method, path: &str) -> Option<RouteHandler> {
        check_path!(path);

        if let Some(pr) = self.routes.get(method) {
            pr.handler(path)
        } else {
            None
        }
    }
}

impl From<Router> for CompiledRouter {
    fn from(r: Router) -> Self {
        r.compile()
    }
}

struct PathRouter(PatternSet, VecMap<RouteHandler>);

impl PathRouter {
    fn new() -> Self {
        PathRouter(PatternSet::new(), VecMap::new())
    }

    fn route(&mut self, pattern: Pattern, handler: RouteHandler) -> &mut Self {
        let n = self.1.len();
        assert_eq!(self.0.insert(pattern), Some(n));
        self.1.insert(n, handler);
        self
    }

    fn compile(self) -> CompiledPathRouter {
        CompiledPathRouter(self.0.compile(), self.1)
    }

    fn merge(&mut self, other: PathRouter) {
        self.0.merge(&other.0);
        let ofs = self.1.len();
        self.1
            .extend(other.1.into_iter().map(|(k, v)| (k + ofs, v)));
    }
}

struct CompiledPathRouter(CompiledPatternSet, VecMap<RouteHandler>);

impl CompiledPathRouter {
    #[inline]
    fn handler(&self, path: &str) -> Option<RouteHandler> {
        self.0.matched_token(path).map(|tok| Arc::clone(&self.1[tok]))
    }
}

#[test]
fn test_router() {
    use std::io;

    let b = Router::new()
        .route(
            Method::Get,
            "/foo/bar",
            |_ctx: Ctx| -> io::Result<Response> { Ok(Response::new().with_body("hello!")) },
        )
        .route(Method::Get, "/foo/bar/baz/?", "hello!!!")
        .mount(
            "/piyo",
            Router::new()
                .route(Method::Get, "/piyo", "/piyo/piyo")
                .route(
                    Method::Get,
                    "/param/{v}",
                    |ctx: Ctx<(String,)>| -> io::Result<Response> { Ok(Response::new().with_body(ctx.params.0)) },
                ),
        )
        .compile();

    let set = &b.routes.get(&Method::Get).unwrap().0;
    assert_eq!(set.len(), 4);

    assert!(b.is_match(&Method::Get, "/foo/bar"));
    assert!(b.is_match(&Method::Get, "/foo/bar/baz/"));
    assert!(b.is_match(&Method::Get, "/piyo/piyo"));
    assert!(b.is_match(&Method::Get, "/piyo/param/fuga"));
    assert!(!b.is_match(&Method::Get, "/piyo"));
    assert!(!b.is_match(&Method::Get, "/foo/bar/"));
    assert!(!b.is_match(&Method::Get, "/foo"));
}
