use proc_macro::TokenStream;
use proc_macro2::Ident;
use proc_macro_error::abort;
use quote::{quote, ToTokens};
use syn::{
    parse::{discouraged::Speculative, Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Semi,
    Result as SynResult,
};
use syn_solidity::{Type, TypeArray, TypeMapping};

pub fn storage_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as StorageInput);

    let output = generate_code(input).unwrap_or_else(|err| abort!(err.span(), err.to_string()));

    TokenStream::from(output)
}

fn generate_code(input: StorageInput) -> SynResult<proc_macro2::TokenStream> {
    let mut expanded = proc_macro2::TokenStream::new();

    for (index, item) in input.items.iter().enumerate() {
        let item_tokens = match item {
            StorageItem::Mapping(mapping) => mapping_impl(&mapping, index)?,
            StorageItem::Array(array) => array_impl(&array, index)?,
        };
        expanded.extend(item_tokens);
    }

    Ok(expanded)
}

#[derive(Clone, Debug, PartialEq)]
struct ExtendedTypeMapping {
    pub type_mapping: TypeMapping,
    pub ident: Ident,
}

impl Parse for ExtendedTypeMapping {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let type_mapping: TypeMapping = input.parse()?;
        let ident: Ident = input.parse()?;

        Ok(Self {
            type_mapping,
            ident,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ExtendedTypeArray {
    pub type_array: TypeArray,
    pub ident: Ident,
}

impl Parse for ExtendedTypeArray {
    fn parse(input: ParseStream) -> SynResult<Self> {
        eprintln!("Parsing ExtendedTypeArray...");

        let ty: Type = input.parse()?;
        eprintln!("Parsed type: {:?}", ty);

        let type_array: TypeArray = match ty {
            Type::Array(array) => array,
            _ => return Err(input.error("Expected an array type")),
        };

        eprintln!("Parsed type_array: {:?}", type_array);
        let ident: Ident = input.parse()?;
        eprintln!("Parsed ident: {:?}", ident);

        Ok(Self { type_array, ident })
    }
}

#[derive(Clone, Debug)]
struct StorageInput {
    items: Punctuated<StorageItem, Semi>,
}

impl Parse for StorageInput {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let items = input.parse_terminated(StorageItem::parse, Semi)?;
        Ok(StorageInput { items })
    }
}

#[derive(Clone, Debug)]
enum StorageItem {
    Mapping(ExtendedTypeMapping),
    Array(ExtendedTypeArray),
}

impl Parse for StorageItem {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let fork = input.fork();
        if let Ok(parsed) = fork.parse::<ExtendedTypeArray>() {
            input.advance_to(&fork);
            return Ok(StorageItem::Array(parsed));
        }
        let fork = input.fork();
        if let Ok(parsed) = fork.parse::<ExtendedTypeMapping>() {
            input.advance_to(&fork);
            return Ok(StorageItem::Mapping(parsed));
        }

        Err(input.error("Failed to parse input as ExtendedTypeMapping or ExtendedTypeArray"))
    }
}

fn mapping_impl(
    mapping: &ExtendedTypeMapping,
    index: usize,
) -> SynResult<proc_macro2::TokenStream> {
    eprintln!(">>>>mapping_impl: {:#?}", mapping.type_mapping);
    let args = process_mapping(&mapping.type_mapping);
    eprintln!(">>>>args: {:#?}", args);

    let ident = &mapping.ident;

    let expanded = quote! {
        struct #ident {
            slot: U256,
        }
        impl #ident {
            const SLOT: U256 = U256::from(#index);
            fn new(slot: U256) -> Self {
                Self { slot: U256::from(#index) }
            }
            fn key(&self, #(#args),*) -> U256 {
                U256::from(0)
            }
        }
    };
    Ok(expanded)
}
#[derive(Debug)]
struct Arg {
    name: Ident,
    ty: Ident,
    is_output: bool,
}

impl ToTokens for Arg {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let ty = &self.ty;
        tokens.extend(quote! { #name: #ty });
    }
}

fn process_mapping(mapping: &TypeMapping) -> Vec<Arg> {
    let mut args = Vec::new();
    let mut current_mapping = mapping;
    let mut i = 0;

    loop {
        let mut arg = Arg {
            name: Ident::new(&format!("arg{}", i), proc_macro2::Span::call_site()),
            ty: Ident::new(
                &current_mapping.key.to_string(),
                proc_macro2::Span::call_site(),
            ),
            is_output: false,
        };
        if let Some(key_name) = &current_mapping.key_name {
            arg.name = key_name.0.clone(); // Извлечение идентификатора из SolIdent
        }
        args.push(arg);
        i += 1;

        match &*current_mapping.value {
            Type::Custom(custom_value) => {
                // let value_arg = Arg {
                //     name: Ident::new("output", proc_macro2::Span::call_site()),
                //     ty: Ident::new(&custom_value.to_string(), proc_macro2::Span::call_site()),
                //     is_output: true,
                // };
                // args.push(value_arg);
                return args;
            }
            Type::Mapping(inner_mapping) => {
                current_mapping = inner_mapping;
            }
            _ => {
                return args;
            }
        }
    }
}

fn array_impl(array: &ExtendedTypeArray, index: usize) -> SynResult<proc_macro2::TokenStream> {
    eprintln!(">>>>array_impl");
    let ident = &array.ident;

    let expanded = quote! {
        struct #ident {
            slot: U256,
        }
        impl #ident {
            const SLOT: U256 = U256::from(#index);

            fn new(slot: U256) -> Self {
                Self { slot: U256::from(#index) }
            }
        }
    };
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_process_mapping_single_level() {
        let mapping: TypeMapping = parse_quote! {
            mapping(Address => MyStruct)
        };

        let args = process_mapping(&mapping);

        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name.to_string(), "arg0");
        assert_eq!(args[0].ty.to_string(), "Address");
    }

    #[test]
    fn test_process_mapping_nested() {
        let mapping: TypeMapping = parse_quote! {
            mapping(Address owner => mapping(Address users => mapping(Address balances => MyStruct)))
        };

        let args = process_mapping(&mapping);

        assert_eq!(args.len(), 3);
        assert_eq!(args[0].name.to_string(), "owner");
        assert_eq!(args[0].ty.to_string(), "Address");

        assert_eq!(args[1].name.to_string(), "users");
        assert_eq!(args[1].ty.to_string(), "Address");

        assert_eq!(args[2].name.to_string(), "balances");
        assert_eq!(args[2].ty.to_string(), "Address");
    }
}
