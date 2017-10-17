use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::BuildHasher;
use std::str::FromStr;

pub trait FromParameters: Sized {
    fn from_parameters<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        params: I,
    ) -> Result<Self, Cow<'static, str>>;
}

impl FromParameters for () {
    fn from_parameters<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        it: I,
    ) -> Result<Self, Cow<'static, str>> {
        if it.into_iter().next().is_some() {
            Err(Cow::from("excess parameters"))
        } else {
            Ok(())
        }
    }
}

impl<A> FromParameters for (A,)
where
    A: FromStr,
    A::Err: Display,
{
    fn from_parameters<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        params: I,
    ) -> Result<Self, Cow<'static, str>> {
        let mut params = params.into_iter();
        let (_, value) = params.next().ok_or_else(|| Cow::from("missing parameter"))?;
        let t = (value.parse().map_err(|e: A::Err| Cow::from(e.to_string()))?,);
        if params.next().is_some() {
            Err(Cow::from("excess parameters"))
        } else {
            Ok(t)
        }
    }
}

macro_rules! tuple_from_parameters {
    ($($tv:ident),+) => {
        impl<$($tv: FromStr),*> FromParameters for ($($tv),*)
        where
            $($tv::Err: Display),+
        {
            fn from_parameters<'a, It: IntoIterator<Item = (&'a str, &'a str)>>(
                params: It,
            ) -> Result<Self, Cow<'static, str>> {
                let mut params = params.into_iter();
                let t = (
                    $(params.next().ok_or(Cow::from("missing parameter"))?.1
                        .parse().map_err(|e: $tv::Err| Cow::from(e.to_string()))?),*
                );
                if params.next().is_some() {
                    Err(Cow::from("excess parameters"))
                } else {
                    Ok(t)
                }
            }
        }
    };
}

tuple_from_parameters!(A, B);
tuple_from_parameters!(A, B, C);
tuple_from_parameters!(A, B, C, D);
tuple_from_parameters!(A, B, C, D, E);
tuple_from_parameters!(A, B, C, D, E, F);
tuple_from_parameters!(A, B, C, D, E, F, G);
tuple_from_parameters!(A, B, C, D, E, F, G, H);
tuple_from_parameters!(A, B, C, D, E, F, G, H, I);
tuple_from_parameters!(A, B, C, D, E, F, G, H, I, J);

impl<T, S: BuildHasher + Default> FromParameters for HashMap<String, T, S>
where
    T: FromStr,
    T::Err: Display,
{
    fn from_parameters<'a, I: IntoIterator<Item = (&'a str, &'a str)>>(
        params: I,
    ) -> Result<HashMap<String, T, S>, Cow<'static, str>> {
        let params = params.into_iter();
        let mut map = HashMap::with_capacity_and_hasher(params.size_hint().0, S::default());
        for (name, val) in params {
            map.insert(
                name.to_string(),
                val.parse().map_err(|e: T::Err| Cow::from(e.to_string()))?,
            );
        }
        Ok(map)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tuple() {
        assert_eq!(
            <(i32,)>::from_parameters(vec![("foo", "1234")]).unwrap(),
            (1234,)
        );
        assert_eq!(
            <(i32, String)>::from_parameters(vec![("foo", "1234"), ("bar", "baz")]).unwrap(),
            (1234, "baz".into())
        );
        assert_eq!(
            <(i32, i32)>::from_parameters(vec![]),
            Err(Cow::from("missing parameter"))
        );
        assert!(
            <(i32,)>::from_parameters(vec![("foo", "1234"), ("bar", "baz")]).is_err()
        );
    }

    #[test]
    fn map() {
        assert!(
            HashMap::<String, i32>::from_parameters(vec![("foo", "0"), ("bar", "1")].into_iter())
                .unwrap()
                .len() == 2
        );
        assert!(
            ::fxhash::FxHashMap::<String, i32>::from_parameters(
                vec![("foo", "0"), ("bar", "1")].into_iter()
            ).unwrap()
                .len() == 2
        );
    }
}
