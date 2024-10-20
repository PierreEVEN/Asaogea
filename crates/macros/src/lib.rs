use proc_macro::{TokenStream};

#[proc_macro_derive(ResourceObject)]
pub fn derive_answer_fn(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_resource_object(&ast)
}

fn impl_resource_object(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let data: &syn::Data = &ast.data;

    if let syn::Data::Struct(_) = data {
        format!(r#"
        impl Resource for {name} {{
            fn destroy(&mut self, ctx: &CtxAppWindow) {{


            }}
        }}

        impl Drop for {name} {{
            fn drop(&mut self) {{

            }}
        }}
        "#).parse().unwrap()
    } else {
        panic!("The ResourceObject derive macro can only be applied to resources.");
    }
}