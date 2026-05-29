//! `rekaptr-rsx` — a tiny JSX-flavored macro for adabraka-gpui builder chains.
//!
//! Proof-of-concept. `rsx! { <div flex gap_2> ... </div> }` expands to the
//! equivalent `div().flex().gap_2()...` builder chain — so it works with
//! *any* gpui-shaped builder API, including the adabraka fork, because it
//! only emits method calls that resolve in the caller's scope.
//!
//! Syntax:
//!   - `<div>` — lowercase tag becomes `div()`.
//!   - `<VStack>` — uppercase tag becomes `VStack::new()`.
//!   - `<Icon(name)>` — parenthesized args go to the constructor: `Icon::new(name)`.
//!   - `flex` — a bare attribute is a flag method: `.flex()`.
//!   - `bg={expr}` — an attribute with a value: `.bg(expr)`. The braces may hold
//!     any token stream, e.g. multiple args (`on_click={a, b}`) or a closure.
//!   - `{expr}` child — becomes `.child(expr)`.
//!   - `{..iter}` child — becomes `.children(iter)`.
//!   - `<when {cond}> ... </when>` — becomes `.when(cond, |el| el.child(...))`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, Ident, Token,
};

enum Node {
    Element(Element),
    Block(Expr),
    Children(Expr),
    When { cond: Expr, children: Vec<Node> },
}

struct Element {
    tag: Ident,
    ctor_args: Option<TokenStream2>,
    attrs: Vec<Attr>,
    children: Vec<Node>,
}

struct Attr {
    name: Ident,
    value: Option<TokenStream2>,
}

impl Parse for Node {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![<]) {
            parse_element(input)
        } else if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            if content.peek(Token![..]) {
                content.parse::<Token![..]>()?;
                Ok(Node::Children(content.parse()?))
            } else {
                Ok(Node::Block(content.parse()?))
            }
        } else {
            Err(input.error("expected `<element>` or `{expr}`"))
        }
    }
}

fn parse_element(input: ParseStream) -> syn::Result<Node> {
    input.parse::<Token![<]>()?;
    let tag: Ident = input.parse()?;

    if tag == "when" {
        let content;
        braced!(content in input);
        let cond: Expr = content.parse()?;
        input.parse::<Token![>]>()?;
        let children = parse_children(input, "when")?;
        return Ok(Node::When { cond, children });
    }

    let ctor_args = if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        Some(content.parse()?)
    } else {
        None
    };

    let mut attrs = Vec::new();
    loop {
        if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            return Ok(Node::Element(Element {
                tag,
                ctor_args,
                attrs,
                children: Vec::new(),
            }));
        }
        if input.peek(Token![>]) {
            input.parse::<Token![>]>()?;
            break;
        }
        let name: Ident = input.parse()?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            let content;
            braced!(content in input);
            Some(content.parse()?)
        } else {
            None
        };
        attrs.push(Attr { name, value });
    }

    let children = parse_children(input, &tag.to_string())?;
    Ok(Node::Element(Element {
        tag,
        ctor_args,
        attrs,
        children,
    }))
}

fn parse_children(input: ParseStream, tag: &str) -> syn::Result<Vec<Node>> {
    let mut children = Vec::new();
    loop {
        if input.is_empty() {
            return Err(input.error(format!("unclosed `<{tag}>`")));
        }
        if input.peek(Token![<]) && input.peek2(Token![/]) {
            input.parse::<Token![<]>()?;
            input.parse::<Token![/]>()?;
            let close: Ident = input.parse()?;
            if close != tag {
                return Err(syn::Error::new(
                    close.span(),
                    format!("expected closing tag `</{tag}>`"),
                ));
            }
            input.parse::<Token![>]>()?;
            return Ok(children);
        }
        children.push(input.parse()?);
    }
}

impl Element {
    fn to_expr(&self) -> TokenStream2 {
        let tag = &self.tag;
        let starts_upper = tag
            .to_string()
            .chars()
            .next()
            .map(char::is_uppercase)
            .unwrap_or(false);
        let mut expr = match (&self.ctor_args, starts_upper) {
            (Some(args), true) => quote! { #tag::new(#args) },
            (Some(args), false) => quote! { #tag(#args) },
            (None, true) => quote! { #tag::new() },
            (None, false) => quote! { #tag() },
        };
        for attr in &self.attrs {
            let name = &attr.name;
            expr = match &attr.value {
                Some(v) => quote! { #expr.#name(#v) },
                None => quote! { #expr.#name() },
            };
        }
        for child in &self.children {
            expr = child.append_to(expr);
        }
        expr
    }
}

impl Node {
    fn append_to(&self, parent: TokenStream2) -> TokenStream2 {
        match self {
            Node::Element(e) => {
                let c = e.to_expr();
                quote! { #parent.child(#c) }
            }
            Node::Block(e) => quote! { #parent.child(#e) },
            Node::Children(e) => quote! { #parent.children(#e) },
            Node::When { cond, children } => {
                let mut body = quote! { __rsx_el };
                for child in children {
                    body = child.append_to(body);
                }
                quote! { #parent.when(#cond, |__rsx_el| { #body }) }
            }
        }
    }
}

#[proc_macro]
pub fn rsx(input: TokenStream) -> TokenStream {
    let node = parse_macro_input!(input as Node);
    match node {
        Node::Element(e) => e.to_expr().into(),
        Node::Block(e) => quote! { #e }.into(),
        Node::Children(_) | Node::When { .. } => syn::Error::new(
            proc_macro2::Span::call_site(),
            "`rsx!` root must be an element like `<div> ... </div>`",
        )
        .to_compile_error()
        .into(),
    }
}
