use itertools::Itertools;
use regex::{self, Regex, RegexSet};
use std::borrow::Cow;
use std::cmp::{Ord, Ordering};
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::iter::FromIterator;
use std::str::FromStr;
use vec_map::VecMap;

macro_rules! check_path {
    ($path:expr) => {
        assert!($path.starts_with('/'), "paths must start with '/'");
    };
}

pub type PatternToken = usize;

#[derive(Debug, Clone)]
pub struct PatternSet {
    /// `Pattern`s in more-specific-first order.
    patterns: BTreeMap<Pattern, PatternToken>,
    next_tok: PatternToken,
}

impl FromIterator<Pattern> for PatternSet {
    fn from_iter<I: IntoIterator<Item = Pattern>>(iter: I) -> Self {
        let mut next_tok = 0;
        let patterns = BTreeMap::from_iter(iter.into_iter().enumerate().map(|(tok, pat)| {
            next_tok = tok;
            (pat, tok)
        }));
        next_tok += 1;
        PatternSet { patterns, next_tok }
    }
}

impl Default for PatternSet {
    fn default() -> Self {
        PatternSet::new()
    }
}

impl PatternSet {
    #[inline]
    pub fn new() -> Self {
        PatternSet {
            patterns: BTreeMap::new(),
            next_tok: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    #[inline]
    pub fn insert(&mut self, pat: Pattern) -> Option<PatternToken> {
        if self.patterns.contains_key(&pat) {
            return None;
        }
        self.patterns.insert(pat, self.next_tok);
        let tok = self.next_tok;
        self.next_tok = self.next_tok.checked_add(1).expect("token overflow");
        Some(tok)
    }

    pub fn is_match(&self, path: &str) -> bool {
        self.patterns.keys().any(|pat| pat.is_match(path))
    }

    pub fn matched_token(&self, path: &str) -> Option<PatternToken> {
        for (pat, tok) in &self.patterns {
            if pat.is_match(path) {
                return Some(*tok);
            }
        }

        None
    }

    pub fn compile(&self) -> CompiledPatternSet {
        let mut map = VecMap::with_capacity(self.patterns.len());
        let re_set = RegexSet::new(self.patterns.iter().enumerate().map(|(i, (pat, &tok))| {
            let re = pat.to_re_string();
            let names = pat.param_names().map(|s| s.to_string()).collect::<Vec<_>>();
            map.insert(
                i,
                (
                    tok,
                    if names.is_empty() {
                        None
                    } else {
                        Some(Regex::new(&re).expect("failed to compile regex"))
                    },
                    names,
                ),
            );
            re
        }));
        CompiledPatternSet {
            re_set: re_set.expect("failed to compile regex"),
            map,
        }
    }

    pub fn prefix(&self, prefix: &Pattern) -> PatternSet {
        assert!(
            !prefix.terminated(),
            r#""{}" is terminated pattern"#,
            prefix
        );
        let mut patterns = BTreeMap::new();
        for (pat, &tok) in &self.patterns {
            let mut p = prefix.clone();
            p.segments.extend(pat.segments.iter().cloned());
            p.terminator = pat.terminator.clone();
            patterns.insert(p, tok);
        }
        PatternSet {
            patterns,
            next_tok: self.next_tok,
        }
    }

    pub fn combine(&mut self, other: &PatternSet) {
        let new_next_tok = self.next_tok + other.next_tok;
        let off = self.next_tok;
        self.patterns.extend(
            other
                .patterns
                .iter()
                .map(|(pat, tok)| (pat.clone(), tok + off)),
        );
        self.next_tok = new_next_tok;
    }
}

#[derive(Clone, Debug)]
pub struct CompiledPatternSet {
    re_set: RegexSet,
    map: VecMap<(PatternToken, Option<Regex>, Vec<String>)>,
}

impl CompiledPatternSet {
    pub fn len(&self) -> usize {
        self.re_set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.re_set.len() == 0
    }

    #[inline]
    pub fn is_match(&self, path: &str) -> bool {
        check_path!(path);
        let path = &path[1..];
        self.re_set.is_match(path)
    }

    #[inline]
    pub fn matched_token(&self, path: &str) -> Option<PatternToken> {
        check_path!(path);
        let path = &path[1..];
        self.re_set
            .matches(path)
            .iter()
            .next()
            .and_then(|i| self.map.get(i).map(|&(tok, _, _)| tok))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Pattern {
    segments: Vec<Segment>,
    terminator: Option<Terminator>,
}

impl Pattern {
    pub fn new() -> Pattern {
        Default::default()
    }

    pub fn with_capacity(cap: usize) -> Pattern {
        Pattern {
            segments: Vec::with_capacity(cap),
            terminator: None,
        }
    }

    pub fn terminated(&self) -> bool {
        self.terminator.is_some()
    }

    pub fn push(&mut self, segment: Segment) {
        if let Some(ref term) = self.terminator {
            panic!(
                r#"tried to push a segment to the pattern already terminated with "{:?}""#,
                term.expr(),
            );
        }
        if let Segment::Fixed(ref name) = segment {
            assert!(
                self.segments.iter().all(|s| {
                    if let Segment::Fixed(ref s) = *s {
                        s != name
                    } else {
                        true
                    }
                }),
                "duplicated parameter name"
            );
        }
        if let Some(&Segment::Fixed(ref s)) = self.segments.last() {
            assert!(!s.is_empty(), "there cannot be empty segment");
            // except in the last segment
        }
        self.segments.push(segment);
    }

    /// Tests if this pattern matches with `path`.
    pub fn is_match(&self, path: &str) -> bool {
        use self::Segment::*;
        use itertools::EitherOrBoth::*;
        use itertools::Position::*;

        check_path!(path);

        let path = if self.terminator == Some(Terminator::OptionalSlash) && path.ends_with('/') {
            &path[..path.len() - 1]
        } else {
            path
        };

        for p in path.split('/')
            .skip(1)
            .with_position()
            .zip_longest(self.segments.iter())
        {
            match p {
                Both(a, &Fixed(ref b)) => if a.into_inner() != b {
                    return false;
                },
                Both(a, &Parameter(_, allow_empty)) => if !allow_empty && a.into_inner().is_empty()
                {
                    return false;
                },
                Left(_) if matches!(self.terminator, Some(Terminator::Tail(..))) => return true,
                Left(_) | Right(_) => return false,
            }

            if matches!(p, Both(Last(_), _))
                && matches!(self.terminator, Some(Terminator::Tail(..)))
            {
                // `/foo/bar/:path` does not match `/foo/bar`
                return path.ends_with('/');
            }
        }
        true
    }

    // TODO: this should be private
    pub(crate) fn to_re_string(&self) -> String {
        use self::Segment::*;
        use itertools::Position::*;

        let mut out = String::new();
        out.push('^');

        for seg in self.segments.iter().with_position() {
            match *seg.into_inner() {
                Fixed(ref s) => {
                    out.push_str(&regex::escape(s));
                }
                Parameter(_, allow_empty) => {
                    out.push_str(if allow_empty { "([^/]*)" } else { "([^/]+)" });
                }
            }

            if matches!(seg, First(..) | Middle(..)) {
                out.push('/');
            }
        }

        if let Some(t) = self.terminator.as_ref() {
            if !self.segments.is_empty() {
                out.push('/');
            }
            match *t {
                Terminator::OptionalSlash => {
                    out.push('?');
                }
                Terminator::Tail(..) => out.push_str("(.*)"),
            }
        }


        out.push('$');
        out
    }

    // TODO: this should be private
    pub(crate) fn param_names(&self) -> ParamNamesIter {
        ParamNamesIter(&self.segments, 0)
    }
}

impl Ord for Pattern {
    fn cmp(&self, other: &Pattern) -> Ordering {
        use self::Segment::*;

        fn is_empty_fixed_segment(s: &Segment) -> bool {
            if let Fixed(ref s) = *s {
                s.is_empty()
            } else {
                false
            }
        }

        let len_self = self.segments
            .iter()
            .take_while(|s| !is_empty_fixed_segment(s))
            .count();
        let len_other = other
            .segments
            .iter()
            .take_while(|s| !is_empty_fixed_segment(s))
            .count();

        if len_self == len_other {
            for (a, b) in self.segments.iter().zip(other.segments.iter()) {
                match (a, b) {
                    // {a?} ∋ {a}
                    (&Parameter(ref a, oa), &Parameter(ref b, ob)) => {
                        let c = oa.cmp(&ob).then(a.cmp(b));
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                    (&Fixed(ref a), &Fixed(ref b)) => {
                        let c = a.cmp(b);
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                    (&Fixed(..), &Parameter(..)) => return Ordering::Less,
                    (&Parameter(..), &Fixed(..)) => return Ordering::Greater,
                }
            }

            self.terminator.cmp(&other.terminator)
        } else {
            len_other.cmp(&len_self)
        }
    }
}

impl PartialOrd for Pattern {
    #[inline]
    fn partial_cmp(&self, other: &Pattern) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use self::Segment::*;
        use itertools::Position::*;

        write!(f, "/")?;
        for s in self.segments.iter().with_position() {
            match *s.into_inner() {
                Fixed(ref s) => write!(f, "{}", s)?,
                Parameter(ref name, allow_empty) => {
                    write!(f, "{{{}{}}}", name, if allow_empty { "?" } else { "" })?;
                }
            }
            if matches!(s, First(..) | Middle(..)) {
                write!(f, "/")?;
            }
        }
        match self.terminator {
            Some(Terminator::OptionalSlash) => {
                write!(f, "/?")?;
            }
            Some(Terminator::Tail(ref name)) => {
                write!(f, "/:{}", name)?;
            }
            _ => (),
        }
        Ok(())
    }
}

pub(crate) struct ParamNamesIter<'a>(&'a [Segment], usize);

impl<'a> Iterator for ParamNamesIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.1 >= self.0.len() {
            return None;
        }

        for i in self.1..self.0.len() {
            if let Segment::Parameter(ref name, _) = self.0[i] {
                self.1 = i + 1;
                return Some(name);
            }
        }

        self.1 = self.0.len();
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Terminator {
    OptionalSlash,
    Tail(String),
}

impl Terminator {
    fn expr(&self) -> Cow<'static, str> {
        match *self {
            Terminator::OptionalSlash => Cow::from("/?"),
            Terminator::Tail(ref name) => Cow::from(format!(":{}", name)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Segment {
    Fixed(String),
    Parameter(String, bool),
}

impl FromStr for Segment {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with(':') {
            panic!("`:path` must be the last segment"); // TODO: err
        }
        if s.starts_with('{') && s.ends_with('}') {
            let s = &s[1..s.len() - 1];
            let (s, allow_empty) = if s.ends_with('?') {
                (&s[..s.len() - 1], true)
            } else {
                (s, false)
            };
            Ok(Segment::Parameter(s.to_string(), allow_empty))
        } else {
            // TODO: reject illegal URL path
            Ok(Segment::Fixed(s.to_string()))
        }
    }
}

impl FromStr for Pattern {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pat = Pattern::new();
        let mut term = None;

        let s = if s.ends_with("/?") {
            term = Some(Terminator::OptionalSlash);
            &s[..s.len() - 2]
        } else if let Some(last) = s.rfind('/') {
            if s[last + 1..].starts_with(':') {
                term = Some(Terminator::Tail(s[last + 2..].to_string()));
                &s[..last]
            } else {
                s
            }
        } else {
            s
        };

        let s = if s.starts_with('/') { &s[1..] } else { s };

        if !s.is_empty() || term.is_none() {
            for p in s.split('/') {
                pat.push(p.parse()?);
            }
        }

        pat.terminator = term;

        Ok(pat)
    }
}

#[test]
fn test_pattern() {
    let pat1: Pattern = "/foo/bar/{user}".parse().expect("failed to parse");
    assert!(pat1.is_match("/foo/bar/piyo"));
    assert!(!pat1.is_match("/foo/bar/piyo/"));
    assert!(!pat1.is_match("/foo/bar/"));
    assert!(!pat1.is_match("/foo/bar"));
    assert_eq!(pat1.to_re_string(), "^foo/bar/([^/]+)$");
    assert!(pat1.param_names().next().unwrap() == "user");

    let pat2: Pattern = "/foo/bar/{user?}".parse().expect("failed to parse");
    assert!(pat2.is_match("/foo/bar/piyo"));
    assert!(!pat2.is_match("/foo/bar/piyo/"));
    assert!(pat2.is_match("/foo/bar/"));
    assert!(!pat2.is_match("/foo/bar"));
    assert_eq!(pat2.to_re_string(), "^foo/bar/([^/]*)$");

    let pat3: Pattern = "/foo/bar/".parse().expect("failed to parse");
    assert!(!pat3.is_match("/foo/bar/piyo"));
    assert!(!pat3.is_match("/foo/bar/piyo/"));
    assert!(pat3.is_match("/foo/bar/"));
    assert!(!pat3.is_match("/foo/bar"));
    assert_eq!(pat3.to_re_string(), "^foo/bar/$");

    let pat4: Pattern = "/foo/bar/?".parse().expect("failed to parse");
    assert!(!pat4.is_match("/foo/bar/piyo"));
    assert!(!pat4.is_match("/foo/bar/piyo/"));
    assert!(pat4.is_match("/foo/bar/"));
    assert!(pat4.is_match("/foo/bar"));
    assert_eq!(pat4.to_re_string(), "^foo/bar/?$");

    let pat5: Pattern = "/foo/bar/:path".parse().expect("failed to parse");
    assert!(pat5.is_match("/foo/bar/piyo"));
    assert!(pat5.is_match("/foo/bar/piyo/"));
    assert!(pat5.is_match("/foo/bar/piyo/piyo"));
    assert!(pat5.is_match("/foo/bar/"));
    assert!(!pat5.is_match("/foo/bar"));
    assert_eq!(pat5.to_re_string(), "^foo/bar/(.*)$");

    assert!(pat2 > pat1);
    assert!(pat3 > pat1);
    assert!(pat4 > pat1);
    assert!(pat5 > pat1);

    assert!(pat3 > pat2);
    assert!(pat4 > pat3);
    assert!(pat5 > pat4);

    let pats = vec![pat1, pat2, pat3, pat4, pat5];
    let pset = PatternSet::from_iter(pats.clone().into_iter());
    assert!(pset.next_tok == 5);
    assert!(pset.patterns.keys().eq(pats.iter()), "{:?}", pset.patterns);

    assert_eq!(pset.matched_token("/foo/bar/"), Some(1)); // `pat3` is unreachable
    assert_eq!(pset.matched_token("/foo/bar/piyo"), Some(0));
    assert_eq!(pset.matched_token("/foo/bar"), Some(3));
    assert_eq!(pset.matched_token("/foo/bar/piyo/"), Some(4));

    let pset = pset.compile();
    assert_eq!(pset.matched_token("/foo/bar/"), Some(1));
    assert_eq!(pset.matched_token("/foo/bar/piyo"), Some(0));
    assert_eq!(pset.matched_token("/foo/bar"), Some(3));
    assert_eq!(pset.matched_token("/foo/bar/piyo/"), Some(4));

    let p: Pattern = "/:path".parse().unwrap();
    assert_eq!(p.to_re_string(), "^(.*)$");
    assert!(p.is_match("/hugahuga"));
    assert!(p.is_match("/"));

    let p: Pattern = "/".parse().unwrap();
    assert_eq!(p.to_re_string(), "^$");
    assert!(!p.is_match("/hugahuga"));
    assert!(p.is_match("/"));
}
