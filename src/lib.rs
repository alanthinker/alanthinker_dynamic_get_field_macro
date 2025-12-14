// src/lib.rs

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FnArg, ImplItem, ImplItemFn,
    ItemImpl, Pat, PatIdent, Path, Type, TypePath,
};
// =======================
// 宏: DynamicGet
// =======================
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
// =======================
// 宏: DynamicGet
// =======================

/// 动态方法属性宏
///
/// 用法：
/// #[dynamic_method(StructName)]
/// pub fn method_name(&self, ...) -> ... { ... }
///
/// 或者：
/// #[dynamic_method(StructName)]
/// pub fn method_name(&mut self, ...) -> ... { ... }
// 新宏: #[dynamic_methods] 应用于impl块
#[proc_macro_attribute]
pub fn dynamic_methods(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut impl_block = parse_macro_input!(input as ItemImpl);

    // 从impl块推导结构体名（类型）
    let struct_type = match &*impl_block.self_ty {
        Type::Path(TypePath {
            path: Path { segments, .. },
            ..
        }) if segments.len() == 1 => {
            let segment = &segments[0];
            segment.ident.clone()
        }
        _ => {
            return syn::Error::new_spanned(&impl_block.self_ty, "Expected simple struct type")
                .to_compile_error()
                .into()
        }
    };

    // 遍历impl块中的所有项，找到方法并为每个生成注册代码
    let mut registrations = Vec::new();
    for item in &mut impl_block.items {
        if let ImplItem::Fn(method) = item {
            // 复制方法签名等
            let method_name = &method.sig.ident;
            let sig = &method.sig;

            // 检查是否有self（必须是方法）
            let has_receiver = sig
                .inputs
                .iter()
                .any(|arg| matches!(arg, FnArg::Receiver(_)));
            if !has_receiver {
                continue; // 跳过非方法
            }

            // 确定是否mut self
            let mut needs_mut = false;
            if let Some(FnArg::Receiver(receiver)) = sig.inputs.first() {
                needs_mut = receiver.mutability.is_some();
            }

            // 生成唯一const名
            let const_ident = syn::Ident::new(
                &format!("__DYNAMIC_METHOD_{}_{}", struct_type, method_name)
                    .replace("-", "_")
                    .to_uppercase(),
                method_name.span(),
            );

            // 解析参数（类似你的原代码，跳过self）
            let mut arg_downcasts = Vec::new();
            let mut call_args = Vec::new();
            let mut arg_index = 0usize;
            for arg in sig.inputs.iter().skip(1) {
                // 跳过self
                if let FnArg::Typed(pat_type) = arg {
                    let ty = &*pat_type.ty;
                    let temp_var =
                        syn::Ident::new(&format!("__arg_{}", arg_index), pat_type.span());
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
                        // 错误处理
                        return syn::Error::new_spanned(
                            &pat_type.pat,
                            "Only simple identifiers or _ supported",
                        )
                        .to_compile_error()
                        .into();
                    }
                    arg_index += 1;
                }
            }

            // 生成注册代码
            let registration = if needs_mut {
                quote! {
                    const #const_ident: () = {
                        use ::alanthinker_dynamic_get_field_trait::{MethodInfo, MethodKind};
                        use ::inventory;
                        inventory::submit! {
                            MethodInfo {
                                type_id: std::any::TypeId::of::<#struct_type>(),
                                name: stringify!(#method_name),
                                kind: MethodKind::Mutable {
                                    call: move |obj: &mut dyn ::std::any::Any, args: &[&dyn ::std::any::Any]| -> Option<Box<dyn ::std::any::Any>> {
                                        #(#arg_downcasts)*
                                        if let Some(this) = obj.downcast_mut::<#struct_type>() {
                                            let result = this.#method_name(#(#call_args),*);
                                            return Some(Box::new(result));
                                        }
                                        None
                                    }
                                }
                            }
                        };
                    };
                }
            } else {
                quote! {
                    const #const_ident: () = {
                        use ::alanthinker_dynamic_get_field_trait::{MethodInfo, MethodKind};
                        use ::inventory;
                        inventory::submit! {
                            MethodInfo {
                                type_id: std::any::TypeId::of::<#struct_type>(),
                                name: stringify!(#method_name),
                                kind: MethodKind::Immutable {
                                    call: move |obj: &dyn ::std::any::Any, args: &[&dyn ::std::any::Any]| -> Option<Box<dyn ::std::any::Any>> {
                                        #(#arg_downcasts)*
                                        if let Some(this) = obj.downcast_ref::<#struct_type>() {
                                            let result = this.#method_name(#(#call_args),*);
                                            return Some(Box::new(result));
                                        }
                                        None
                                    }
                                }
                            }
                        };
                    };
                }
            };

            // 将注册代码注入到方法后（作为const项）
            registrations.push(registration);
        }
    }

    // 生成扩展后的impl块：原impl + 注册代码
    let expanded = quote! {
        #impl_block
        #(#registrations)*
    };

    TokenStream::from(expanded)
}
