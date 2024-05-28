#![feature(proc_macro_diagnostic)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
  parse_macro_input, punctuated::Punctuated, DeriveInput, Ident, Token,
};

/// Automagically inserts the `register_component!` macro call after this,
/// just so its easier to read.
///
/// You can call this like `register_component(marker)` to automagically
/// insert a stub implementation of Component that registers nothing.
#[proc_macro_attribute]
pub fn register_component(
  args: TokenStream,
  input: TokenStream,
) -> TokenStream {
  // we want this puttable on structs/enums/unions i guess
  // so pretend it's a derive input
  let input = parse_macro_input!(input as DeriveInput);
  let struct_name = input.ident.clone();

  let (impl_generics, ty_generics, where_clause) =
    input.generics.split_for_impl();

  let directives = parse_macro_input!(
    args with Punctuated::<Ident, Token![,]>::parse_terminated
  );

  let mut marker = false;
  for directive in directives.iter() {
    match directive.to_string().as_str() {
      "marker" => {
        marker = true;
      }
      _ => directive
        .span()
        .unwrap()
        .error("only `marker` can go here")
        .emit(),
    }
  }

  let mut expanded = quote! {
    // attr macros don't automatically pass through the body
    // so do that
    #input

    ::palkia::manually_register_component!(#struct_name);
  };

  if marker {
    expanded.extend(quote! {
      impl #impl_generics ::palkia::component::Component
        for #struct_name #ty_generics #where_clause {
        fn register(
          builder: ::palkia::component::ComponentRegisterer<Self>
        ) -> ::palkia::component::ComponentRegisterer<Self>
        where
          Self: Sized {
          builder
        }
      }
    })
  }

  TokenStream::from(expanded)
}

/// Automagically derive `Message`.
///
/// This literally just pastes in `impl Message for Foo {}`.
#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let struct_name = input.ident;

  let (impl_generics, ty_generics, where_clause) =
    input.generics.split_for_impl();

  let expanded = quote! {
    impl #impl_generics palkia::messages::Message for #struct_name #ty_generics #where_clause {
      // No - op
    }
  };

  TokenStream::from(expanded)
}

/// Automagically derive `Resource`.
///
/// This literally just pastes in `impl Resource for Foo {}`, then calls
/// the registerer macro.
#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let struct_name = input.ident;

  let (impl_generics, ty_generics, where_clause) =
    input.generics.split_for_impl();

  let expanded = quote! {
    impl #impl_generics palkia::resource::Resource for #struct_name #ty_generics #where_clause {
      // No - op
    }

    ::palkia::manually_register_resource!(#struct_name);
  };

  TokenStream::from(expanded)
}
