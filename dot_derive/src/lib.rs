#![feature(proc_macro_quote)]


use quote::{quote};
use syn::{parse_macro_input, DeriveInput, Data};

#[proc_macro_derive(Dot, attributes(display, graph))]
pub fn derive_dot(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let mut field_impl = vec![];
    let mut graph_impl = vec![];

    match input.data {
        Data::Struct(data) => for field in data.fields {
            for attr in &field.attrs {
                let name = match &field.ident {
                    Some(ident) => ident,
                    None => continue,
                };

                if let Some(path) = attr.meta.path().get_ident() {
                    if path == "display" {
                        field_impl.push(quote!{
                            write!(output, "|{}: {}", stringify!(#name), self.#name)?;
                        });
                    }

                    if path == "graph" {
                        field_impl.push(quote!{
                            write!(output, "|<{}> {}", stringify!(#name), stringify!(#name))?;
                        });

                        graph_impl.push(quote!(
                            {
                                let to_label = self.#name.dot(output)?;

                                writeln!(output, "{}:{} -> {};", label, stringify!(#name), to_label)?;
                            }
                        ));
                    }
                }
            }
        },

        _ => (),
    }

    let expanded = quote! {
        impl dot::Dot for #name
        where
            #name: dot::DotLabel
        {
            fn dot(&self, output: &mut dyn std::io::Write) -> std::io::Result<String> {
                let label = self.dot_label();

                write!(output, "{} [ shape = record, label = \"{}", label, stringify!(#name))?;

                #( #field_impl )*

                writeln!(output,"\" ];")?;

                #( #graph_impl )*

                Ok(label)
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}
