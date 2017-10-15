use {Ctx, Handler};
use futures::{Future, IntoFuture};
use hyper::{Method, Request, Response};
use param::FromParameters;
use pattern::{Pattern, PatternSet, UncompiledPatternSet};
use std::error::Error;
use util::{Control, HttpMethodMap};
use vec_map::VecMap;

pub struct Router;

impl Router {
    pub fn new() -> Builder {
        Builder::new()
    }
}

pub struct Builder {
    routes: HttpMethodMap<UncompiledPathRouter>,
    // err_routes: HttpMethodMap<UncompiledPathRouter>,
}

impl Builder {
    fn new() -> Self {
        Builder {
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
        let pattern = pattern.parse().expect("failed to parse pattern");
        let f = move |req: Request| -> Box<Future<Item = Response, Error = Box<Error + Send>>> {
            let fut = handler
                .call(Ctx {
                    parameters: P::from_parameters(vec![]).unwrap(), // TODO
                    request: req,
                })
                .into_future()
                .map_err(|e| Box::new(e) as Box<Error + Send>);
            Box::new(fut)
        };
        let f = Box::new(f) as RouteHandler;

        if !self.routes.contains_key(&method) {
            let mut pr = UncompiledPathRouter::new();
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

    pub fn mount(mut self, pattern: &str, b: Builder) -> Self {
        let pattern = pattern.parse().expect("failed to parse pattern");
        b.routes.into_each(|k, v| -> Control<()> {
            let new = v.0.prefix(&pattern);
            // nll
            if self.routes.contains_key(&k) {
                self.routes
                    .get_mut(&k)
                    .unwrap()
                    .combine(UncompiledPathRouter(new, v.1));
            } else {
                self.routes
                    .insert(k.clone(), UncompiledPathRouter(new, v.1));
            }
            Default::default()
        });
        self
    }
}

type RouteHandler = Box<
    Fn(Request)
        -> Box<Future<Item = Response, Error = Box<Error + Send + 'static>> + 'static>,
>;

struct UncompiledPathRouter(UncompiledPatternSet, VecMap<RouteHandler>);

impl UncompiledPathRouter {
    fn new() -> Self {
        UncompiledPathRouter(UncompiledPatternSet::new(), VecMap::new())
    }

    fn route(&mut self, pattern: Pattern, handler: RouteHandler) -> &mut Self {
        let n = self.1.len();
        assert!(self.0.insert(pattern) == Some(n));
        self.1.insert(n, handler);
        self
    }

    fn compile(self) -> PathRouter {
        PathRouter(self.0.compile(), self.1)
    }

    fn combine(&mut self, other: UncompiledPathRouter) {
        self.0.combine(&other.0);
        let ofs = self.1.len();
        self.1
            .extend(other.1.into_iter().map(|(k, v)| (k + ofs, v)));
    }
}

struct PathRouter(PatternSet, VecMap<RouteHandler>);

#[test]
fn test_router() {
    use std::io;

    let b = Router::new()
        .route(
            Method::Get,
            "/foo/bar",
            |ctx: Ctx| -> io::Result<Response> { Ok(Response::new().with_body("hello!")) },
        )
        .route(Method::Get, "/foo/bar/baz/?", "hello!!!")
        .mount(
            "/piyo",
            Router::new().route(Method::Get, "/piyo", "/piyo/piyo"),
        );
    let set = &b.routes.get(&Method::Get).unwrap().0;
    assert_eq!(set.len(), 3);
    assert!(set.is_match_uncompiled("/foo/bar"));
    assert!(set.is_match_uncompiled("/foo/bar/baz/"));
    assert!(set.is_match_uncompiled("/piyo/piyo"));
}
