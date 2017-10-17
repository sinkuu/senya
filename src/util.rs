use hyper::Method;
use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::iter::FromIterator;

macro_rules! http_method_map {
    ($($hyper_method:tt, $name:ident);+;) => {
        http_method_map!($($hyper_method, $name);+);
    };
    ($($hyper_method:tt, $name:ident);+) => {
        pub struct HttpMethodMap<T> {
            $($name: Option<T>),+,
            extensions: HashMap<String, T>,
        }

        #[allow(unused)]
        impl<T> HttpMethodMap<T> {
            pub fn new() -> Self {
                Self {
                    $($name: None),+,
                    extensions: HashMap::new(),
                }
            }
            pub fn insert(&mut self, method: Method, value: T) -> Option<T> {
                match method {
                    $(Method::$hyper_method => {
                        let prev = self.$name.take();
                        self.$name = Some(value);
                        prev
                    }),+,
                    Method::Extension(s) => {
                        self.extensions.insert(s, value)
                    }
                }
            }

            pub fn get(&self, method: &Method) -> Option<&T> {
                match *method {
                    $(Method::$hyper_method => {
                        self.$name.as_ref()
                    }),+,
                    Method::Extension(ref s) => {
                        self.extensions.get(s)
                    }
                }
            }

            pub fn get_mut(&mut self, method: &Method) -> Option<&mut T> {
                match *method {
                    $(Method::$hyper_method => {
                        self.$name.as_mut()
                    }),+,
                    Method::Extension(ref s) => {
                        self.extensions.get_mut(s)
                    }
                }
            }

            pub fn contains_key(&self, method: &Method) -> bool {
                match *method {
                    $(Method::$hyper_method => self.$name.is_some()),+,
                    Method::Extension(ref s) => self.extensions.contains_key(s),
                }
            }

            pub fn for_each<F: FnMut(&Method, &T) -> Control<B>, B>(&self, mut f: F) -> Option<B> {
                $(
                    if let Some(ref val) = self.$name.as_ref() {
                        if let Control::Break(b) = f(&Method::$hyper_method, val) {
                            return Some(b);
                        }
                    }

                )+
                for (k, v) in &self.extensions {
                    if let Control::Break(b) = f(&Method::Extension(k.to_string()), v) {
                        return Some(b);
                    }
                }
                None
            }

            pub fn into_each<F: FnMut(Method, T) -> Control<B>, B>(self, mut f: F) -> Option<B> {
                $(
                    if let Some(val) = self.$name {
                        if let Control::Break(b) = f(Method::$hyper_method, val) {
                            return Some(b);
                        }
                    }

                )+
                for (k, v) in self.extensions {
                    if let Control::Break(b) = f(Method::Extension(k), v) {
                        return Some(b);
                    }
                }
                None
            }

            pub fn map<F: Fn(&Method, T) -> U, U>(self, f: F) -> HttpMethodMap<U> {
                HttpMethodMap {
                    $($name: self.$name.map(|v| f(&Method::$hyper_method, v))),+,
                    extensions: HashMap::from_iter(self.extensions.into_iter().map(|(k,v)| {
                        let k = Method::Extension(k);
                        let v = f(&k, v);
                        if let Method::Extension(k) = k {
                            (k, v)
                        } else {
                            unreachable!()
                        }
                    })),
                }
            }
        }

        impl<T: Debug> Debug for HttpMethodMap<T> {
            fn fmt(&self, f: &mut Formatter) -> fmt::Result {
                let mut d = f.debug_map();
                $(
                    if let Some(ref val) = self.$name {
                        d.entry(&stringify!($name), val);
                    }
                )+
                d.entries(self.extensions.iter());
                d.finish()
            }
        }
    };
}

#[allow(unused)]
#[derive(Debug)]
pub enum Control<B> {
    Continue,
    Break(B),
}

impl<B> Default for Control<B> {
    fn default() -> Self {
        Control::Continue
    }
}

http_method_map!(
    Options, options;
    Get, get;
    Post, post;
    Put, put;
    Delete, delete;
    Head, head;
    Trace, trace;
    Connect, connect;
    Patch, patch;
);
