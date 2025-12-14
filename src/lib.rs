// src/lib.rs

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FnArg, ImplItemFn, Pat,
    PatIdent,
};
// =======================
// 1. derive 宏: DynamicGet
// =======================

// 然后宏实现：
#[proc_macro_derive(DynamicGet)]
pub fn derive_dynamic_get(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return syn::Error::new_spanned(struct_name, "Only named fields are supported")
                    .to_compile_error()
                    .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(struct_name, "Only structs are supported")
                .to_compile_error()
                .into();
        }
    };

    let mut get_field_match_arms = Vec::new();
    let mut has_field_match_arms = Vec::new();
    let mut field_names_vec = Vec::new();

    for field in fields {
        let field_ident = field.ident.as_ref().unwrap();
        let field_name_str = field_ident.to_string();

        get_field_match_arms.push(quote! {
            #field_name_str => Some(&self.#field_ident as &dyn std::any::Any)
        });

        has_field_match_arms.push(quote! {
            #field_name_str => true
        });

        field_names_vec.push(field_name_str);
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

            fn field_names(&self) -> Vec<String> {
                vec![
                    #( #field_names_vec.to_string(), )*
                ]
            }
        }
    };

    TokenStream::from(expanded)
}

/// 动态方法属性宏
///
/// 用法：
/// #[dynamic_method(StructName)]
/// pub fn method_name(&self, ...) -> ... { ... }
///
/// 或者：
/// #[dynamic_method(StructName)]
/// pub fn method_name(&mut self, ...) -> ... { ... }
#[proc_macro_attribute]
pub fn dynamic_method(args: TokenStream, input: TokenStream) -> TokenStream {
    let struct_name: syn::Ident = parse_macro_input!(args as syn::Ident);
    let method = parse_macro_input!(input as ImplItemFn);
    let method_name = &method.sig.ident;
    let sig = &method.sig;

    // 检查是否有接收器（self）
    let has_receiver = sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, FnArg::Receiver(_)));
    if !has_receiver {
        return syn::Error::new_spanned(
            &method.sig.ident,
            "dynamic_method can only be used on methods (functions with self)",
        )
        .to_compile_error()
        .into();
    }

    // 确定接收器类型：是否为 &mut self
    let mut needs_mut = false;
    let mut found_self = false;

    for arg in &sig.inputs {
        if let FnArg::Receiver(receiver) = arg {
            found_self = true;
            needs_mut = receiver.mutability.is_some();
            break; // 只关心第一个参数（应该是self）
        }
    }

    if !found_self {
        return syn::Error::new_spanned(
            &method.sig.ident,
            "Method must have self as the first parameter",
        )
        .to_compile_error()
        .into();
    }

    // 生成唯一 const 名
    let const_ident = syn::Ident::new(
        &format!("__DYNAMIC_METHOD_{}_{}", struct_name, method_name)
            .replace("-", "_")
            .to_uppercase(),
        method_name.span(),
    );

    // 解析参数：跳过 self，只处理显式参数
    let mut arg_downcasts = Vec::new();
    let mut call_args = Vec::new();
    let mut arg_index = 0usize;

    for arg in &sig.inputs {
        match arg {
            // 跳过 self 参数
            FnArg::Receiver(_) => continue,
            FnArg::Typed(pat_type) => {
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
                                return None;
                            };
                        });
                        call_args.push(quote! { #temp_var.clone() });
                    }
                } else {
                    return syn::Error::new_spanned(
                        &pat_type.pat,
                        "Only simple identifiers (_) are supported in function args",
                    )
                    .to_compile_error()
                    .into();
                }

                arg_index += 1;
            }
        }
    }

    // 返回类型
    let _ret_ty = match &sig.output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };

    // 原始方法定义
    let item = &method;

    // 根据方法类型生成不同的代码
    if needs_mut {
        // 可变方法
        let expanded = quote! {
            #item

            const #const_ident: () = {
                use ::alanthinker_dynamic_get_field_trait::MethodInfo;
                use ::inventory;

                inventory::submit! {
                    MethodInfo::Mutable {
                        name: stringify!(#method_name),
                        call: move |obj: &mut dyn ::std::any::Any, args: &[&dyn ::std::any::Any]| -> Option<Box<dyn ::std::any::Any>> {
                            #(#arg_downcasts)*

                            if let Some(this) = obj.downcast_mut::<#struct_name>() {
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
    } else {
        // 不可变方法
        let expanded = quote! {
            #item

            const #const_ident: () = {
                use ::alanthinker_dynamic_get_field_trait::MethodInfo;
                use ::inventory;

                inventory::submit! {
                    MethodInfo::Immutable {
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
}
