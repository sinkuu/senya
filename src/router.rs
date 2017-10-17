use {Ctx, Handler};
use futures::{Future, IntoFuture};
use hyper::{Method, Request, Response};
use param::FromParameters;
use pattern::{CompiledPatternSet, Pattern, PatternSet};
use std::error::Error;
use std::sync::Arc;
use util::{Control, HttpMethodMap};
use vec_map::VecMap;

pub type RouteHandler = Arc<
    Fn(Request)
        -> Box<Future<Item = Response, Error = Box<Error + Send + 'static>> + 'static>,
>;

pub struct Router {
    routes: HttpMethodMap<CompiledPathRouter>,
}

impl Router {
    #[inline]
    pub fn builder() -> RouterBuilder {
        RouterBuilder::new()
    }

    #[inline]
    pub fn is_match(&self, method: &Method, path: &str) -> bool {
        assert!(path.starts_with('/'));

        if let Some(pr) = self.routes.get(method) {
            pr.0.is_match(path)
        } else {
            false
        }
    }

    pub fn handler(&self, method: &Method, path: &str) -> Option<RouteHandler> {
        assert!(path.starts_with('/'));

        if let Some(pr) = self.routes.get(method) {
            pr.0.matched_token(path).map(|tok| pr.1[tok].clone())
        } else {
            None
        }
    }
}

pub struct RouterBuilder {
    routes: HttpMethodMap<PathRouter>,
    // err_routes: UncompiledPathRouter,
}

impl RouterBuilder {
    fn new() -> Self {
        RouterBuilder {
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
        use regex::Regex;
        let pattern: Pattern = pattern.parse().expect("failed to parse pattern");
        // TODO: factor these thing
        let re = Regex::new(&pattern.to_re_string()).unwrap();
        let pn = pattern
            .param_names()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        let f = move |req: Request| -> Box<Future<Item = Response, Error = Box<Error + Send>>> {
            // println!("{:?} {:?}", re, pn);
            let params = {
                let ci = re.captures_iter(&req.path()[1..]).next().unwrap();
                let ps = pn.iter()
                    .map(|s| s.as_str())
                    .zip(ci.iter().skip(1).map(|i| i.unwrap().as_str()));
                // TODO: URL decode, POST body parsing
                P::from_parameters(ps).unwrap()
            };

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

    pub fn mount(mut self, pattern: &str, b: RouterBuilder) -> Self {
        let pattern = pattern.parse().expect("failed to parse pattern");
        b.routes.into_each(|k, v| -> Control<()> {
            let new = v.0.prefix(&pattern);
            // nll
            if self.routes.contains_key(&k) {
                self.routes
                    .get_mut(&k)
                    .unwrap()
                    .combine(PathRouter(new, v.1));
            } else {
                self.routes.insert(k.clone(), PathRouter(new, v.1));
            }
            Default::default()
        });
        self
    }

    pub fn build(self) -> Router {
        Router {
            routes: self.routes.map(|_, value| value.compile()),
        }
    }
}


struct PathRouter(PatternSet, VecMap<RouteHandler>);

impl PathRouter {
    fn new() -> Self {
        PathRouter(PatternSet::new(), VecMap::new())
    }

    fn route(&mut self, pattern: Pattern, handler: RouteHandler) -> &mut Self {
        let n = self.1.len();
        assert!(self.0.insert(pattern) == Some(n));
        self.1.insert(n, handler);
        self
    }

    fn compile(self) -> CompiledPathRouter {
        CompiledPathRouter(self.0.compile(), self.1)
    }

    fn combine(&mut self, other: PathRouter) {
        self.0.combine(&other.0);
        let ofs = self.1.len();
        self.1
            .extend(other.1.into_iter().map(|(k, v)| (k + ofs, v)));
    }
}

struct CompiledPathRouter(CompiledPatternSet, VecMap<RouteHandler>);

#[test]
fn test_router() {
    use std::io;

    let b = Router::builder()
        .route(
            Method::Get,
            "/foo/bar",
            |_ctx: Ctx| -> io::Result<Response> { Ok(Response::new().with_body("hello!")) },
        )
        .route(Method::Get, "/foo/bar/baz/?", "hello!!!")
        .mount(
            "/piyo",
            Router::builder()
                .route(Method::Get, "/piyo", "/piyo/piyo")
                .route(
                    Method::Get,
                    "/param/{v}",
                    |ctx: Ctx<(String,)>| -> io::Result<Response> { Ok(Response::new().with_body(ctx.params.0)) },
                ),
        )
        .build();

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
