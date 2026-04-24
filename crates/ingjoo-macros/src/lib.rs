//! `#[derive(IngjooModel)]` — 从结构体定义自动生成 [`ModelDescriptor`] 构建链。
//!
//! # 结构体属性
//!
//! ```ignore
//! #[derive(IngjooModel)]
//! #[ingjoo(table = "articles", audit)]
//! struct Article { ... }
//! ```
//!
//! - `table = "..."` (**必填**) 数据库表名
//! - `audit` (可选) 自动包含审计字段
//!
//! # 字段属性
//!
//! ```ignore
//! #[ingjoo(type = "text", required)]
//! title: String,
//! ```
//!
//! - `type = "text|integer|float|boolean|timestamp|json|many2one"` (默认 `"text"`)
//! - `required` (可选)
//! - `unique` (可选)
//! - `related = "..."` (仅 many2one) 关联模型名

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, LitStr,
};

fn field_type_tokens(type_str: &str) -> Option<TokenStream2> {
    match type_str {
        "text" => Some(quote!(::ingjoo_core::module::registry::FieldType::Text)),
        "integer" => Some(quote!(::ingjoo_core::module::registry::FieldType::Integer)),
        "float" => Some(quote!(::ingjoo_core::module::registry::FieldType::Float)),
        "boolean" => Some(quote!(::ingjoo_core::module::registry::FieldType::Boolean)),
        "timestamp" => Some(quote!(::ingjoo_core::module::registry::FieldType::Timestamp)),
        "json" => Some(quote!(::ingjoo_core::module::registry::FieldType::Json)),
        _ => None,
    }
}

fn parse_struct_attrs(input: &DeriveInput) -> syn::Result<(String, bool)> {
    let mut table_name = String::new();
    let mut audit = false;

    for attr in &input.attrs {
        if !attr.path().is_ident("ingjoo") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("table") {
                let val: LitStr = meta.value()?.parse()?;
                table_name = val.value();
            } else if meta.path.is_ident("audit") {
                audit = true;
            } else {
                return Err(meta.error(format!(
                    "未知的结构体属性 `{}`，支持: table, audit",
                    meta.path.require_ident()?
                )));
            }
            Ok(())
        })?;
    }

    if table_name.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "#[ingjoo(table = \"...\")] 是必填属性",
        ));
    }

    Ok((table_name, audit))
}

struct FieldAttrs {
    field_type: String,
    required: bool,
    unique: bool,
    related: Option<String>,
}

fn parse_field_attrs(field: &syn::Field) -> syn::Result<FieldAttrs> {
    let mut attrs = FieldAttrs {
        field_type: String::from("text"),
        required: false,
        unique: false,
        related: None,
    };

    for attr in &field.attrs {
        if !attr.path().is_ident("ingjoo") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type") {
                let val: LitStr = meta.value()?.parse()?;
                attrs.field_type = val.value();
            } else if meta.path.is_ident("required") {
                attrs.required = true;
            } else if meta.path.is_ident("unique") {
                attrs.unique = true;
            } else if meta.path.is_ident("related") {
                let val: LitStr = meta.value()?.parse()?;
                attrs.related = Some(val.value());
            } else {
                return Err(meta.error(format!(
                    "未知的字段属性 `{}`，支持: type, required, unique, related",
                    meta.path.require_ident()?
                )));
            }
            Ok(())
        })?;
    }

    Ok(attrs)
}

fn build_field_call(name: &str, attrs: FieldAttrs, field: &syn::Field) -> syn::Result<TokenStream2> {
    if attrs.field_type == "many2one" {
        let related = attrs.related.unwrap_or_else(|| name.to_string());
        return Ok(quote! {
            .many2one(#name, #related)
        });
    }

    let ft = field_type_tokens(&attrs.field_type).ok_or_else(|| {
        syn::Error::new_spanned(
            field,
            format!(
                "未知的字段类型 `{}`，支持: text, integer, float, boolean, timestamp, json, many2one",
                attrs.field_type
            ),
        )
    })?;

    // 当前 builder 不支持 required+unique 同时设置，按优先级: required > unique > plain
    if attrs.required {
        Ok(quote! { .required_field(#name, #ft) })
    } else if attrs.unique {
        Ok(quote! { .unique_field(#name, #ft) })
    } else {
        Ok(quote! { .field(#name, #ft) })
    }
}

fn impl_ingjoo_model(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let struct_name = &input.ident;
    let (table_name, audit) = parse_struct_attrs(input)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "IngjooModel 只支持具名字段的结构体（Named Fields）",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "IngjooModel 只支持结构体",
            ))
        }
    };

    let mut field_calls = Vec::new();
    for field in fields {
        let field_ident = field.ident.as_ref().unwrap();
        let name = field_ident.to_string();
        let attrs = parse_field_attrs(field)?;
        let call = build_field_call(&name, attrs, field)?;
        field_calls.push(call);
    }

    let model_name = struct_name.to_string();
    let audit_call = if audit {
        quote! { .with_audit_fields() }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl #struct_name {
            /// 由 `#[derive(IngjooModel)]` 自动生成的模型描述符
            pub fn descriptor() -> ::ingjoo_core::module::registry::ModelDescriptor {
                ::ingjoo_core::module::registry::ModelDescriptor::new(#model_name, #table_name)
                    #audit_call
                    #(#field_calls)*
            }
        }
    })
}

#[proc_macro_derive(IngjooModel, attributes(ingjoo))]
pub fn derive_ingjoo_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_ingjoo_model(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
