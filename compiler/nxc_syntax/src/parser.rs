//! Hand-written recursive-descent parser (spec chapter 02).
//!
//! Error recovery: a syntax error inside a statement skips to the end of that
//! statement; inside an item, to the next item. Independent errors are still reported,
//! and a single mistake never produces a cascade.

use crate::ast::*;
use crate::diag::{DiagSink, Diagnostic};
use crate::source::{Edition, Span};
use crate::token::{StrPart, Token, TokenKind};

mod ed1;

pub fn parse_module(tokens: Vec<Token>, diags: &DiagSink) -> Module {
    parse_module_in(tokens, diags, Edition::V0_2)
}

/// Parses a module written in `edition` (ADR 0010). Both editions give the same AST.
pub fn parse_module_in(tokens: Vec<Token>, diags: &DiagSink, edition: Edition) -> Module {
    let mut p = Parser::new(tokens, diags);
    p.ed1 = edition == Edition::V1;
    p.module()
}

struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    diags: &'a DiagSink,
    /// Position of the last token at which an error was reported (suppresses cascades).
    last_error_at: Option<usize>,
    /// The file is written in edition 1 (ADR 0010).
    ed1: bool,
    /// Edition 1: a final `!` belongs to the enclosing construct (`when e! {`).
    no_bang: bool,
    /// Edition 1: names given to the functions of the tests.
    test_names: Vec<String>,
    /// The test being read expects a panic (`test "…" expects panic`).
    test_panics: bool,
    /// Nesting of expressions, blocks, types and patterns being read (E0113).
    depth: u32,
    /// E0113 was reported: the rest of the nesting is not reported again.
    too_deep: bool,
}

/// Deepest nesting of expressions, blocks, types and patterns (E0113). The analyses that
/// follow the parser are recursive: without a limit, a generated source of 100 000 nested
/// parentheses overflowed the stack of `nxc` (spec §7.4: the compiler never crashes).
pub const MAX_NESTING: u32 = 1000;
/// Most binary operators in a row in one expression (`a + b + …`): each one adds a level
/// to the tree that the analyses walk recursively.
pub const MAX_OPERATOR_CHAIN: u32 = 10_000;

type PResult<T> = Result<T, ()>;

const BINOPS: &[(TokenKind, BinOp, u8)] = &[
    (TokenKind::Or, BinOp::Or, 1),
    (TokenKind::And, BinOp::And, 2),
    (TokenKind::EqEq, BinOp::Eq, 3),
    (TokenKind::NotEq, BinOp::Ne, 3),
    (TokenKind::Lt, BinOp::Lt, 3),
    (TokenKind::Le, BinOp::Le, 3),
    (TokenKind::Gt, BinOp::Gt, 3),
    (TokenKind::Ge, BinOp::Ge, 3),
    (TokenKind::Pipe, BinOp::BitOr, 4),
    (TokenKind::Caret, BinOp::BitXor, 5),
    (TokenKind::Amp, BinOp::BitAnd, 6),
    (TokenKind::Shl, BinOp::Shl, 7),
    (TokenKind::Shr, BinOp::Shr, 7),
    (TokenKind::Plus, BinOp::Add, 8),
    (TokenKind::Minus, BinOp::Sub, 8),
    (TokenKind::PlusWrap, BinOp::AddWrap, 8),
    (TokenKind::MinusWrap, BinOp::SubWrap, 8),
    (TokenKind::Star, BinOp::Mul, 9),
    (TokenKind::Slash, BinOp::Div, 9),
    (TokenKind::Percent, BinOp::Rem, 9),
    (TokenKind::StarWrap, BinOp::MulWrap, 9),
];

fn assign_op(k: &TokenKind) -> Option<AssignOp> {
    use TokenKind::*;
    Some(match k {
        Eq => AssignOp::Assign,
        PlusEq => AssignOp::Compound(BinOp::Add),
        MinusEq => AssignOp::Compound(BinOp::Sub),
        StarEq => AssignOp::Compound(BinOp::Mul),
        SlashEq => AssignOp::Compound(BinOp::Div),
        PercentEq => AssignOp::Compound(BinOp::Rem),
        PlusWrapEq => AssignOp::Compound(BinOp::AddWrap),
        MinusWrapEq => AssignOp::Compound(BinOp::SubWrap),
        StarWrapEq => AssignOp::Compound(BinOp::MulWrap),
        AmpEq => AssignOp::Compound(BinOp::BitAnd),
        PipeEq => AssignOp::Compound(BinOp::BitOr),
        CaretEq => AssignOp::Compound(BinOp::BitXor),
        ShlEq => AssignOp::Compound(BinOp::Shl),
        ShrEq => AssignOp::Compound(BinOp::Shr),
        _ => return None,
    })
}

impl<'a> Parser<'a> {
    fn new(toks: Vec<Token>, diags: &'a DiagSink) -> Parser<'a> {
        Parser { toks, pos: 0, diags, last_error_at: None, ed1: false, no_bang: false, test_names: Vec::new(), test_panics: false, depth: 0, too_deep: false }
    }

    // -----------------------------------------------------------------------
    // Token helpers

    fn peek(&self) -> &TokenKind {
        &self.toks[self.pos.min(self.toks.len() - 1)].kind
    }

    fn peek_at(&self, n: usize) -> &TokenKind {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].kind
    }

    fn span(&self) -> Span {
        self.toks[self.pos.min(self.toks.len() - 1)].span
    }

    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.span()
        } else {
            self.toks[self.pos - 1].span
        }
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos.min(self.toks.len() - 1)].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn is(&self, k: &TokenKind) -> bool {
        self.peek() == k
    }

    fn eat(&mut self, k: &TokenKind) -> bool {
        if self.is(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn skip_newlines(&mut self) {
        while self.peek().is_terminator() {
            self.bump();
        }
    }

    /// Enters one level of nesting; past `MAX_NESTING`, reports E0113 once and fails.
    /// Each successful call is paired with `self.depth -= 1`.
    fn enter(&mut self) -> PResult<()> {
        if self.depth >= MAX_NESTING {
            self.too_deep(format!("more than {} levels of nesting", MAX_NESTING));
            return Err(());
        }
        self.depth += 1;
        Ok(())
    }

    fn too_deep(&mut self, what: String) {
        if !self.too_deep {
            self.too_deep = true;
            let span = self.span();
            self.diags.emit(
                Diagnostic::error("E0113", "this construction is too deeply nested or too long")
                    .primary(span, what)
                    .help("split it with intermediate variables or functions"),
            );
        }
    }

    fn error_here(&mut self, d: Diagnostic) {
        if self.last_error_at == Some(self.pos) {
            return;
        }
        self.last_error_at = Some(self.pos);
        self.diags.emit(d);
    }

    fn at_line_start(&self) -> bool {
        self.toks[self.pos.min(self.toks.len() - 1)].line_start
    }

    fn expected(&mut self, what: &str) {
        let found = self.peek().describe();
        let span = self.span();
        let mut d = Diagnostic::error("E0101", format!("expected {}, found {}", what, found)).primary(span, format!("expected {}", what));
        if matches!(self.peek(), TokenKind::Newline) || self.at_line_start() {
            // Point at the end of the previous token: that is where something is missing.
            let at = self.prev_span().end();
            d = Diagnostic::error("E0101", format!("expected {}, found end of line", what)).primary(at, format!("expected {} here", what));
        }
        self.error_here(d);
    }

    fn expect(&mut self, k: TokenKind, what: &str) -> PResult<Span> {
        if self.is(&k) {
            Ok(self.bump().span)
        } else {
            self.expected(what);
            Err(())
        }
    }

    fn ident(&mut self, what: &str) -> PResult<Ident> {
        match self.peek().clone() {
            TokenKind::Ident(name) if name != "_" => {
                let span = self.bump().span;
                if self.ed1 && ed1::WORDS.contains(&name.as_str()) {
                    self.error_here(
                        Diagnostic::error("E0010", format!("`{}` is a keyword in edition 1", name))
                            .primary(span, "reserved word")
                            .help(format!("choose another name, for example `{}_`", name)),
                    );
                }
                Ok(Ident { name, span })
            }
            _ => {
                self.expected(what);
                Err(())
            }
        }
    }

    /// Expects the end of a statement or item.
    fn expect_terminator(&mut self, closing: &TokenKind) -> PResult<()> {
        if self.peek().is_terminator() {
            self.skip_newlines();
            Ok(())
        } else if self.is(closing) || self.at_eof() {
            Ok(())
        } else {
            let found = self.peek().describe();
            let span = self.span();
            self.error_here(
                Diagnostic::error("E0101", format!("expected end of line, found {}", found))
                    .primary(span, "unexpected token")
                    .help("put each statement on its own line, or separate them with `;`"),
            );
            Err(())
        }
    }

    /// Skips to the end of the current statement without consuming a closing `}`.
    fn recover_stmt(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                TokenKind::Eof => return,
                TokenKind::Newline | TokenKind::Semi if depth <= 0 => {
                    self.skip_newlines();
                    return;
                }
                TokenKind::RBrace if depth <= 0 => return,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => depth -= 1,
                _ => {}
            }
            self.bump();
        }
    }

    /// Skips to the next token that can start an item at nesting depth 0.
    fn recover_item(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                TokenKind::Eof => return,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth -= 1;
                    if depth < 0 {
                        // Stray closing delimiter at top level: skip it.
                        depth = 0;
                    }
                }
                TokenKind::Newline if depth == 0 => {
                    self.skip_newlines();
                    if matches!(
                        self.peek(),
                        TokenKind::Fn
                            | TokenKind::Struct
                            | TokenKind::Enum
                            | TokenKind::Impl
                            | TokenKind::Trait
                            | TokenKind::Const
                            | TokenKind::Static
                            | TokenKind::Type
                            | TokenKind::Extern
                            | TokenKind::Pub
                            | TokenKind::At
                            | TokenKind::Import
                            | TokenKind::Unsafe
                            | TokenKind::Eof
                    ) {
                        return;
                    }
                    continue;
                }
                _ => {}
            }
            self.bump();
        }
    }

    // -----------------------------------------------------------------------
    // Module and items

    fn module(&mut self) -> Module {
        self.skip_newlines();
        let mut decl = None;
        if self.is(&TokenKind::Module) {
            let start = self.bump().span;
            if let Ok(path) = self.path_idents() {
                let span = start.to(self.prev_span());
                decl = Some((path, span));
            }
            if self.expect_terminator(&TokenKind::Eof).is_err() {
                self.recover_item();
            }
        }
        let mut imports = Vec::new();
        let mut items = Vec::new();
        while !self.at_eof() {
            if self.is(&TokenKind::Import) {
                if let Ok(imp) = self.import() {
                    if !items.is_empty() {
                        self.diags.emit(
                            Diagnostic::error("E0107", "imports must come before all other items")
                                .primary(imp.span, "this import comes after an item")
                                .help("move this import to the top of the file"),
                        );
                    }
                    imports.push(imp);
                }
                if self.expect_terminator(&TokenKind::Eof).is_err() {
                    self.recover_item();
                }
                continue;
            }
            if self.is(&TokenKind::Module) {
                let span = self.span();
                self.error_here(Diagnostic::error("E0101", "the `module` declaration must be the first line of the file").primary(span, "misplaced module declaration"));
                self.bump();
                self.recover_item();
                continue;
            }
            let parsed = if self.ed1 { self.items1() } else { self.item(false).map(|i| vec![i]) };
            match parsed {
                Ok(new_items) => {
                    items.extend(new_items);
                    if self.expect_terminator(&TokenKind::Eof).is_err() {
                        self.recover_item();
                    }
                }
                Err(()) => self.recover_item(),
            }
        }
        Module { decl, imports, items }
    }

    fn path_idents(&mut self) -> PResult<Vec<Ident>> {
        let mut v = vec![self.ident("a module name")?];
        while self.is(&TokenKind::Dot) && matches!(self.peek_at(1), TokenKind::Ident(_)) {
            self.bump();
            v.push(self.ident("a name")?);
        }
        Ok(v)
    }

    fn import(&mut self) -> PResult<Import> {
        let start = self.bump().span; // `import`
        let path = self.path_idents()?;
        let kind = if self.is(&TokenKind::Dot) && self.peek_at(1) == &TokenKind::LBrace {
            self.bump();
            self.bump();
            let mut list = Vec::new();
            loop {
                if self.eat(&TokenKind::RBrace) {
                    break;
                }
                let name = self.ident("a name to import")?;
                let alias = if self.eat(&TokenKind::As) { Some(self.ident("an alias")?) } else { None };
                list.push((name, alias));
                if !self.eat(&TokenKind::Comma) {
                    self.expect(TokenKind::RBrace, "`,` or `}`")?;
                    break;
                }
            }
            ImportKind::Items(list)
        } else if self.eat(&TokenKind::As) {
            ImportKind::Module { alias: Some(self.ident("an alias")?) }
        } else {
            ImportKind::Module { alias: None }
        };
        Ok(Import { path, kind, span: start.to(self.prev_span()) })
    }

    fn attrs(&mut self) -> PResult<Vec<Attr>> {
        let mut attrs = Vec::new();
        while self.is(&TokenKind::At) {
            let start = self.bump().span;
            let name = self.ident("an attribute name")?;
            let mut args = Vec::new();
            if self.eat(&TokenKind::LParen) {
                loop {
                    if self.eat(&TokenKind::RParen) {
                        break;
                    }
                    let sp = self.span();
                    let lit = match self.peek().clone() {
                        TokenKind::Str(s) if s.parts.len() == 1 => match &s.parts[0] {
                            StrPart::Text(t) => Lit::Str(t.clone()),
                            _ => {
                                self.expected("a literal");
                                return Err(());
                            }
                        },
                        TokenKind::Int(v) => Lit::Int(v),
                        // `@derive(Eq, Clone)`: names are given as text.
                        TokenKind::Ident(n) => Lit::Str(n),
                        TokenKind::True => Lit::Bool(true),
                        TokenKind::False => Lit::Bool(false),
                        _ => {
                            self.expected("a literal");
                            return Err(());
                        }
                    };
                    self.bump();
                    args.push((lit, sp));
                    if !self.eat(&TokenKind::Comma) {
                        self.expect(TokenKind::RParen, "`,` or `)`")?;
                        break;
                    }
                }
            }
            attrs.push(Attr { name, args, span: start.to(self.prev_span()) });
            self.skip_newlines();
        }
        Ok(attrs)
    }

    fn item(&mut self, in_impl: bool) -> PResult<Item> {
        let start = self.span();
        let attrs = self.attrs()?;
        let is_pub = self.eat(&TokenKind::Pub);
        let kind = match self.peek() {
            TokenKind::Fn | TokenKind::Unsafe => ItemKind::Fn(self.fn_decl()?),
            TokenKind::Struct if !in_impl => ItemKind::Struct(self.struct_decl()?),
            TokenKind::Enum if !in_impl => ItemKind::Enum(self.enum_decl()?),
            TokenKind::Impl if !in_impl => ItemKind::Impl(self.impl_decl()?),
            TokenKind::Trait if !in_impl => ItemKind::Trait(self.trait_decl()?),
            TokenKind::Const if !in_impl => ItemKind::Const(self.const_decl()?),
            TokenKind::Static if !in_impl => ItemKind::Static(self.static_decl()?),
            TokenKind::Type if !in_impl => ItemKind::TypeAlias(self.type_alias()?),
            TokenKind::Extern if !in_impl => ItemKind::ExternFn(self.extern_fn()?),
            _ => {
                let found = self.peek().describe();
                let span = self.span();
                let msg = if in_impl { "expected a function in `impl` block" } else { "expected an item" };
                let mut d = Diagnostic::error("E0108", format!("{}, found {}", msg, found)).primary(span, "unexpected here");
                if !in_impl && matches!(self.peek(), TokenKind::Let | TokenKind::Var) {
                    d = d.help("variables can only be declared inside functions; use `const` for a module-level constant");
                } else if !in_impl {
                    d = d.note("a module contains `fn`, `struct`, `enum`, `trait`, `impl`, `const`, `static`, `type` and `extern` declarations");
                }
                self.error_here(d);
                return Err(());
            }
        };
        Ok(Item { attrs, is_pub, kind, span: start.to(self.prev_span()) })
    }

    fn generics(&mut self) -> PResult<Vec<Ident>> {
        let mut v = Vec::new();
        if self.eat(&TokenKind::LBracket) {
            loop {
                if self.eat(&TokenKind::RBracket) {
                    break;
                }
                v.push(self.ident("a type parameter name")?);
                if !self.eat(&TokenKind::Comma) {
                    self.expect(TokenKind::RBracket, "`,` or `]`")?;
                    break;
                }
            }
        }
        Ok(v)
    }

    /// `[T, U: Trait + Other]`: names and inline bounds.
    fn generics_with_bounds(&mut self) -> PResult<(Vec<Ident>, Vec<BoundDecl>)> {
        let mut names = Vec::new();
        let mut bounds = Vec::new();
        if self.eat(&TokenKind::LBracket) {
            loop {
                if self.eat(&TokenKind::RBracket) {
                    break;
                }
                let name = self.ident("a type parameter name")?;
                if self.eat(&TokenKind::Colon) {
                    let traits = self.bound_list()?;
                    bounds.push(BoundDecl { param: name.clone(), traits });
                }
                names.push(name);
                if !self.eat(&TokenKind::Comma) {
                    self.expect(TokenKind::RBracket, "`,` or `]`")?;
                    break;
                }
            }
        }
        Ok((names, bounds))
    }

    /// `Trait + Other + module.Third`
    fn bound_list(&mut self) -> PResult<Vec<TypeExpr>> {
        let mut traits = vec![self.type_expr()?];
        while self.eat(&TokenKind::Plus) {
            traits.push(self.type_expr()?);
        }
        Ok(traits)
    }

    /// `where T: Trait, U: Other + Third`
    fn where_clause(&mut self, bounds: &mut Vec<BoundDecl>) -> PResult<()> {
        if !self.eat(&TokenKind::Where) {
            return Ok(());
        }
        loop {
            let param = self.ident("a type parameter name")?;
            self.expect(TokenKind::Colon, "`:` and the traits it implements")?;
            let traits = self.bound_list()?;
            bounds.push(BoundDecl { param, traits });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(())
    }

    fn convention(&mut self) -> Convention {
        if self.eat(&TokenKind::Inout) {
            Convention::Inout
        } else if self.eat(&TokenKind::Owned) {
            Convention::Owned
        } else {
            Convention::Borrow
        }
    }

    fn fn_decl(&mut self) -> PResult<FnDecl> {
        let start = self.span();
        let is_unsafe = self.eat(&TokenKind::Unsafe);
        self.expect(TokenKind::Fn, "`fn`")?;
        let name = self.ident("a function name")?;
        let (generics, mut bounds) = self.generics_with_bounds()?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            let pstart = self.span();
            let conv = self.convention();
            if self.is(&TokenKind::SelfValue) {
                self.bump();
                params.push(Param { conv, kind: ParamKind::SelfParam, span: pstart.to(self.prev_span()) });
            } else {
                let pname = self.ident("a parameter name")?;
                self.expect(TokenKind::Colon, "`:` and a type")?;
                if matches!(self.peek(), TokenKind::Inout | TokenKind::Owned) {
                    let kw = self.peek().to_string();
                    let span = self.span();
                    self.error_here(
                        Diagnostic::error("E0101", format!("the convention `{}` goes before the parameter name", kw))
                            .primary(span, "misplaced convention")
                            .help(format!("write `{} {}: Type`", kw, pname.name)),
                    );
                    return Err(());
                }
                let ty = self.type_expr()?;
                params.push(Param { conv, kind: ParamKind::Named { name: pname, ty }, span: pstart.to(self.prev_span()) });
            }
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        let ret = if self.eat(&TokenKind::Arrow) { Some(self.type_expr()?) } else { None };
        self.where_clause(&mut bounds)?;
        let body = if self.is(&TokenKind::LBrace) { Some(self.block()?) } else { None };
        Ok(FnDecl { name, generics, bounds, params, ret, body, is_unsafe, span: start.to(self.prev_span()) })
    }

    /// Parses `{ elem sep elem sep ... }` where sep is `,` or a newline.
    fn braced_list<T>(&mut self, what: &str, mut elem: impl FnMut(&mut Self) -> PResult<T>) -> PResult<Vec<T>> {
        let open = self.expect(TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut v = Vec::new();
        loop {
            if self.eat(&TokenKind::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unclosed(open);
                return Err(());
            }
            match elem(self) {
                Ok(x) => v.push(x),
                Err(()) => {
                    self.recover_stmt();
                    continue;
                }
            }
            if self.eat(&TokenKind::Comma) {
                self.skip_newlines();
            } else if self.peek().is_terminator() {
                self.skip_newlines();
                self.eat(&TokenKind::Comma);
                self.skip_newlines();
            } else if !self.is(&TokenKind::RBrace) {
                self.expected(&format!("`,`, a new line or `}}` after {}", what));
                self.recover_stmt();
            }
        }
        Ok(v)
    }

    fn unclosed(&mut self, open: Span) {
        let span = self.span();
        self.error_here(
            Diagnostic::error("E0101", "unexpected end of file")
                .primary(span, "expected `}`")
                .secondary(open, "unclosed delimiter"),
        );
    }

    fn struct_decl(&mut self) -> PResult<StructDecl> {
        self.bump(); // struct
        let name = self.ident("a struct name")?;
        let generics = self.generics()?;
        let fields = self.braced_list("a field", |p| {
            let start = p.span();
            let is_pub = p.eat(&TokenKind::Pub);
            let indirect = p.eat(&TokenKind::Indirect);
            let name = p.ident("a field name")?;
            p.expect(TokenKind::Colon, "`:` and a type")?;
            let ty = p.type_expr()?;
            Ok(FieldDecl { is_pub, indirect, name, ty, span: start.to(p.prev_span()) })
        })?;
        Ok(StructDecl { name, generics, fields })
    }

    fn enum_decl(&mut self) -> PResult<EnumDecl> {
        self.bump(); // enum
        let name = self.ident("an enum name")?;
        let generics = self.generics()?;
        let variants = self.braced_list("a variant", |p| {
            let start = p.span();
            let indirect = p.eat(&TokenKind::Indirect);
            let name = p.ident("a variant name")?;
            let mut fields = Vec::new();
            let mut indirect = indirect;
            if p.eat(&TokenKind::LParen) {
                loop {
                    if p.eat(&TokenKind::RParen) {
                        break;
                    }
                    if p.is(&TokenKind::Indirect) {
                        let span = p.bump().span;
                        p.diags.emit(
                            Diagnostic::error("E0106", "`indirect` applies to a whole variant")
                                .primary(span, "not allowed on a variant value")
                                .help(format!("write `indirect {}(...)`", name.name)),
                        );
                        indirect = true;
                    }
                    fields.push(p.type_expr()?);
                    if !p.eat(&TokenKind::Comma) {
                        p.expect(TokenKind::RParen, "`,` or `)`")?;
                        break;
                    }
                }
            }
            Ok(VariantDecl { indirect, name, fields, span: start.to(p.prev_span()) })
        })?;
        Ok(EnumDecl { name, generics, variants })
    }

    fn impl_decl(&mut self) -> PResult<ImplDecl> {
        self.bump(); // impl
        let first = self.type_expr()?;
        let (trait_, ty, bounds) = if self.eat(&TokenKind::For) {
            // `impl Trait for Type` or `impl Trait for Type[A: Bound, B]`
            let tstart = self.span();
            let mut segments = vec![self.ident("a type name")?];
            while self.eat(&TokenKind::Dot) {
                segments.push(self.ident("a type name")?);
            }
            let mut args = Vec::new();
            let mut bounds = Vec::new();
            if self.eat(&TokenKind::LBracket) {
                loop {
                    if self.eat(&TokenKind::RBracket) {
                        break;
                    }
                    let aspan = self.span();
                    let name = self.ident("a type parameter name")?;
                    if self.eat(&TokenKind::Colon) {
                        let traits = self.bound_list()?;
                        bounds.push(BoundDecl { param: name.clone(), traits });
                    }
                    args.push(TypeExpr { kind: TypeExprKind::Path { segments: vec![name], args: vec![] }, span: aspan.to(self.prev_span()) });
                    if !self.eat(&TokenKind::Comma) {
                        self.expect(TokenKind::RBracket, "`,` or `]`")?;
                        break;
                    }
                }
            }
            let ty = TypeExpr { kind: TypeExprKind::Path { segments, args }, span: tstart.to(self.prev_span()) };
            (Some(first), ty, bounds)
        } else {
            (None, first, Vec::new())
        };
        let open = self.expect(TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut items = Vec::new();
        loop {
            if self.eat(&TokenKind::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unclosed(open);
                return Err(());
            }
            match self.item(true) {
                Ok(it) => {
                    items.push(it);
                    if self.expect_terminator(&TokenKind::RBrace).is_err() {
                        self.recover_stmt();
                    }
                }
                Err(()) => self.recover_stmt(),
            }
        }
        Ok(ImplDecl { ty, trait_, bounds, items })
    }

    fn trait_decl(&mut self) -> PResult<TraitDecl> {
        self.bump(); // trait
        let name = self.ident("a trait name")?;
        let supertraits = if self.eat(&TokenKind::Colon) { self.bound_list()? } else { Vec::new() };
        let open = self.expect(TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut items = Vec::new();
        loop {
            if self.eat(&TokenKind::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unclosed(open);
                return Err(());
            }
            match self.item(true) {
                Ok(it) => {
                    items.push(it);
                    if self.expect_terminator(&TokenKind::RBrace).is_err() {
                        self.recover_stmt();
                    }
                }
                Err(()) => self.recover_stmt(),
            }
        }
        Ok(TraitDecl { name, supertraits, items })
    }

    fn const_decl(&mut self) -> PResult<ConstDecl> {
        self.bump(); // const
        let name = self.ident("a constant name")?;
        self.expect(TokenKind::Colon, "`:` and a type (constants must be annotated)")?;
        let ty = self.type_expr()?;
        self.expect(TokenKind::Eq, "`=`")?;
        let value = self.expr()?;
        Ok(ConstDecl { name, ty, value })
    }

    fn static_decl(&mut self) -> PResult<StaticDecl> {
        self.bump(); // static
        let mutable = self.eat(&TokenKind::Var);
        let name = self.ident("a static variable name")?;
        self.expect(TokenKind::Colon, "`:` and a type (static variables must be annotated)")?;
        let ty = self.type_expr()?;
        self.expect(TokenKind::Eq, "`=` and an initial value")?;
        let value = self.expr()?;
        Ok(StaticDecl { name, mutable, ty, value })
    }

    fn type_alias(&mut self) -> PResult<TypeAliasDecl> {
        self.bump(); // type
        let name = self.ident("a type name")?;
        self.expect(TokenKind::Eq, "`=`")?;
        let ty = self.type_expr()?;
        Ok(TypeAliasDecl { name, ty })
    }

    fn extern_fn(&mut self) -> PResult<ExternFnDecl> {
        self.bump(); // extern
        let abi_span = self.span();
        let abi = match self.peek().clone() {
            TokenKind::Str(s) => match s.parts.as_slice() {
                [StrPart::Text(t)] => {
                    self.bump();
                    t.clone()
                }
                _ => {
                    self.expected("an ABI string such as \"C\"");
                    return Err(());
                }
            },
            _ => {
                self.expected("an ABI string such as \"C\"");
                return Err(());
            }
        };
        self.expect(TokenKind::Fn, "`fn`")?;
        let name = self.ident("a function name")?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            let pname = self.ident("a parameter name")?;
            self.expect(TokenKind::Colon, "`:` and a type")?;
            let ty = self.type_expr()?;
            params.push((pname, ty));
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        let ret = if self.eat(&TokenKind::Arrow) { Some(self.type_expr()?) } else { None };
        if self.is(&TokenKind::LBrace) {
            let span = self.span();
            self.error_here(Diagnostic::error("E0101", "an `extern` function cannot have a body").primary(span, "unexpected body"));
            return Err(());
        }
        Ok(ExternFnDecl { abi, abi_span, name, params, ret })
    }

    // -----------------------------------------------------------------------
    // Types

    fn type_expr(&mut self) -> PResult<TypeExpr> {
        self.enter()?;
        let t = self.type_expr_inner();
        self.depth -= 1;
        t
    }

    fn type_expr_inner(&mut self) -> PResult<TypeExpr> {
        if self.ed1 {
            if let Some(t) = self.type_expr1()? {
                return Ok(t);
            }
        }
        let start = self.span();
        let kind = match self.peek().clone() {
            TokenKind::LParen => {
                self.bump();
                if self.eat(&TokenKind::RParen) {
                    TypeExprKind::Unit
                } else {
                    let first = self.type_expr()?;
                    if self.eat(&TokenKind::Comma) {
                        let mut elems = vec![first];
                        loop {
                            if self.eat(&TokenKind::RParen) {
                                break;
                            }
                            elems.push(self.type_expr()?);
                            if !self.eat(&TokenKind::Comma) {
                                self.expect(TokenKind::RParen, "`,` or `)`")?;
                                break;
                            }
                        }
                        TypeExprKind::Tuple(elems)
                    } else {
                        self.expect(TokenKind::RParen, "`)`")?;
                        return Ok(TypeExpr { kind: first.kind, span: start.to(self.prev_span()) });
                    }
                }
            }
            TokenKind::LBracket => {
                self.bump();
                let elem = self.type_expr()?;
                if !self.is(&TokenKind::Semi) {
                    let span = self.span();
                    self.error_here(
                        Diagnostic::error("E0106", "expected `;` and a length in array type")
                            .primary(span, "expected `;`")
                            .help("fixed-size arrays are written `[T; N]`; growable arrays are `Vec[T]`"),
                    );
                    return Err(());
                }
                self.bump();
                let len = self.expr()?;
                self.expect(TokenKind::RBracket, "`]`")?;
                TypeExprKind::Array(Box::new(elem), Box::new(len))
            }
            TokenKind::Fn => {
                self.bump();
                self.expect(TokenKind::LParen, "`(`")?;
                let mut params = Vec::new();
                loop {
                    if self.eat(&TokenKind::RParen) {
                        break;
                    }
                    let conv = self.convention();
                    params.push((conv, self.type_expr()?));
                    if !self.eat(&TokenKind::Comma) {
                        self.expect(TokenKind::RParen, "`,` or `)`")?;
                        break;
                    }
                }
                let ret = if self.eat(&TokenKind::Arrow) { Some(Box::new(self.type_expr()?)) } else { None };
                TypeExprKind::Fn(params, ret)
            }
            TokenKind::SelfType => {
                self.bump();
                TypeExprKind::SelfType
            }
            TokenKind::Dyn => {
                self.bump();
                let t = self.type_expr()?;
                TypeExprKind::Dyn(Box::new(t))
            }
            TokenKind::Ident(_) => {
                let mut segments = vec![self.ident("a type name")?];
                while self.is(&TokenKind::Dot) {
                    self.bump();
                    segments.push(self.ident("a type name")?);
                }
                let mut args = Vec::new();
                if self.eat(&TokenKind::LBracket) {
                    loop {
                        if self.eat(&TokenKind::RBracket) {
                            break;
                        }
                        args.push(self.type_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            self.expect(TokenKind::RBracket, "`,` or `]`")?;
                            break;
                        }
                    }
                } else if self.is(&TokenKind::Lt) {
                    let span = self.span();
                    self.error_here(
                        Diagnostic::error("E0106", "generic arguments are written with square brackets")
                            .primary(span, "unexpected `<`")
                            .help(format!("write `{}[...]`", segments.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join("."))),
                    );
                    return Err(());
                }
                TypeExprKind::Path { segments, args }
            }
            TokenKind::Lt => {
                let span = self.span();
                self.error_here(
                    Diagnostic::error("E0106", "expected a type, found `<`")
                        .primary(span, "unexpected `<`")
                        .help("generic arguments are written with square brackets: `Vec[i32]`"),
                );
                return Err(());
            }
            _ => {
                let found = self.peek().describe();
                let span = self.span();
                self.error_here(Diagnostic::error("E0106", format!("expected a type, found {}", found)).primary(span, "expected a type"));
                return Err(());
            }
        };
        Ok(TypeExpr { kind, span: start.to(self.prev_span()) })
    }

    // -----------------------------------------------------------------------
    // Blocks and statements

    fn block(&mut self) -> PResult<Block> {
        self.enter()?;
        let b = self.block_inner();
        self.depth -= 1;
        b
    }

    fn block_inner(&mut self) -> PResult<Block> {
        let open = self.expect(TokenKind::LBrace, "`{`")?;
        self.skip_newlines();
        let mut stmts = Vec::new();
        loop {
            if self.is(&TokenKind::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unclosed(open);
                return Err(());
            }
            match self.stmt() {
                Ok(s) => {
                    stmts.push(s);
                    if self.expect_terminator(&TokenKind::RBrace).is_err() {
                        self.recover_stmt();
                    }
                }
                Err(()) => self.recover_stmt(),
            }
        }
        let close = self.bump().span;
        // A statement that computes a value and drops it, before the end of the block, is
        // almost always a line cut in two: `let total = a` then `- b` (audit G0, §4.1).
        if let Some((_, before_last)) = stmts.split_last() {
            for s in before_last {
                if let StmtKind::Expr(e) = &s.kind {
                    if is_inert(e) {
                        self.diags.emit(
                            Diagnostic::error("E0110", "this expression has no effect: its value is lost")
                                .primary(e.span, "computed, then dropped")
                                .help("if this line continues the previous one, end the previous line with the operator (`a -` then `b`)")
                                .help("to compute a value and drop it on purpose, write `let _ = ...`"),
                        );
                    }
                }
            }
        }
        Ok(Block { stmts, span: open.to(close) })
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.span();
        if self.ed1 {
            if let Some(kind) = self.stmt1()? {
                return Ok(Stmt { kind, span: start.to(self.prev_span()) });
            }
        }
        let kind = match self.peek().clone() {
            TokenKind::Let => {
                self.bump();
                let pat = self.pattern()?;
                let ty = if self.eat(&TokenKind::Colon) { Some(self.type_expr()?) } else { None };
                if !self.is(&TokenKind::Eq) {
                    let span = self.prev_span().end();
                    self.error_here(
                        Diagnostic::error("E0101", "a `let` declaration needs a value")
                            .primary(span, "expected `=` and a value here")
                            .help("use `var name: Type` to declare a variable that is assigned later"),
                    );
                    return Err(());
                }
                self.bump();
                let init = self.expr()?;
                StmtKind::Let { pat, ty, init }
            }
            TokenKind::Var => {
                self.bump();
                let name = self.ident("a variable name")?;
                let ty = if self.eat(&TokenKind::Colon) { Some(self.type_expr()?) } else { None };
                let init = if self.eat(&TokenKind::Eq) { Some(self.expr()?) } else { None };
                StmtKind::Var { name, ty, init }
            }
            // A block statement opens a scope (useful to destroy values earlier).
            TokenKind::LBrace => {
                let b = self.block()?;
                let span = b.span;
                StmtKind::Expr(Expr { kind: ExprKind::Block(b), span })
            }
            TokenKind::While => self.while_stmt(None)?,
            TokenKind::For => self.for_stmt(None)?,
            TokenKind::Ident(_) if self.peek_at(1) == &TokenKind::Colon && matches!(self.peek_at(2), TokenKind::While | TokenKind::For) => {
                let label = self.ident("a label")?;
                self.bump(); // `:`
                if self.is(&TokenKind::While) {
                    self.while_stmt(Some(label))?
                } else {
                    self.for_stmt(Some(label))?
                }
            }
            _ => {
                let e = self.expr()?;
                if let Some(op) = assign_op(self.peek()) {
                    let op_span = self.bump().span;
                    let value = self.expr()?;
                    StmtKind::Assign { op, op_span, target: e, value }
                } else if self.ed1 {
                    StmtKind::Expr(self.fallback(e)?)
                } else {
                    StmtKind::Expr(e)
                }
            }
        };
        Ok(Stmt { kind, span: start.to(self.prev_span()) })
    }

    fn while_stmt(&mut self, label: Option<Ident>) -> PResult<StmtKind> {
        let start = self.bump().span; // while
        if self.eat(&TokenKind::Let) {
            // `while let P = e { body }` is `while true { if let P = e { body } else { break } }`.
            let pat = self.pattern()?;
            self.expect(TokenKind::Eq, "`=`")?;
            let mutable = self.eat(&TokenKind::Amp);
            let e = self.expr()?;
            let body = self.block()?;
            let span = start.to(body.span);
            let brk = Expr { kind: ExprKind::Break(None), span };
            let else_block = Block { stmts: vec![Stmt { kind: StmtKind::Expr(brk), span }], span };
            let if_let = Expr {
                kind: ExprKind::If { cond: Cond::Let(Box::new(pat), Box::new(e), mutable), then: body, else_: Some(Box::new(Expr { kind: ExprKind::Block(else_block), span })) },
                span,
            };
            let outer = Block { stmts: vec![Stmt { kind: StmtKind::Expr(if_let), span }], span };
            let cond = Expr { kind: ExprKind::Lit(Lit::Bool(true)), span: start };
            return Ok(StmtKind::While { label, cond, body: outer });
        }
        let cond = self.expr()?;
        let body = self.block()?;
        Ok(StmtKind::While { label, cond, body })
    }

    fn for_stmt(&mut self, label: Option<Ident>) -> PResult<StmtKind> {
        self.bump(); // for
        let pat = self.pattern()?;
        self.expect(TokenKind::In, "`in`")?;
        let mutable = self.eat(&TokenKind::Amp);
        let first = self.expr()?;
        let iter = if self.is(&TokenKind::DotDot) || self.is(&TokenKind::DotDotEq) {
            let inclusive = self.bump().kind == TokenKind::DotDotEq;
            let end = self.expr()?;
            if mutable {
                self.diags.emit(Diagnostic::error("E0412", "a range cannot be iterated with `&`").primary(first.span.to(end.span), "the numbers of a range are values"));
            }
            ForIter::Range { start: first, end, inclusive }
        } else {
            ForIter::Expr { expr: first, mutable }
        };
        let body = self.block()?;
        Ok(StmtKind::For { label, pat, iter, body })
    }

    // -----------------------------------------------------------------------
    // Expressions

    fn expr(&mut self) -> PResult<Expr> {
        self.enter()?;
        let e = self.expr_inner();
        self.depth -= 1;
        e
    }

    fn expr_inner(&mut self) -> PResult<Expr> {
        let mut e = self.binary(1)?;
        while self.ed1 && self.is(&TokenKind::PipeGt) {
            e = self.pipe1(e)?;
        }
        Ok(e)
    }


    fn binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.cast_expr()?;
        let mut operators = 0u32;
        loop {
            operators += 1;
            if operators > MAX_OPERATOR_CHAIN {
                self.too_deep(format!("more than {} operators in a row", MAX_OPERATOR_CHAIN));
                return Err(());
            }
            if self.ed1 && min_prec <= 3 {
                if self.is_word("is") {
                    lhs = self.is_expr(lhs)?;
                    continue;
                }
                if self.eat(&TokenKind::In) {
                    lhs = self.in_expr(lhs)?;
                    continue;
                }
                if matches!(self.peek(), TokenKind::EqEqEq | TokenKind::NotEqEq) {
                    let op_span = self.bump().span;
                    self.not_yet(op_span, "an identity comparison (`===`, `!==`)");
                    let rhs = self.binary(4)?;
                    lhs = Expr { kind: ExprKind::Error, span: lhs.span.to(rhs.span) };
                    continue;
                }
            }
            let found = BINOPS.iter().find(|(k, _, _)| k == self.peek());
            let (op, prec) = match found {
                Some((_, op, prec)) if *prec >= min_prec => (*op, *prec),
                _ => break,
            };
            let op_span = self.bump().span;
            let rhs = self.binary(prec + 1)?;
            let span = lhs.span.to(rhs.span);
            if op.is_comparison() {
                if let Some((_, next, _)) = BINOPS.iter().find(|(k, o, _)| k == self.peek() && o.is_comparison()) {
                    let next_span = self.span();
                    self.diags.emit(
                        Diagnostic::error("E0104", "comparison operators cannot be chained")
                            .primary(next_span, format!("second comparison `{}`", next.symbol()))
                            .secondary(op_span, "first comparison")
                            .help("split the comparison with `and`: `a < b and b < c`"),
                    );
                    // Consume the rest of the chain; the whole expression is erroneous.
                    while BINOPS.iter().any(|(k, o, _)| k == self.peek() && o.is_comparison()) {
                        self.bump();
                        let r = self.binary(prec + 1)?;
                        lhs = Expr { kind: ExprKind::Error, span: span.to(r.span) };
                    }
                    continue;
                }
            }
            lhs = Expr { kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), span };
        }
        Ok(lhs)
    }

    fn cast_expr(&mut self) -> PResult<Expr> {
        let mut e = self.unary()?;
        while self.eat(&TokenKind::As) {
            let ty = self.type_expr()?;
            let span = e.span.to(ty.span);
            e = Expr { kind: ExprKind::Cast(Box::new(e), ty), span };
        }
        Ok(e)
    }

    fn unary(&mut self) -> PResult<Expr> {
        // Only an operator (`-`, `not`, `~`, `move`) adds a level.
        if !matches!(self.peek(), TokenKind::Minus | TokenKind::Not | TokenKind::Tilde | TokenKind::Move) {
            return self.unary_inner();
        }
        self.enter()?;
        let e = self.unary_inner();
        self.depth -= 1;
        e
    }

    fn unary_inner(&mut self) -> PResult<Expr> {
        let start = self.span();
        let op = match self.peek() {
            TokenKind::Minus => Some(UnOp::Neg),
            TokenKind::Not => Some(UnOp::Not),
            TokenKind::Tilde => Some(UnOp::BitNot),
            TokenKind::Move => {
                self.bump();
                let inner = self.unary()?;
                let span = start.to(inner.span);
                return Ok(Expr { kind: ExprKind::Move(Box::new(inner)), span });
            }
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let inner = self.unary()?;
            let span = start.to(inner.span);
            return Ok(Expr { kind: ExprKind::Unary(op, Box::new(inner)), span });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            match self.peek().clone() {
                TokenKind::Dot => {
                    self.bump();
                    match self.peek().clone() {
                        TokenKind::Ident(name) if name != "_" => {
                            let span = self.bump().span;
                            let full = e.span.to(span);
                            e = Expr { kind: ExprKind::Field(Box::new(e), Ident { name, span }), span: full };
                        }
                        TokenKind::Int(i) => {
                            let span = self.bump().span;
                            let full = e.span.to(span);
                            e = Expr { kind: ExprKind::TupleField(Box::new(e), i.min(u32::MAX as u128) as u32, span), span: full };
                        }
                        _ => {
                            self.expected("a field or method name after `.`");
                            return Err(());
                        }
                    }
                }
                TokenKind::LParen => {
                    self.bump();
                    let args = self.call_args()?;
                    let span = e.span.to(self.prev_span());
                    e = Expr { kind: ExprKind::Call(Box::new(e), args), span };
                }
                TokenKind::LBracket => {
                    self.bump();
                    let mut args = Vec::new();
                    loop {
                        if self.eat(&TokenKind::RBracket) {
                            break;
                        }
                        if self.is(&TokenKind::Fn) {
                            let t = self.type_expr()?;
                            let span = t.span;
                            args.push(Expr { kind: ExprKind::Type(t), span });
                        } else {
                            args.push(self.expr()?);
                        }
                        if !self.eat(&TokenKind::Comma) {
                            self.expect(TokenKind::RBracket, "`]`")?;
                            break;
                        }
                    }
                    let span = e.span.to(self.prev_span());
                    e = Expr { kind: ExprKind::Index(Box::new(e), args), span };
                }
                TokenKind::Question => {
                    let q = self.bump().span;
                    let span = e.span.to(q);
                    e = Expr { kind: ExprKind::Try(Box::new(e)), span };
                }
                // Edition 1: `value!` marks a value modified here (an argument, the receiver of
                // a method); the analysis checks the conventions as for 0.2.
                TokenKind::Bang if self.ed1 => {
                    if self.no_bang && self.peek_at(1) == &TokenKind::LBrace {
                        break;
                    }
                    self.bump();
                    if !self.is(&TokenKind::Dot) {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn call_args(&mut self) -> PResult<Vec<Arg>> {
        let mut args = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            let start = self.span();
            let label = if matches!(self.peek(), TokenKind::Ident(_)) && self.peek_at(1) == &TokenKind::Colon {
                let l = self.ident("a label")?;
                self.bump();
                Some(l)
            } else {
                None
            };
            if self.ed1 && self.is(&TokenKind::Amp) {
                let span = self.span();
                self.removed(span, "&x", "`x!` after the argument");
                return Err(());
            }
            let mut inout = if self.is(&TokenKind::Amp) { Some(self.bump().span) } else { None };
            let expr = self.expr()?;
            if self.ed1 && self.toks[self.pos - 1].kind == TokenKind::Bang {
                inout = Some(self.prev_span());
            }
            args.push(Arg { label, inout, expr, span: start.to(self.prev_span()) });
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> PResult<Expr> {
        let start = self.span();
        let lit = |k: ExprKind, span: Span| Ok(Expr { kind: k, span });
        if self.ed1 {
            match self.peek().clone() {
                TokenKind::Ident(w) if w == "when" => return self.when1(),
                TokenKind::Ident(w) if w == "spawn" => {
                    self.not_yet(start, "a task (`spawn`)");
                    return Err(());
                }
                TokenKind::Ident(w) if w == "it" => {
                    self.not_yet(start, "`it`");
                    return Err(());
                }
                TokenKind::Match => {
                    self.removed(start, "match", "`when value { .Case then … }`");
                    return Err(());
                }
                TokenKind::Fn => return self.closure1(),
                // `.Name`: a variant of the type the context expects.
                TokenKind::Dot if matches!(self.peek_at(1), TokenKind::Ident(_)) => {
                    self.bump();
                    let name = self.ident("a variant name")?;
                    return lit(ExprKind::Variant(name), start.to(self.prev_span()));
                }
                TokenKind::Amp => {
                    self.removed(start, "&x", "`x!` after the argument");
                    return Err(());
                }
                TokenKind::Break | TokenKind::Continue => {
                    let is_break = self.is(&TokenKind::Break);
                    self.bump();
                    let label = if self.eat(&TokenKind::Hash) { Some(self.ident("a label")?) } else { None };
                    let span = start.to(self.prev_span());
                    return lit(if is_break { ExprKind::Break(label) } else { ExprKind::Continue(label) }, span);
                }
                _ => {}
            }
        }
        match self.peek().clone() {
            TokenKind::Int(v) => {
                self.bump();
                lit(ExprKind::Lit(Lit::Int(v)), start)
            }
            TokenKind::Float(v) => {
                self.bump();
                lit(ExprKind::Lit(Lit::Float(v)), start)
            }
            TokenKind::Char(c) => {
                self.bump();
                lit(ExprKind::Lit(Lit::Char(c)), start)
            }
            TokenKind::True => {
                self.bump();
                lit(ExprKind::Lit(Lit::Bool(true)), start)
            }
            TokenKind::False => {
                self.bump();
                lit(ExprKind::Lit(Lit::Bool(false)), start)
            }
            TokenKind::Str(s) => {
                self.bump();
                if let [StrPart::Text(t)] = s.parts.as_slice() {
                    return lit(ExprKind::Lit(Lit::Str(t.clone())), start);
                }
                let mut parts = Vec::new();
                for part in s.parts.iter() {
                    match part {
                        StrPart::Text(t) => parts.push(StrPartAst::Text(t.clone())),
                        StrPart::Interp { tokens, spec, span } => {
                            let mut sub = Parser::new(tokens.clone(), self.diags);
                            sub.ed1 = self.ed1;
                            let e = match sub.expr() {
                                Ok(e) => {
                                    if !sub.at_eof() {
                                        let sp = sub.span();
                                        sub.error_here(Diagnostic::error("E0101", "unexpected token in interpolation").primary(sp, "expected `}` or `:` before this"));
                                    }
                                    e
                                }
                                Err(()) => Expr { kind: ExprKind::Error, span: *span },
                            };
                            parts.push(StrPartAst::Interp { expr: Box::new(e), spec: spec.clone() });
                        }
                    }
                }
                lit(ExprKind::Interp(parts), start)
            }
            TokenKind::Ident(name) => {
                self.bump();
                if name == "_" {
                    self.diags.emit(
                        Diagnostic::error("E0105", "`_` cannot be used as a value")
                            .primary(start, "`_` is only allowed in patterns")
                            .help("to ignore a value, write `let _ = expression`"),
                    );
                    return lit(ExprKind::Error, start);
                }
                lit(ExprKind::Ident(name), start)
            }
            TokenKind::SelfValue => {
                self.bump();
                lit(ExprKind::SelfValue, start)
            }
            TokenKind::SelfType => {
                self.bump();
                lit(ExprKind::SelfType, start)
            }
            TokenKind::LParen => {
                self.bump();
                if self.eat(&TokenKind::RParen) {
                    return lit(ExprKind::Unit, start.to(self.prev_span()));
                }
                let first = self.expr()?;
                if self.eat(&TokenKind::Comma) {
                    let mut elems = vec![first];
                    loop {
                        if self.eat(&TokenKind::RParen) {
                            break;
                        }
                        elems.push(self.expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            self.expect(TokenKind::RParen, "`,` or `)`")?;
                            break;
                        }
                    }
                    return lit(ExprKind::Tuple(elems), start.to(self.prev_span()));
                }
                self.expect(TokenKind::RParen, "`)`")?;
                lit(ExprKind::Paren(Box::new(first)), start.to(self.prev_span()))
            }
            TokenKind::LBracket => {
                self.bump();
                if self.eat(&TokenKind::RBracket) {
                    return lit(ExprKind::Array(Vec::new()), start.to(self.prev_span()));
                }
                let first = self.expr()?;
                if self.eat(&TokenKind::Semi) {
                    let count = self.expr()?;
                    self.expect(TokenKind::RBracket, "`]`")?;
                    return lit(ExprKind::ArrayRepeat(Box::new(first), Box::new(count)), start.to(self.prev_span()));
                }
                let mut elems = vec![first];
                loop {
                    if self.eat(&TokenKind::RBracket) {
                        break;
                    }
                    if !self.eat(&TokenKind::Comma) {
                        self.expect(TokenKind::RBracket, "`,` or `]`")?;
                        break;
                    }
                    if self.eat(&TokenKind::RBracket) {
                        break;
                    }
                    elems.push(self.expr()?);
                }
                lit(ExprKind::Array(elems), start.to(self.prev_span()))
            }
            TokenKind::If => self.if_expr(),
            TokenKind::Match => self.match_expr(),
            TokenKind::Fn => self.closure_expr(),
            TokenKind::Return => {
                self.bump();
                let ends = matches!(
                    self.peek(),
                    TokenKind::Newline | TokenKind::Semi | TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket | TokenKind::Comma | TokenKind::Eof
                );
                let value = if ends { None } else { Some(Box::new(self.expr()?)) };
                lit(ExprKind::Return(value), start.to(self.prev_span()))
            }
            TokenKind::Break | TokenKind::Continue => {
                let is_break = self.is(&TokenKind::Break);
                self.bump();
                let label = if matches!(self.peek(), TokenKind::Ident(_)) { Some(self.ident("a label")?) } else { None };
                let span = start.to(self.prev_span());
                if is_break {
                    lit(ExprKind::Break(label), span)
                } else {
                    lit(ExprKind::Continue(label), span)
                }
            }
            TokenKind::Unsafe => {
                self.bump();
                let b = self.block()?;
                let span = start.to(b.span);
                lit(ExprKind::Unsafe(b), span)
            }
            TokenKind::LBrace => {
                self.error_here(
                    Diagnostic::error("E0102", "expected an expression, found `{`")
                        .primary(start, "a block is not an expression")
                        .note("blocks only follow `if`, `else`, `while`, `for`, `match`, `unsafe`, `fn` and `=>`")
                        .help("structures are built with parentheses: `Point(x: 1.0, y: 2.0)`"),
                );
                Err(())
            }
            TokenKind::Amp => {
                self.error_here(
                    Diagnostic::error("E0412", "`&` is only used to pass an `inout` argument")
                        .primary(start, "unexpected `&`")
                        .note("NEXUL has no reference values; parameters borrow their arguments automatically"),
                );
                Err(())
            }
            _ => {
                let found = self.peek().describe();
                let missing_at_eol = matches!(self.peek(), TokenKind::Newline) || self.at_line_start();
                let d = if missing_at_eol {
                    Diagnostic::error("E0102", "expected an expression, found end of line").primary(self.prev_span().end(), "expected an expression here")
                } else {
                    Diagnostic::error("E0102", format!("expected an expression, found {}", found)).primary(start, "expected an expression")
                };
                self.error_here(d);
                Err(())
            }
        }
    }

    /// `fn(a, b: T) -> R { ... }` or `fn(a) => expr`
    fn closure_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span; // fn
        self.expect(TokenKind::LParen, "`(` and the parameters of the anonymous function")?;
        let mut params = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            let conv = self.convention();
            let name = self.ident("a parameter name")?;
            let ty = if self.eat(&TokenKind::Colon) { Some(self.type_expr()?) } else { None };
            params.push(ClosureParam { conv, name, ty });
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        let ret = if self.eat(&TokenKind::Arrow) { Some(self.type_expr()?) } else { None };
        let body = if self.eat(&TokenKind::FatArrow) {
            ClosureBody::Expr(self.expr()?)
        } else if self.is(&TokenKind::LBrace) {
            ClosureBody::Block(self.block()?)
        } else {
            self.expected("`{` or `=>` and the body of the anonymous function");
            return Err(());
        };
        let span = start.to(self.prev_span());
        Ok(Expr { kind: ExprKind::Closure(Box::new(ClosureExpr { params, ret, body })), span })
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span; // if
        if self.ed1 && self.is(&TokenKind::Let) {
            let span = self.span();
            self.removed(span, "if let P = e", "`if e is P`");
            return Err(());
        }
        let cond = if self.eat(&TokenKind::Let) {
            let pat = self.pattern()?;
            self.expect(TokenKind::Eq, "`=`")?;
            let mutable = self.eat(&TokenKind::Amp);
            let e = self.expr()?;
            Cond::Let(Box::new(pat), Box::new(e), mutable)
        } else {
            let e = self.expr()?;
            match ed1::is_test(&e) {
                // `if e is P` binds the names of `P` in the block.
                Some((pat, x, mutable)) if self.ed1 => Cond::Let(Box::new(pat), Box::new(x), mutable),
                _ => Cond::Expr(Box::new(e)),
            }
        };
        let then = self.block()?;
        // Allow `}` and `else` on separate lines.
        if self.is(&TokenKind::Newline) && self.peek_at(1) == &TokenKind::Else {
            self.bump();
        }
        let else_ = if self.eat(&TokenKind::Else) {
            if self.is(&TokenKind::If) {
                Some(Box::new(self.if_expr()?))
            } else {
                let b = self.block()?;
                let span = b.span;
                Some(Box::new(Expr { kind: ExprKind::Block(b), span }))
            }
        } else {
            None
        };
        Ok(Expr { kind: ExprKind::If { cond, then, else_ }, span: start.to(self.prev_span()) })
    }

    fn match_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span; // match
        let mutable = self.eat(&TokenKind::Amp);
        let scrutinee = self.expr()?;
        let open = self.expect(TokenKind::LBrace, "`{` after the matched expression")?;
        self.skip_newlines();
        let mut arms = Vec::new();
        loop {
            if self.eat(&TokenKind::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unclosed(open);
                return Err(());
            }
            let arm = (|| -> PResult<Arm> {
                let astart = self.span();
                let pat = self.pattern()?;
                let guard = if self.eat(&TokenKind::If) { Some(self.expr()?) } else { None };
                self.expect(TokenKind::FatArrow, "`=>`")?;
                let body = if self.is(&TokenKind::LBrace) {
                    let b = self.block()?;
                    let span = b.span;
                    Expr { kind: ExprKind::Block(b), span }
                } else {
                    let e = self.expr()?;
                    // An arm may also be a single assignment: `Some(n) => total += n`.
                    if let Some(op) = assign_op(self.peek()) {
                        let op_span = self.bump().span;
                        let value = self.expr()?;
                        let span = e.span.to(value.span);
                        let stmt = Stmt { kind: StmtKind::Assign { op, op_span, target: e, value }, span };
                        Expr { kind: ExprKind::Block(Block { stmts: vec![stmt], span }), span }
                    } else {
                        e
                    }
                };
                Ok(Arm { pat, guard, body, span: astart.to(self.prev_span()) })
            })();
            match arm {
                Ok(a) => arms.push(a),
                Err(()) => {
                    self.recover_stmt();
                    continue;
                }
            }
            if self.eat(&TokenKind::Comma) {
                self.skip_newlines();
            } else if self.peek().is_terminator() {
                self.skip_newlines();
            } else if !self.is(&TokenKind::RBrace) {
                self.expected("`,`, a new line or `}` after a match arm");
                self.recover_stmt();
            }
        }
        Ok(Expr { kind: ExprKind::Match { scrutinee: Box::new(scrutinee), arms, mutable }, span: start.to(self.prev_span()) })
    }

    // -----------------------------------------------------------------------
    // Patterns

    /// The values of a constructor pattern, after `(`.
    fn pat_args(&mut self) -> PResult<Vec<PatArg>> {
        let mut args = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            if self.is(&TokenKind::DotDot) {
                args.push(PatArg::Rest(self.bump().span));
            } else if matches!(self.peek(), TokenKind::Ident(_)) && self.peek_at(1) == &TokenKind::Colon {
                let field = self.ident("a field name")?;
                self.bump();
                let p = self.pattern()?;
                args.push(PatArg::Named(field, p));
            } else {
                args.push(PatArg::Positional(self.pattern()?));
            }
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        Ok(args)
    }

    fn pattern(&mut self) -> PResult<Pattern> {
        self.enter()?;
        let p = self.pattern_inner();
        self.depth -= 1;
        p
    }

    fn pattern_inner(&mut self) -> PResult<Pattern> {
        let first = self.pattern_simple()?;
        if !self.is(&TokenKind::Pipe) {
            return Ok(first);
        }
        let mut alts = vec![first];
        while self.eat(&TokenKind::Pipe) {
            alts.push(self.pattern_simple()?);
        }
        let span = alts[0].span.to(alts.last().unwrap().span);
        Ok(Pattern { kind: PatKind::Or(alts), span })
    }

    fn pat_lit(&mut self) -> PResult<PatLit> {
        let neg = self.eat(&TokenKind::Minus);
        let l = match self.peek().clone() {
            TokenKind::Int(v) => {
                self.bump();
                let v = v as i128;
                PatLit::Int(if neg { -v } else { v })
            }
            TokenKind::Char(c) if !neg => {
                self.bump();
                PatLit::Char(c)
            }
            TokenKind::True if !neg => {
                self.bump();
                PatLit::Bool(true)
            }
            TokenKind::False if !neg => {
                self.bump();
                PatLit::Bool(false)
            }
            TokenKind::Str(s) if !neg => match s.parts.as_slice() {
                [StrPart::Text(t)] => {
                    self.bump();
                    PatLit::Str(t.clone())
                }
                _ => {
                    let span = self.span();
                    self.error_here(Diagnostic::error("E0109", "a pattern cannot contain an interpolated string").primary(span, "interpolation not allowed here"));
                    return Err(());
                }
            },
            TokenKind::Float(_) => {
                let span = self.span();
                self.error_here(
                    Diagnostic::error("E0109", "floating-point literals cannot be used in patterns")
                        .primary(span, "float in pattern")
                        .help("compare with a guard instead: `x if x == 1.5 => ...`"),
                );
                return Err(());
            }
            _ => {
                self.expected("a literal");
                return Err(());
            }
        };
        Ok(l)
    }

    fn pattern_simple(&mut self) -> PResult<Pattern> {
        let start = self.span();
        if self.ed1 && self.is(&TokenKind::Dot) {
            // `.Name` or `.Name(…)`: a variant of the type examined (never a new variable).
            self.bump();
            let name = self.ident("a variant name")?;
            let kind = if self.eat(&TokenKind::LParen) { PatKind::Ctor { path: vec![name], args: self.pat_args()? } } else { PatKind::Path(vec![name]) };
            return Ok(Pattern { kind, span: start.to(self.prev_span()) });
        }
        let kind = match self.peek().clone() {
            TokenKind::Ident(name) if name == "_" => {
                self.bump();
                PatKind::Wild
            }
            TokenKind::Ident(_) => {
                let mut path = vec![self.ident("a name")?];
                while self.is(&TokenKind::Dot) {
                    self.bump();
                    path.push(self.ident("a variant name")?);
                }
                if self.eat(&TokenKind::LParen) {
                    let args = self.pat_args()?;
                    PatKind::Ctor { path, args }
                } else if path.len() == 1 {
                    // Edition 1: `x!` binds a value modified in place (`when e! { … }`).
                    if self.ed1 && self.is(&TokenKind::Bang) && self.peek_at(1) != &TokenKind::In {
                        self.bump();
                    }
                    PatKind::Ident(path.pop().unwrap().name)
                } else {
                    PatKind::Path(path)
                }
            }
            TokenKind::LParen => {
                self.bump();
                if self.eat(&TokenKind::RParen) {
                    PatKind::Tuple(Vec::new())
                } else {
                    let first = self.pattern()?;
                    if self.eat(&TokenKind::Comma) {
                        let mut elems = vec![first];
                        loop {
                            if self.eat(&TokenKind::RParen) {
                                break;
                            }
                            elems.push(self.pattern()?);
                            if !self.eat(&TokenKind::Comma) {
                                self.expect(TokenKind::RParen, "`,` or `)`")?;
                                break;
                            }
                        }
                        PatKind::Tuple(elems)
                    } else {
                        self.expect(TokenKind::RParen, "`)`")?;
                        return Ok(Pattern { kind: first.kind, span: start.to(self.prev_span()) });
                    }
                }
            }
            TokenKind::Int(_) | TokenKind::Minus | TokenKind::Char(_) | TokenKind::True | TokenKind::False | TokenKind::Str(_) | TokenKind::Float(_) => {
                let lo = self.pat_lit()?;
                if self.is(&TokenKind::DotDot) || self.is(&TokenKind::DotDotEq) {
                    let inclusive = self.bump().kind == TokenKind::DotDotEq;
                    let hi = self.pat_lit()?;
                    PatKind::Range(lo, hi, inclusive)
                } else {
                    PatKind::Lit(lo)
                }
            }
            _ => {
                let found = self.peek().describe();
                let span = self.span();
                self.error_here(Diagnostic::error("E0109", format!("expected a pattern, found {}", found)).primary(span, "expected a pattern"));
                return Err(());
            }
        };
        Ok(Pattern { kind, span: start.to(self.prev_span()) })
    }
}

/// An expression without effect: no call, no `?`, no control flow, no block. Used as a
/// statement, it computes a value nobody reads.
fn is_inert(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Lit(_) | ExprKind::Interp(_) | ExprKind::Ident(_) | ExprKind::Variant(_) | ExprKind::SelfValue | ExprKind::SelfType | ExprKind::Unit => true,
        ExprKind::Paren(x) | ExprKind::Unary(_, x) | ExprKind::Cast(x, _) | ExprKind::Field(x, _) | ExprKind::TupleField(x, _, _) | ExprKind::Move(x) => is_inert(x),
        ExprKind::Binary(_, a, b) | ExprKind::ArrayRepeat(a, b) => is_inert(a) && is_inert(b),
        ExprKind::Tuple(xs) | ExprKind::Array(xs) => xs.iter().all(is_inert),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::source::SourceMap;

    fn parse(src: &str) -> (Module, Vec<&'static str>) {
        let mut sm = SourceMap::new();
        let f = sm.add("t.nx", src);
        let d = DiagSink::new();
        let toks = lex(src, f, &d);
        let m = parse_module(toks, &d);
        (m, d.take_sorted().into_iter().map(|d| d.code).collect())
    }

    fn ok(src: &str) -> Module {
        let (m, errs) = parse(src);
        assert!(errs.is_empty(), "unexpected errors {:?} in:\n{}", errs, src);
        m
    }

    #[test]
    fn parses_items() {
        let m = ok(r#"
module app.main
import std.math
import std.io.{read_line, read_file as lire}

pub struct Point { x: f64, y: f64 }
struct Paire[A, B] {
    pub premier: A
    indirect second: B
}
enum Forme {
    Cercle(f64)
    Rect(f64, f64),
    Vide
}
impl Point {
    pub fn norme(self) -> f64 { return self.x }
    fn bouger(inout self, dx: f64, owned nom: String, inout autre: Point) { self.x += dx }
}
const MAX: i32 = 10
type Metres = f64
@link("m")
extern "C" fn cbrt(x: f64) -> f64
@intrinsic
pub fn size_of[T]() -> usize
"#);
        assert_eq!(m.imports.len(), 2);
        assert_eq!(m.items.len(), 8);
        assert!(m.decl.is_some());
    }

    #[test]
    fn parses_statements_and_expressions() {
        ok(r#"
fn main() {
    var total = 0
    for i in 0..10 {
        total += i
    }
    let s = if total > 3 { "grand" } else { "petit" }
    let a = match s {
        "grand" | "big" => 1
        _ => 2,
    }
    outer: while true {
        if a == 1 { break outer }
        else { continue }
    }
    let p = Point(x: 1.0, y: 2.0).norme()
    swap(&a, &b)
    let v = Vec[i32].new()
    let t = (1, 2).0
    let arr = [0; 4]
    let r = x?
    let m = move v
    let c = a & mask == 0 or not b
    let z = -x as u32
    if let Some(y) = opt {
        println("{y:>4} et {a + 1}")
    }
    let w = valeur
        .methode()
}
"#);
    }

    #[test]
    fn error_codes() {
        assert_eq!(parse("fn f() { let x = a < b < c }").1, vec!["E0104"]);
        assert_eq!(parse("fn f() { let x = }").1, vec!["E0102"]);
        // A value computed and dropped before the end of a block: a line cut in two.
        assert_eq!(parse("fn f() {\n let t = a\n - b\n g()\n}").1, vec!["E0110"]);
        assert_eq!(parse("fn f() {\n not ok\n (a + 1)\n g()\n}").1, vec!["E0110", "E0110"]);
        assert!(parse("fn f() {\n g()\n let _ = a - b\n x = 1\n a - b\n}").1.is_empty());
        assert_eq!(parse("let x = 1").1, vec!["E0108"]);
        assert_eq!(parse("fn f() { x = { 1 } }").1, vec!["E0102"]);
        assert_eq!(parse("fn f() -> Vec<i32> { }").1, vec!["E0106"]);
        // One error per broken statement, and parsing continues afterwards.
        assert_eq!(parse("fn f() {\n let = 1\n let y = )\n}\nfn g() { let x = 1 2 }").1, vec!["E0109", "E0102", "E0101"]);
    }

    #[test]
    fn nesting_and_operator_chains_are_limited() {
        // The parser is recursive: like `nxc`, run it on a large stack.
        std::thread::Builder::new().stack_size(256 << 20).spawn(nesting_and_operator_chains).unwrap().join().unwrap();
    }

    fn nesting_and_operator_chains() {
        let nested = |n: usize| format!("fn f() {{\n let x = {}1{}\n}}\n", "(".repeat(n), ")".repeat(n));
        // At the limit: accepted (the `let` and the parentheses count as levels).
        assert!(parse(&nested(MAX_NESTING as usize - 10)).1.is_empty());
        // Beyond it, and far beyond it (100 000 levels overflowed the stack): one E0113.
        assert_eq!(parse(&nested(MAX_NESTING as usize + 1)).1, vec!["E0113"]);
        assert_eq!(parse(&nested(100_000)).1, vec!["E0113"]);
        assert_eq!(parse(&format!("fn f() {}\n", "{".repeat(100_000))).1.first(), Some(&"E0113"));
        let chain = |n: usize| format!("fn f() {{\n let x = 1{}\n}}\n", " + 1".repeat(n));
        assert!(parse(&chain(MAX_OPERATOR_CHAIN as usize - 1)).1.is_empty());
        assert_eq!(parse(&chain(MAX_OPERATOR_CHAIN as usize + 1)).1, vec!["E0113"]);
    }
}
