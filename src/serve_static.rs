use {Ctx, Handler};
use hyper::Response;
use hyper::header::ContentType;
use mime_guess;
use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{self, Path, PathBuf};

header! {
    (XContentTypeOptions, "X-Content-Type-Options") => Cow[str]
}

#[derive(Debug)]
pub struct ServeStatic {
    base: PathBuf,
    guess: bool,
    nosniff: bool,
}

impl ServeStatic {
    pub fn new<P: AsRef<Path>>(base: P) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
            guess: true,
            nosniff: true,
        }
    }

    pub fn guess_content_type(mut self, yes: bool) -> Self {
        self.guess = yes;
        self
    }

    pub fn nosniff(mut self, yes: bool) -> Self {
        self.nosniff = yes;
        self
    }
}

impl Handler<(String,)> for ServeStatic {
    type Result = Result<Response, Error>;
    type Error = Error;

    fn call(&self, ctx: Ctx<(String,)>) -> Self::Result {
        let path: String = ctx.params.0.parse().map_err(|_| Error::Parse)?;
        let path = Path::new(&path);
        for c in path.components() {
            if !matches!(c, path::Component::Normal(..)) {
                return Err(Error::Parse);
            }
        }
        let mut base = self.base.clone();
        base.push(path);

        let mut res = Response::new();

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            res = res.with_header(ContentType(mime_guess::get_mime_type(ext)));
            if self.nosniff {
                res = res.with_header(XContentTypeOptions(Cow::from("nosniff")));
            }
        }
        let mut f = BufReader::new(File::open(base)?);
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Ok(res.with_body(buf))
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Parse,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref io) => io.description(),
            Error::Parse => "parse error",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::Io(ref io) => Some(io as &StdError),
            Error::Parse => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref io) => io.fmt(f),
            Error::Parse => write!(f, "parse error"),
        }
    }
}
