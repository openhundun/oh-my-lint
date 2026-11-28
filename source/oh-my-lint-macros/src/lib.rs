use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::parse::{Parse, Parser};
use syn::spanned::Spanned;
use syn::{Attribute, Error, Expr, Fields, Ident, ItemEnum, LitStr, Result, Token, Type, Variant, parenthesized, parse_macro_input};

/// Generates ordered selection and a rule catalog from an enum of clause rules.
#[proc_macro_attribute]
pub fn clause_rules(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemEnum);
    ClauseArgs::parse(args, item.ident.span())
        .and_then(|args| args.expand(item))
        .unwrap_or_else(|error| error.into_compile_error())
        .into()
}

struct ClauseArgs {
    binding: Ident,
    input: Type,
    output: Type,
    applies_to: LitStr,
}

impl ClauseArgs {
    fn parse(args: TokenStream, span: Span) -> Result<Self> {
        let mut input = None;
        let mut output = None;
        let mut applies_to = None;
        syn::meta::parser(|meta| {
            if meta.path.is_ident("input") {
                set_once(&mut input, &meta, || {
                    let content;
                    parenthesized!(content in meta.input);
                    let binding = content.parse::<Ident>()?;
                    content.parse::<Token![:]>()?;
                    let input = content.parse::<Type>()?;
                    if !content.is_empty() {
                        return Err(content.error("expected a single input binding"));
                    }
                    Ok((binding, input))
                })
            } else if meta.path.is_ident("output") {
                set_value(&mut output, &meta)
            } else if meta.path.is_ident("applies_to") {
                set_value(&mut applies_to, &meta)
            } else {
                Err(meta.error("unknown clause_rules field"))
            }
        })
        .parse(args)?;

        let (binding, input) = required(input, span, "input")?;
        Ok(Self {
            binding,
            input,
            output: required(output, span, "output")?,
            applies_to: required(applies_to, span, "applies_to")?,
        })
    }

    fn expand(self, mut item: ItemEnum) -> Result<TokenStream2> {
        if let Some(lt) = &item.generics.lt_token {
            return Err(Error::new(lt.span(), "clause_rules does not support generics"));
        }
        if let Some(where_clause) = &item.generics.where_clause {
            return Err(Error::new_spanned(where_clause, "clause_rules does not support a where clause"));
        }
        let variant_count = item.variants.len();
        if variant_count == 0 {
            return Err(Error::new(item.ident.span(), "clause_rules requires at least an `otherwise` variant"));
        }

        let mut code_set = HashSet::with_capacity(variant_count);
        let mut rule_list = Vec::with_capacity(variant_count);
        for (index, variant) in item.variants.iter_mut().enumerate() {
            let rule = Rule::from_variant(variant, index == variant_count - 1)?;
            let code = rule.code.value();
            if !is_kebab_id(&code) {
                return Err(Error::new(rule.code.span(), "rule code must be a lowercase ASCII kebab-case ID"));
            }
            if !code_set.insert(code) {
                return Err(Error::new(rule.code.span(), "duplicate rule code"));
            }
            rule_list.push(rule);
        }

        let Self { binding, input, output, applies_to } = self;
        let name = &item.ident;
        let variant_list = item.variants.iter().map(|variant| &variant.ident);
        let selection_list = variant_list.clone().zip(&rule_list).map(|(variant, rule)| rule.selection(variant));
        let mode_arm_list = variant_list.clone().zip(&rule_list).map(|(variant, rule)| {
            let output = &rule.output;
            quote!(Self::#variant => #output,)
        });
        let code_arm_list = variant_list.enumerate().map(|(index, variant)| quote!(Self::#variant => const { Self::DOCS[#index].code },));
        let doc_list = rule_list.iter().map(|rule| rule.doc(&applies_to));

        Ok(quote! {
            #item

            impl #name {
                fn select(#binding: #input) -> Self {
                    #(#selection_list)*
                }

                pub(crate) const fn mode(self) -> #output {
                    match self {
                        #(#mode_arm_list)*
                    }
                }

                pub(crate) const fn code(self) -> &'static str {
                    match self {
                        #(#code_arm_list)*
                    }
                }

                pub(crate) const DOCS: &'static [crate::model::RuleDoc] = &[
                    #(#doc_list,)*
                ];
            }
        })
    }
}

struct Rule {
    code: LitStr,
    when: Option<Expr>,
    output: Expr,
    description: LitStr,
    trigger: LitStr,
    layout: LitStr,
    rationale: LitStr,
    before: LitStr,
    after: LitStr,
}

impl Rule {
    fn from_variant(variant: &mut Variant, is_final: bool) -> Result<Self> {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new_spanned(&variant.fields, "clause_rules variants must be unit variants"));
        }
        let mut rule_attr_list = variant.attrs.iter().enumerate().filter(|(_, attr)| attr.path().is_ident("rule"));
        let (index, _) = rule_attr_list.next().ok_or_else(|| Error::new(variant.ident.span(), "missing #[rule(...)] attribute"))?;
        if let Some((_, duplicate)) = rule_attr_list.next() {
            return Err(Error::new_spanned(duplicate, "duplicate #[rule(...)] attribute"));
        }
        Self::parse(&variant.attrs.remove(index), is_final)
    }

    fn parse(attr: &Attribute, is_final: bool) -> Result<Self> {
        let (mut code, mut when, mut otherwise, mut output) = (None, None, None, None);
        let (mut description, mut trigger, mut layout, mut rationale, mut before, mut after) = (None, None, None, None, None, None);
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("code") {
                set_value(&mut code, &meta)
            } else if meta.path.is_ident("when") {
                set_value(&mut when, &meta)
            } else if meta.path.is_ident("otherwise") {
                set_once(&mut otherwise, &meta, || {
                    if !meta.input.is_empty() && !meta.input.peek(Token![,]) {
                        return Err(meta.error("`otherwise` is a flag and takes no value"));
                    }
                    Ok(meta.path.span())
                })
            } else if meta.path.is_ident("output") {
                set_value(&mut output, &meta)
            } else if meta.path.is_ident("description") {
                set_value(&mut description, &meta)
            } else if meta.path.is_ident("trigger") {
                set_value(&mut trigger, &meta)
            } else if meta.path.is_ident("layout") {
                set_value(&mut layout, &meta)
            } else if meta.path.is_ident("rationale") {
                set_value(&mut rationale, &meta)
            } else if meta.path.is_ident("before") {
                set_value(&mut before, &meta)
            } else if meta.path.is_ident("after") {
                set_value(&mut after, &meta)
            } else {
                Err(meta.error("unknown rule field"))
            }
        })?;

        let span = attr.span();
        match (when.is_some(), otherwise) {
            (true, Some(span)) => return Err(Error::new(span, "`otherwise` cannot be combined with `when`")),
            (_, Some(span)) if !is_final => return Err(Error::new(span, "only the final rule may have `otherwise`")),
            (_, None) if is_final => return Err(Error::new(span, "the final rule must have `otherwise`")),
            (false, None) => return Err(Error::new(span, "missing `when` field")),
            _ => {}
        }
        Ok(Self {
            code: required(code, span, "code")?,
            when,
            output: required(output, span, "output")?,
            description: required(description, span, "description")?,
            trigger: required(trigger, span, "trigger")?,
            layout: required(layout, span, "layout")?,
            rationale: required(rationale, span, "rationale")?,
            before: required(before, span, "before")?,
            after: required(after, span, "after")?,
        })
    }

    fn selection(&self, variant: &Ident) -> TokenStream2 {
        if let Some(when) = &self.when {
            quote!(if #when { return Self::#variant; })
        } else {
            quote!(Self::#variant)
        }
    }

    fn doc(&self, applies_to: &LitStr) -> TokenStream2 {
        let Self { code, description, trigger, layout, rationale, before, after, .. } = self;
        quote! {
            crate::model::RuleDoc {
                code: #code,
                description: #description,
                applies_to: #applies_to,
                when: #trigger,
                layout: #layout,
                rationale: #rationale,
                before: #before,
                after: #after,
            }
        }
    }
}

fn set_once<T>(slot: &mut Option<T>, meta: &ParseNestedMeta<'_>, parse: impl FnOnce() -> Result<T>) -> Result<()> {
    if slot.is_some() {
        return Err(meta.error(format!("duplicate `{}` field", meta.path.require_ident()?)));
    }
    *slot = Some(parse()?);
    Ok(())
}

fn set_value<T: Parse>(slot: &mut Option<T>, meta: &ParseNestedMeta<'_>) -> Result<()> {
    set_once(slot, meta, || meta.value()?.parse())
}

fn required<T>(value: Option<T>, span: Span, field: &str) -> Result<T> {
    value.ok_or_else(|| Error::new(span, format!("missing `{field}` field")))
}

fn is_kebab_id(code: &str) -> bool {
    code.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && code
            .split('-')
            .all(|word| !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()))
}
