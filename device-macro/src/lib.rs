use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input, Ident, ImplItem, Token, Visibility,
};

struct DevicesEnum {
    ident: Ident,
    devices: Vec<Ident>,
    vis: Visibility,
}

impl Parse for DevicesEnum {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis: Visibility = input.parse()?;
        <Token![enum]>::parse(input)?;
        let ident: Ident = input.parse()?;

        let content;
        braced!(content in input);

        let mut devices = Vec::new();
        while !content.is_empty() {
            devices.push(<Device>::parse(&content)?.0);
        }

        Ok(Self {
            ident,
            devices,
            vis,
        })
    }
}

struct Device(Ident);

impl Parse for Device {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;

        let content;
        parenthesized!(content in input);

        let _device_struct: Ident = content.parse()?;
        <Token![,]>::parse(input)?;

        Ok(Self(ident))
    }
}

#[proc_macro_derive(Devices)]
pub fn devices(input: TokenStream) -> TokenStream {
    let DevicesEnum { ident, devices, .. } = parse_macro_input!(input as DevicesEnum);

    quote! {
        #[async_trait::async_trait]
        impl crate::LedDevice for #ident {
            async fn connect(&mut self) -> io::Result<()> {
                match self {
                    #(#ident::#devices(d) => d.connect().await,)*
                }
            }

            async fn on_event(&mut self, event: crate::Event) -> io::Result<()> {
                match self {
                    #(#ident::#devices(d) => d.on_event(event).await,)*
                }
            }
        }
    }
    .into()
}
