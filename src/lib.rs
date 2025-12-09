// src/lib.rs

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FnArg, ImplItemFn, Pat, PatIdent};
// =======================
// 1. derive 宏: DynamicGet
// =======================

#[proc_macro_derive(DynamicGet)]
pub fn derive_dynamic_get(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return syn::Error::new_spanned(struct_name, "Only named fields are supported").to_compile_error().into();
            }
        },
        _ => {
            return syn::Error::new_spanned(struct_name, "Only structs are supported").to_compile_error().into();
        }
    };

    let mut get_field_match_arms = Vec::new();
    let mut has_field_match_arms = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();

        get_field_match_arms.push(quote! {
            #field_name_str => Some(&self.#field_name as &dyn std::any::Any)
        });

        has_field_match_arms.push(quote! {
            #field_name_str => true
        });
    }

    let expanded = quote! {
        impl DynamicGetter for #struct_name {
            fn get_field(&self, name: &str) -> Option<&dyn std::any::Any> {
                match name {
                    #(#get_field_match_arms,)*
                    _ => None,
                }
            }

            fn has_field(&self, name: &str) -> bool {
                match name {
                    #(#has_field_match_arms,)*
                    _ => false,
                }
            }
        }
    };

    TokenStream::from(expanded)
}

// =======================
// 2. attribute 宏: dynamic_method
// =======================
#[proc_macro_attribute]
pub fn dynamic_method(args: TokenStream, input: TokenStream) -> TokenStream {
    let struct_name: syn::Ident = parse_macro_input!(args as syn::Ident);
    let method = parse_macro_input!(input as ImplItemFn);
    let method_name = &method.sig.ident;
    let sig = &method.sig;

    // 生成唯一 const 名
    let const_ident = syn::Ident::new(&format!("__DYNAMIC_METHOD_{}_{}", struct_name, method_name).replace("-", "_").to_uppercase(), method_name.span());

    // 解析参数：跳过 self，只处理显式参数
    let mut arg_downcasts = Vec::new();
    let mut call_args = Vec::new();
    let mut arg_index = 0usize;

    for arg in &sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            let ty = &*pat_type.ty;
            let temp_var = syn::Ident::new(&format!("__arg_{}", arg_index), pat_type.span());

            // 只支持简单 ident 或 _
            if let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat {
                if ident == "_" {
                    arg_downcasts.push(quote! {
                        let _: Option<&#ty> = args.get(#arg_index).and_then(|a| a.downcast_ref());
                    });
                } else {
                    arg_downcasts.push(quote! {
                        let Some(#temp_var): Option<&#ty> = args.get(#arg_index).and_then(|a| a.downcast_ref()) else {
                            ::std::eprint!(
                                "Failed to downcast arg {} (expected: {})\n",
                                #arg_index,
                                std::any::type_name::<#ty>()
                            );
                            return None;
                        };
                    });
                    call_args.push(quote! { #temp_var.clone() });
                }
            } else {
                return syn::Error::new_spanned(&pat_type.pat, "Only simple identifiers (_) are supported in function args")
                    .to_compile_error()
                    .into();
            }

            arg_index += 1;
        }
        // 忽略 self
    }

    // 返回类型
    let _ret_ty = match &sig.output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };

    let item = &method;

    let expanded = quote! {
        #item

        const #const_ident: () = {
            use ::alanthinker_dynamic_get_field_trait::MethodInfo;
            use ::inventory;

            inventory::submit! {
                MethodInfo {
                    name: stringify!(#method_name),
                    call: move |obj: &dyn ::std::any::Any, args: &[&dyn ::std::any::Any]| -> Option<Box<dyn ::std::any::Any>> {
                        #(#arg_downcasts)*

                        if let Some(this) = obj.downcast_ref::<#struct_name>() {
                            let result = this.#method_name(#(#call_args),*);
                            return Some(Box::new(result));
                        }

                        None
                    }
                }
            };
        };
    };

    TokenStream::from(expanded)
}
