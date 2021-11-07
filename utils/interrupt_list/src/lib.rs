#![feature(proc_macro_hygiene)]

extern crate proc_macro;

use proc_macro::TokenStream;

use syn::parse_macro_input;
use syn::AttributeArgs;
use syn::Lit;
use syn::NestedMeta;

#[proc_macro_attribute]
pub fn interrupt_list(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let enum_name = parse_macro_input!(attrs as syn::Ident);

    let items = parse_macro_input!(item as MacroState);

    let m_vis = items.module.0;
    let m_nam = items.module.1;

    let names = items
        .interrupts
        .iter()
        .map(|a| {
            let ident = format!("{}", a.ident);

            let mut res = String::with_capacity(ident.len());
            let mut capitalize = true;
            for c in ident.chars() {
                if capitalize {
                    res.push(c.to_ascii_uppercase());
                    capitalize = false;
                    continue;
                }
                if c == ' ' || c == '_' {
                    capitalize = true;
                    continue;
                }
                res.push(c);
            }
            quote::format_ident!("{}", res)
        })
        .collect::<Vec<_>>();

    let values = items.interrupts.iter().map(|a| a.number);

    let allowed = items.allowed;

    #[cfg(feature = "libx64")]
    let libx64_impl = quote::quote! {
        impl libx64::idt::TrustedUserInterruptIndex for #enum_name {}
    };

    #[cfg(not(feature = "libx64"))]
    let libx64_impl = quote::quote! {};

    quote::quote! {
        #m_vis mod #m_nam {

            #[derive(Debug, Clone, Copy, Eq, PartialEq)]
            #[repr(u8)]
            pub enum #enum_name {
                #(#names = #values),*
            }

            #libx64_impl

            impl From<#enum_name> for usize {
                fn from(v: #enum_name) -> usize {
                    usize::from(v as u8)
                }

            }

            #(#allowed)*
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn user_interrupt(initial_attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = initial_attr.clone();
    let attrs = parse_macro_input!(attrs as AttributeArgs);

    if attrs.len() != 1 {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(initial_attr),
            "Wrong number of arguments for macro, expected a single interrupt number",
        )
        .into_compile_error()
        .into();
    }
    let lit = if let Some(lit) = attrs.into_iter().next() {
        lit
    } else {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(initial_attr),
            "Wrong number of arguments for macro, expected a single interrupt number",
        )
        .into_compile_error()
        .into();
    };

    match lit {
        NestedMeta::Lit(Lit::Int(int)) => {
            let num = match int.base10_parse::<u32>() {
                Ok(num) => num,
                Err(err) => return err.into_compile_error().into(),
            };
            if !(32..256).contains(&num) {
                return syn::Error::new_spanned(
                    int,
                    "Invalid interrupt number, user interrupts must be between index 32 and 255",
                )
                .into_compile_error()
                .into();
            }
        }
        _ => {
            return syn::Error::new_spanned(lit, "Expected a u8 integer value")
                .into_compile_error()
                .into();
        }
    }

    let temp = item.clone();
    parse_macro_input!(item as ValidateInterruptSignature);
    temp
}

struct InterruptInfo {
    ident: syn::Ident,
    number: Option<u8>,
}

struct MacroState {
    module: (syn::Visibility, syn::Ident),
    interrupts: Vec<InterruptInfo>,
    allowed: Vec<syn::Item>,
}

impl syn::parse::Parse for MacroState {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let f = input.parse::<syn::ItemMod>()?;

        let module_name = f.ident.clone();
        let module_vis = f.vis.clone();

        let all = f.content.as_ref().unwrap().1.iter().map(|i| match i {
            e @ (syn::Item::Const(_) |
                 syn::Item::Static(_) |
                 syn::Item::Macro(_) | 
                 syn::Item::Fn(_) |
                 syn::Item::Use(_)) => Ok(e),
            _ => Err(syn::Error::new_spanned(
                i,
                "Item kind is not allow within the module, you can only define constants and x86 interrupt functions",
            )),
        });

        all.clone().find(Result::is_err).transpose()?;

        let mut interrupts = vec![];
        let mut allowed = vec![];

        for item in all.filter(Result::is_ok).map(Result::unwrap).cloned() {
            match item {
                syn::Item::Fn(f) => {
                    let number: Option<u8> = f.attrs.iter().find_map(|attr| {
                        if attr
                            .path
                            .segments
                            .last()
                            .map(|item| item.ident == "user_interrupt")
                            .unwrap_or_default()
                        {
                            let s = format!("{}", attr.tokens);
                            str::parse(&s[1..(s.len() - 1)]).ok()
                        } else {
                            None
                        }
                    });

                    let ident = f.sig.ident.clone();
                    interrupts.push(InterruptInfo { ident, number });
                    allowed.push(syn::Item::Fn(f));
                }
                i @ (syn::Item::Const(_)
                | syn::Item::Use(_)
                | syn::Item::Static(_)
                | syn::Item::Macro(_)) => allowed.push(i),
                _ => unreachable!(),
            }
        }
        Ok(Self {
            module: (module_vis, module_name),
            interrupts,
            allowed,
        })
    }
}

#[derive(Debug)]
struct ValidateInterruptSignature;

impl syn::parse::Parse for ValidateInterruptSignature {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<syn::Visibility>()?;
        let a = input.parse::<syn::Signature>()?;
        input.parse::<syn::Block>()?;
        if !a
            .abi
            .as_ref()
            .map(|abi| matches!(abi.name, Some(ref value) if value.value() == "x86-interrupt" ))
            .unwrap_or_default()
            || a.inputs.len() != 1
        {
            Err(syn::Error::new_spanned(
                a.abi,
                "Expected single argument \"x86-interrupt\" function abi",
            ))
        } else {
            Ok(Self {})
        }
    }
}
