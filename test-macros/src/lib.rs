use proc_macro::TokenStream;

/// Prepends one runtime-specific test attribute.
fn wrap(attribute: &str, item: TokenStream) -> TokenStream {
  let mut output: TokenStream = attribute.parse().expect("invalid generated test attribute");
  output.extend(item);
  output
}

/// Declares a Tokio async test.
#[proc_macro_attribute]
pub fn tokio_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
  wrap("#[tokio::test]", item)
}

/// Declares an async-std async test.
#[proc_macro_attribute]
pub fn async_std_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
  wrap("#[async_std::test]", item)
}

/// Declares a Smol async test through smol-macros.
#[proc_macro_attribute]
pub fn smol_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
  wrap("#[macro_rules_attribute::apply(smol_macros::test!)]", item)
}
