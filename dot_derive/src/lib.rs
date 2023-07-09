#![feature(proc_macro_quote)]


use quote::{quote};
use syn::{parse_macro_input, DeriveInput, Data};

#[proc_macro_derive(Dot, attributes(Display, Graph))]
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
                    if path == "Display" {
                        field_impl.push(quote!{
                            write!(output, "|{}: {}", stringify!(#name), self.#name)?;
                        });
                    }

                    if path == "Graph" {
                        field_impl.push(quote!{
                            write!(output, "|<{}> {}", stringify!(#name), stringify!(#name))?;
                        });

                        graph_impl.push(quote!(
                            {
                                let mut entry_label = String::new();

                                entry_label += label;
                                entry_label += ":";
                                entry_label += stringify!(#name);

                                self.#name.dot(output, &entry_label)?;
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
        {
            fn dot(&self, output: &mut dyn std::io::Write, label: &str) -> std::io::Result<()> {
                write!(output, "{} [ shape = record, label = \"{}", label, stringify!(#name))?;

                #( #field_impl )*

                writeln!(output,"\" ];")?;

                #( #graph_impl )*

                Ok(())
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}
