use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Automagically derive `Message`.
///
/// This literally just pastes in `impl Message for Foo {}`.
#[proc_macro_derive(Message)]
pub fn derive_message(
  input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let struct_name = input.ident;

  let (impl_generics, ty_generics, where_clause) =
    input.generics.split_for_impl();

  let expanded = quote! {
    impl #impl_generics palkia::messages::Message for #struct_name #ty_generics #where_clause {
      // No - op
    }
  };

  proc_macro::TokenStream::from(expanded)
}
