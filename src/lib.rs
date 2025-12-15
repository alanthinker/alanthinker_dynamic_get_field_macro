// src/lib.rs

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FnArg, ImplItem,
    ItemImpl, Pat, PatIdent, Type,
};
// =======================
// 宏: DynamicGet
// =======================
#[proc_macro_derive(dynamic_fields)]
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


fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c.is_uppercase() {
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    
    result
}

// 新宏: #[dynamic_methods] 应用于impl块
#[proc_macro_attribute]
pub fn dynamic_methods(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut impl_block = parse_macro_input!(input as ItemImpl);

    let struct_type = match &*impl_block.self_ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                segment.ident.clone()
            } else {
                return syn::Error::new_spanned(&impl_block.self_ty, "Expected simple struct type")
                    .to_compile_error()
                    .into();
            }
        }
        _ => {
            return syn::Error::new_spanned(&impl_block.self_ty, "Expected simple struct type")
                .to_compile_error()
                .into()
        }
    };

    let mut registrations = Vec::new();

    for item in &mut impl_block.items {
        if let ImplItem::Fn(method) = item {
            let method_name = &method.sig.ident;
            let sig = &method.sig;

            let receiver = sig.inputs.first();
            let is_static = !sig
                .inputs
                .iter()
                .any(|arg| matches!(arg, FnArg::Receiver(_)));
            let mut needs_mut = false;
            if !is_static {
                if let Some(FnArg::Receiver(r)) = receiver {
                    needs_mut = r.mutability.is_some();
                }
            }

            // 获取所有原始参数名
            let mut param_names = Vec::new();
            let start_index = if is_static { 0 } else { 1 };
            
            for arg in sig.inputs.iter().skip(start_index) {
                if let FnArg::Typed(pat_type) = arg {
                    if let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat {
                        param_names.push(ident.clone());
                    }
                }
            }

            let const_ident = syn::Ident::new(
                &format!("__DYNAMIC_METHOD_{}_{}", struct_type, method_name)
                    .replace("-", "_")
                    .to_uppercase(),
                method_name.span(),
            );

            // 生成唯一的包装器函数名
            let snake_struct_type = to_snake_case(&struct_type.to_string());
            let snake_method_name = to_snake_case(&method_name.to_string());
            let wrapper_name = syn::Ident::new(
                &format!("__wrapper_{}_{}", snake_struct_type, snake_method_name),
                method_name.span(),
            );

            let mut arg_downcasts = Vec::new();
            let mut call_args = Vec::new();
            let mut arg_index = 0usize;
            let start_index = if is_static { 0 } else { 1 };

            for (_i, (arg, param_name)) in sig.inputs.iter().skip(start_index)
                .zip(param_names.iter()).enumerate() 
            {
                if let FnArg::Typed(pat_type) = arg {
                    let ty = &pat_type.ty;
                    let param_name_str = param_name.to_string();

                    let temp_var =
                        syn::Ident::new(&format!("{}_in_{}", param_name_str, method_name), pat_type.span());
                    
                    let (downcast_ty, arg_expr) = match &**ty {
                        Type::Reference(type_ref) => {
                            let inner_ty = &type_ref.elem;
                            let downcast_ty = quote! { #inner_ty };
                            
                            let arg_expr = if type_ref.mutability.is_some() {
                                quote! { &mut #temp_var }
                            } else {
                                quote! { &#temp_var }
                            };
                            
                            (downcast_ty, arg_expr)
                        }
                        Type::Path(_) | Type::Tuple(_) | Type::Array(_) | Type::Slice(_) => {
                            let downcast_ty = quote! { #ty };
                            let arg_expr = quote! { * #temp_var };
                            
                            (downcast_ty, arg_expr)
                        }
                        _ => {
                            return syn::Error::new_spanned(
                                ty,
                                format!(r#"Unsupported argument type for method "{}""#, method_name)
                            )
                            .to_compile_error()
                            .into();
                        }
                    };
                    
                    if let Pat::Ident(PatIdent { ident: _, .. }) = &*pat_type.pat {
                        arg_downcasts.push(quote! {
                            let #temp_var = args.get(#arg_index)
                                .ok_or_else(|| ::anyhow::anyhow!(r#"Missing argument name: "{}" in index: {} for method: "{}""#, #param_name_str, #arg_index, stringify!(#method_name)))?
                                .downcast_ref::<#downcast_ty>()
                                .ok_or_else(|| ::anyhow::anyhow!(
                                    r#"Argument name: "{}" in index: {} for method: "{}" must be of type: "&{}""#, 
                                    #param_name_str, 
                                    #arg_index,
                                    stringify!(#method_name), 
                                    std::any::type_name::<#downcast_ty>()
                                ))?;
                        });
                        call_args.push(arg_expr);
                        arg_index += 1;
                    } else {
                        return syn::Error::new_spanned(
                            &pat_type.pat,
                            "Only simple identifiers supported",
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            }

            // 生成包装器函数而不是直接使用闭包
            let wrapper = if is_static {
                quote! {
                    fn #wrapper_name(args: &[&dyn ::std::any::Any]) -> ::anyhow::Result<Box<dyn ::std::any::Any>> {
                        #(#arg_downcasts)*
                        let result = #struct_type::#method_name(#(#call_args),*);
                        Ok(Box::new(result))
                    }
                }
            } else if needs_mut {
                quote! {
                    fn #wrapper_name(obj: &mut dyn ::std::any::Any, args: &[&dyn ::std::any::Any]) -> ::anyhow::Result<Box<dyn ::std::any::Any>> {
                        #(#arg_downcasts)*
                        let this = obj.downcast_mut::<#struct_type>()
                            .ok_or_else(|| ::anyhow::anyhow!(r#"Failed to downcast object to type "{}""#, std::any::type_name::<#struct_type>()))?;
                        let result = this.#method_name(#(#call_args),*);
                        Ok(Box::new(result))
                    }
                }
            } else {
                quote! {
                    fn #wrapper_name(obj: &dyn ::std::any::Any, args: &[&dyn ::std::any::Any]) -> ::anyhow::Result<Box<dyn ::std::any::Any>> {
                        #(#arg_downcasts)*
                        let this = obj.downcast_ref::<#struct_type>()
                            .ok_or_else(|| ::anyhow::anyhow!(r#"Failed to downcast object to type "{}""#, std::any::type_name::<#struct_type>()))?;
                        let result = this.#method_name(#(#call_args),*);
                        Ok(Box::new(result))
                    }
                }
            };

            let registration = if is_static {
                quote! {
                    #wrapper
                    
                    const #const_ident: () = {
                        use ::alanthinker_dynamic_get_field_trait::{MethodInfo, MethodKind};
                        use ::inventory;
                        inventory::submit! {
                            MethodInfo {
                                type_id: std::any::TypeId::of::<#struct_type>(),
                                name: stringify!(#method_name),
                                kind: MethodKind::Static {
                                    call: #wrapper_name
                                }
                            }
                        };
                    };
                }
            } else if needs_mut {
                quote! {
                    #wrapper
                    
                    const #const_ident: () = {
                        use ::alanthinker_dynamic_get_field_trait::{MethodInfo, MethodKind};
                        use ::inventory;
                        inventory::submit! {
                            MethodInfo {
                                type_id: std::any::TypeId::of::<#struct_type>(),
                                name: stringify!(#method_name),
                                kind: MethodKind::Mutable {
                                    call: #wrapper_name
                                }
                            }
                        };
                    };
                }
            } else {
                quote! {
                    #wrapper
                    
                    const #const_ident: () = {
                        use ::alanthinker_dynamic_get_field_trait::{MethodInfo, MethodKind};
                        use ::inventory;
                        inventory::submit! {
                            MethodInfo {
                                type_id: std::any::TypeId::of::<#struct_type>(),
                                name: stringify!(#method_name),
                                kind: MethodKind::Immutable {
                                    call: #wrapper_name
                                }
                            }
                        };
                    };
                }
            };

            registrations.push(registration);
        }
    }

    let expanded = quote! {
        #impl_block
        #(#registrations)*
    };

    TokenStream::from(expanded)
}