use bstr::BStr;

/// Converts string to a bstr
fn b(s: &str) -> &BStr {
    s.into()
}

mod normalize;

mod boolean;

mod integer;

mod color;

mod path;
