//! Edition 1 of the syntax (ADR 0010): `type`, the type before the name, `fn T f()`,
//! `when … then`, `is`, `x!`, `.Name`, `|>`, `test "…"`. It builds the same AST as edition
//! 0.2, so the analysis and the code generation do not change.
//!
//! The words of edition 1 that 0.2 did not have (`when`, `then`, `is`…) are lexed as
//! identifiers and recognized here; the 0.2 words that edition 1 removed (`struct`, `match`,
//! `&x`…) are refused with the new form in the message (E0112). Forms whose meaning is not
//! implemented yet (contracts, effects, `else` fallback…) are refused (E0111) rather than
//! accepted without being checked.

use super::*;

/// Words reserved in edition 1 (lexed as identifiers).
pub(super) const WORDS: &[&str] = &[
    "when", "then", "is", "interface", "extend", "any", "distinct", "it", "repeat", "test", "expects", "requires", "ensures", "with", "every",
    "within", "spawn", "loop",
];

impl<'a> Parser<'a> {
    // -----------------------------------------------------------------------
    // Helpers

    fn kind_at(&self, i: usize) -> &TokenKind {
        &self.toks[i.min(self.toks.len() - 1)].kind
    }

    /// The next token is the word `w` of edition 1.
    pub(super) fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), TokenKind::Ident(n) if n == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.is_word(w) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Index just after the matching closing delimiter of the one at `i`.
    fn skip_balanced(&self, mut i: usize) -> Option<usize> {
        let mut depth = 0i32;
        loop {
            match self.kind_at(i) {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                TokenKind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
    }

    /// If a type starts at token `i`, the index just after it (looking only at tokens).
    fn scan_type(&self, i: usize) -> Option<usize> {
        match self.kind_at(i) {
            TokenKind::Ident(n) if n == "any" && matches!(self.kind_at(i + 1), TokenKind::Ident(_)) => self.scan_type(i + 1),
            TokenKind::Ident(n) if n != "_" => {
                let mut j = i + 1;
                while self.kind_at(j) == &TokenKind::Dot && matches!(self.kind_at(j + 1), TokenKind::Ident(_)) {
                    j += 2;
                }
                if self.kind_at(j) == &TokenKind::LBracket {
                    j = self.skip_balanced(j)?;
                }
                Some(j)
            }
            TokenKind::SelfType => Some(i + 1),
            TokenKind::LParen | TokenKind::LBracket => self.skip_balanced(i),
            TokenKind::Fn => {
                let mut j = i + 1;
                if self.kind_at(j) != &TokenKind::LParen {
                    j = self.scan_type(j)?;
                }
                if self.kind_at(j) != &TokenKind::LParen {
                    return None;
                }
                self.skip_balanced(j)
            }
            _ => None,
        }
    }

    /// At the current position: a type, then a name (`i32 x`, `Vec[T] v`)?
    pub(super) fn typed_name_here(&self) -> bool {
        self.typed_name_at(self.pos)
    }

    fn typed_name_at(&self, i: usize) -> bool {
        match self.scan_type(i) {
            Some(j) => matches!(self.kind_at(j), TokenKind::Ident(n) if n != "is" && n != "then"),
            None => false,
        }
    }

    pub(super) fn not_yet(&mut self, span: Span, what: &str) {
        self.diags.emit(
            Diagnostic::error("E0111", format!("{} is not supported yet", what))
                .primary(span, "planned in edition 1 (ADR 0010), not implemented")
                .note("the compiler refuses it rather than accept it without checking it"),
        );
    }

    pub(super) fn removed(&mut self, span: Span, old: &str, new: &str) {
        self.error_here(Diagnostic::error("E0112", format!("`{}` does not exist in edition 1", old)).primary(span, "written differently in edition 1").help(format!("write {}", new)));
    }

    // -----------------------------------------------------------------------
    // Types

    /// `any I`, `fn R(A, B)`: the forms of types that differ from 0.2. `None`: not one of them.
    pub(super) fn type_expr1(&mut self) -> PResult<Option<TypeExpr>> {
        let start = self.span();
        if self.is_word("any") && matches!(self.peek_at(1), TokenKind::Ident(_)) {
            self.bump();
            let t = self.type_expr()?;
            return Ok(Some(TypeExpr { kind: TypeExprKind::Dyn(Box::new(t)), span: start.to(self.prev_span()) }));
        }
        match self.peek() {
            TokenKind::Dyn => {
                self.removed(start, "dyn", "`any Interface`");
                Err(())
            }
            TokenKind::Fn => {
                self.bump();
                let ret = if self.is(&TokenKind::LParen) { None } else { Some(Box::new(self.type_expr()?)) };
                self.expect(TokenKind::LParen, "`(` and the types of the parameters")?;
                let mut params = Vec::new();
                loop {
                    if self.eat(&TokenKind::RParen) {
                        break;
                    }
                    let owned = self.eat(&TokenKind::Owned);
                    let t = self.type_expr()?;
                    let conv = if self.eat(&TokenKind::Bang) {
                        Convention::Inout
                    } else if owned {
                        Convention::Owned
                    } else {
                        Convention::Borrow
                    };
                    params.push((conv, t));
                    if !self.eat(&TokenKind::Comma) {
                        self.expect(TokenKind::RParen, "`,` or `)`")?;
                        break;
                    }
                }
                Ok(Some(TypeExpr { kind: TypeExprKind::Fn(params, ret), span: start.to(self.prev_span()) }))
            }
            _ => Ok(None),
        }
    }

    // -----------------------------------------------------------------------
    // Items

    /// One declaration of edition 1; `type` gives several items (the type and its functions).
    pub(super) fn items1(&mut self) -> PResult<Vec<Item>> {
        let start = self.span();
        let mut attrs = self.attrs()?;
        let is_pub = self.eat(&TokenKind::Pub);
        let kind = match self.peek().clone() {
            // `unsafe extend T is Transfer`: a declaration the compiler cannot check (ADR 0013).
            TokenKind::Unsafe if matches!(self.peek_at(1), TokenKind::Ident(w) if w == "extend") => {
                self.bump();
                let mut decl = self.extend1()?;
                decl.is_unsafe = true;
                ItemKind::Impl(decl)
            }
            TokenKind::Fn | TokenKind::Unsafe => ItemKind::Fn(self.fn_decl1()?),
            TokenKind::Type => return self.type_decl1(attrs, is_pub, start),
            TokenKind::Const => {
                self.bump();
                let ty = self.type_expr()?;
                let name = self.ident("a constant name")?;
                self.expect(TokenKind::Eq, "`=`")?;
                let value = self.expr()?;
                ItemKind::Const(ConstDecl { name, ty, value })
            }
            TokenKind::Static => {
                self.bump();
                let mutable = self.eat(&TokenKind::Var);
                let ty = self.type_expr()?;
                let name = self.ident("a static variable name")?;
                self.expect(TokenKind::Eq, "`=` and an initial value")?;
                let value = self.expr()?;
                ItemKind::Static(StaticDecl { name, mutable, ty, value })
            }
            TokenKind::Extern => ItemKind::ExternFn(self.extern_fn1()?),
            TokenKind::Ident(w) if w == "interface" => ItemKind::Trait(self.interface1()?),
            TokenKind::Ident(w) if w == "extend" => ItemKind::Impl(self.extend1()?),
            TokenKind::Ident(w) if w == "test" => {
                let tspan = self.span();
                let f = self.test1()?;
                attrs.push(Attr { name: Ident { name: "test".into(), span: tspan }, args: Vec::new(), span: tspan });
            if self.test_panics {
                // A name that cannot be written: only `expects panic` gives it.
                attrs.push(Attr { name: Ident { name: "expects panic".into(), span: tspan }, args: Vec::new(), span: tspan });
            }
                ItemKind::Fn(f)
            }
            TokenKind::Struct | TokenKind::Enum => {
                let span = self.span();
                let word = self.peek().to_string();
                self.removed(span, &word, "`type Name { Type field … }` or `type Name = .A | .B`");
                return Err(());
            }
            TokenKind::Trait => {
                let span = self.span();
                self.removed(span, "trait", "`interface Name { … }`");
                return Err(());
            }
            TokenKind::Impl => {
                let span = self.span();
                self.removed(span, "impl", "the functions inside `type Name { … }`, or `extend Type is Interface { … }`");
                return Err(());
            }
            _ => {
                let found = self.peek().describe();
                let span = self.span();
                self.error_here(
                    Diagnostic::error("E0108", format!("expected an item, found {}", found))
                        .primary(span, "unexpected here")
                        .note("a module contains `fn`, `type`, `interface`, `extend`, `test`, `const`, `static` and `extern` declarations"),
                );
                return Err(());
            }
        };
        Ok(vec![Item { attrs, is_pub, kind, span: start.to(self.prev_span()) }])
    }
    /// `[T is A and B, U]`
    fn generics1(&mut self) -> PResult<(Vec<Ident>, Vec<BoundDecl>)> {
        let mut names = Vec::new();
        let mut bounds = Vec::new();
        if self.eat(&TokenKind::LBracket) {
            loop {
                if self.eat(&TokenKind::RBracket) {
                    break;
                }
                let name = self.ident("a type parameter name")?;
                if self.eat_word("is") {
                    let mut traits = vec![self.type_expr()?];
                    while self.eat(&TokenKind::And) {
                        traits.push(self.type_expr()?);
                    }
                    bounds.push(BoundDecl { param: name.clone(), traits });
                } else if self.is(&TokenKind::Colon) {
                    let span = self.span();
                    self.removed(span, ":", &format!("`{} is Interface`", name.name));
                    return Err(());
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

    /// `[unsafe] fn [Type] name[generics](params) body`
    pub(super) fn fn_decl1(&mut self) -> PResult<FnDecl> {
        let member = std::mem::take(&mut self.member_fn);
        let start = self.span();
        let is_unsafe = self.eat(&TokenKind::Unsafe);
        self.expect(TokenKind::Fn, "`fn`")?;
        let ret = if self.typed_name_here() { Some(self.type_expr()?) } else { None };
        let name = if member { self.member_ident("a function name")? } else { self.ident("a function name")? };
        let (generics, bounds) = self.generics1()?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            params.push(self.param1()?);
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        if self.is(&TokenKind::Arrow) {
            let span = self.span();
            self.removed(span, "->", &format!("the type before the name: `fn Type {}(…)`", name.name));
            return Err(());
        }
        self.clauses1()?;
        let body = if self.is(&TokenKind::LBrace) {
            Some(self.block()?)
        } else if self.is(&TokenKind::Eq) {
            let eq = self.bump().span;
            let e = self.expr()?;
            let span = eq.to(e.span);
            let stmt = if ret.is_some() { StmtKind::Expr(Expr { kind: ExprKind::Return(Some(Box::new(e))), span }) } else { StmtKind::Expr(e) };
            Some(Block { stmts: vec![Stmt { kind: stmt, span }], span })
        } else {
            None
        };
        Ok(FnDecl { name, generics, bounds, params, ret, body, is_unsafe, span: start.to(self.prev_span()) })
    }

    /// `self`, `self!`, `owned self`, `Type name`, `Type name!`, `owned Type name`
    fn param1(&mut self) -> PResult<Param> {
        let start = self.span();
        if self.is(&TokenKind::Inout) {
            self.removed(start, "inout", "the type and the name, then `!`: `Type name!` (or `self!`)");
            return Err(());
        }
        let owned = self.eat(&TokenKind::Owned);
        if self.eat(&TokenKind::SelfValue) {
            let conv = if self.eat(&TokenKind::Bang) {
                Convention::Inout
            } else if owned {
                Convention::Owned
            } else {
                Convention::Borrow
            };
            return Ok(Param { conv, kind: ParamKind::SelfParam, span: start.to(self.prev_span()) });
        }
        if !self.typed_name_here() {
            let span = self.span();
            if matches!(self.peek(), TokenKind::Ident(_)) && self.peek_at(1) == &TokenKind::Colon {
                self.removed(span, "name: Type", "the type before the name: `Type name`");
            } else {
                self.expected("a parameter: its type, then its name");
            }
            return Err(());
        }
        let ty = self.type_expr()?;
        let name = self.ident("a parameter name")?;
        let conv = if self.eat(&TokenKind::Bang) {
            if owned {
                let span = self.prev_span();
                self.error_here(Diagnostic::error("E0101", "a parameter is either `owned` or modified (`!`), not both").primary(span, "remove `!` or `owned`"));
            }
            Convention::Inout
        } else if owned {
            Convention::Owned
        } else {
            Convention::Borrow
        };
        Ok(Param { conv, kind: ParamKind::Named { name, ty }, span: start.to(self.prev_span()) })
    }

    /// `requires`, `ensures`, `with`, `every`: reserved, refused until P5, P7, P8.
    fn clauses1(&mut self) -> PResult<()> {
        while ["requires", "ensures", "with", "every"].iter().any(|w| self.is_word(w)) {
            let start = self.span();
            let what = match self.peek() {
                TokenKind::Ident(w) if w == "requires" || w == "ensures" => "a contract (`requires`, `ensures`)",
                TokenKind::Ident(w) if w == "with" => "a list of effects (`with`)",
                _ => "a periodic task (`every … within …`)",
            };
            self.bump();
            // Skip the clause to report the rest of the function normally.
            while !matches!(self.peek(), TokenKind::LBrace | TokenKind::Eq | TokenKind::Newline | TokenKind::Eof) && !["requires", "ensures", "with", "every"].iter().any(|w| self.is_word(w)) {
                self.bump();
            }
            self.not_yet(start.to(self.prev_span()), what);
        }
        Ok(())
    }

    /// `type Name[T] { fields and functions }`, `type Name is I { … }`, `type Name = .A | .B(T)`,
    /// `type Name is I = .A | .B` (an enumeration that declares interfaces without functions,
    /// such as `Copy`), `type Name = Type`.
    fn type_decl1(&mut self, attrs: Vec<Attr>, is_pub: bool, start: Span) -> PResult<Vec<Item>> {
        self.bump(); // type
        let name = self.ident("a type name")?;
        let (generics, bounds) = self.generics1()?;
        if let Some(b) = bounds.first() {
            let span = b.param.span;
            self.not_yet(span, "a bound on the parameter of a type");
        }
        // The functions of the body apply to the whole type: its parameters are in scope.
        let self_ty = TypeExpr { kind: TypeExprKind::Path { segments: vec![name.clone()], args: vec![] }, span: name.span };
        if self.eat(&TokenKind::Eq) {
            let item_span = |p: &Self| start.to(p.prev_span());
            if self.is(&TokenKind::Dot) || self.is(&TokenKind::Pipe) || self.is(&TokenKind::Indirect) {
                let variants = self.variants1()?;
                return Ok(vec![Item { attrs, is_pub, kind: ItemKind::Enum(EnumDecl { name, generics, variants }), span: item_span(self) }]);
            }
            // `distinct` and `where` (ADR 0014): a new type over `ty`.
            let distinct = self.eat_word("distinct");
            let ty = self.type_expr()?;
            let mut interfaces = Vec::new();
            if distinct && self.eat_word("is") {
                interfaces.push(self.type_expr()?);
                while self.eat(&TokenKind::And) {
                    interfaces.push(self.type_expr()?);
                }
            }
            let cond = if self.eat(&TokenKind::Where) { Some(self.constraint1()?) } else { None };
            if distinct || cond.is_some() {
                if !generics.is_empty() {
                    self.not_yet(name.span, "a generic distinct or constrained type");
                    return Err(());
                }
                let decl = NewtypeDecl { name, base: ty, distinct, interfaces, cond };
                return Ok(vec![Item { attrs, is_pub, kind: ItemKind::Newtype(decl), span: item_span(self) }]);
            }
            return Ok(vec![Item { attrs, is_pub, kind: ItemKind::TypeAlias(TypeAliasDecl { name, ty }), span: item_span(self) }]);
        }
        let mut interfaces: Vec<TypeExpr> = Vec::new();
        if self.eat_word("is") {
            interfaces.push(self.type_expr()?);
            while self.eat(&TokenKind::And) {
                interfaces.push(self.type_expr()?);
            }
        }
        if !interfaces.is_empty() && self.is(&TokenKind::Eq) && matches!(self.peek_at(1), TokenKind::Dot | TokenKind::Pipe | TokenKind::Indirect) {
            self.bump(); // =
            let variants = self.variants1()?;
            let span = start.to(self.prev_span());
            let mut items = vec![Item { attrs, is_pub, kind: ItemKind::Enum(EnumDecl { name, generics, variants }), span }];
            for i in interfaces {
                items.push(Item { attrs: vec![], is_pub: false, kind: ItemKind::Impl(ImplDecl { ty: self_ty.clone(), trait_: Some(i), bounds: vec![], items: vec![], is_unsafe: false }), span });
            }
            return Ok(items);
        }
        let open = self.expect(TokenKind::LBrace, "`{`, `=` or `is`")?;
        self.skip_newlines();
        let mut fields = Vec::new();
        let mut fns = Vec::new();
        loop {
            if self.eat(&TokenKind::RBrace) {
                break;
            }
            if self.at_eof() {
                self.unclosed(open);
                return Err(());
            }
            let res = (|| -> PResult<()> {
                let mstart = self.span();
                let mattrs = self.attrs()?;
                let mpub = self.eat(&TokenKind::Pub);
                // `fn i32(i32) f` is a field whose type is a function; `fn i32 f(…)` a function.
                if matches!(self.peek(), TokenKind::Fn | TokenKind::Unsafe) && !self.typed_name_here() {
                    self.member_fn = true;
                    let f = self.fn_decl1()?;
                    fns.push(Item { attrs: mattrs, is_pub: mpub, kind: ItemKind::Fn(f), span: mstart.to(self.prev_span()) });
                } else {
                    let indirect = self.eat(&TokenKind::Indirect);
                    if !self.typed_name_here() {
                        self.expected("a field (its type, then its name) or a function");
                        return Err(());
                    }
                    let ty = self.type_expr()?;
                    let fname = self.member_ident("a field name")?;
                    fields.push(FieldDecl { is_pub: mpub, indirect, name: fname, ty, span: mstart.to(self.prev_span()) });
                }
                Ok(())
            })();
            if res.is_err() {
                self.recover_stmt();
                continue;
            }
            if self.eat(&TokenKind::Comma) || self.peek().is_terminator() {
                self.skip_newlines();
            } else if !self.is(&TokenKind::RBrace) {
                self.expected("a new line or `}`");
                self.recover_stmt();
            }
        }
        let span = start.to(self.prev_span());
        let mut items = vec![Item { attrs, is_pub, kind: ItemKind::Struct(StructDecl { name, generics, fields }), span }];
        // `Copy` has no function (ADR 0012): `type P is Copy { … fn … }` keeps its functions
        // for the type, or for its other interface.
        let is_copy = |t: &TypeExpr| matches!(&t.kind, TypeExprKind::Path { segments, args } if args.is_empty() && segments.last().is_some_and(|s| s.name == "Copy"));
        let (copy, mut interfaces): (Vec<TypeExpr>, Vec<TypeExpr>) = interfaces.into_iter().partition(is_copy);
        for c in copy {
            items.push(Item { attrs: vec![], is_pub: false, kind: ItemKind::Impl(ImplDecl { ty: self_ty.clone(), trait_: Some(c), bounds: vec![], items: vec![], is_unsafe: false }), span });
        }
        match interfaces.len() {
            0 if !fns.is_empty() => items.push(Item { attrs: vec![], is_pub: false, kind: ItemKind::Impl(ImplDecl { ty: self_ty, trait_: None, bounds: vec![], items: fns, is_unsafe: false }), span }),
            0 => {}
            1 => items.push(Item { attrs: vec![], is_pub: false, kind: ItemKind::Impl(ImplDecl { ty: self_ty, trait_: interfaces.pop(), bounds: vec![], items: fns, is_unsafe: false }), span }),
            _ => {
                if !fns.is_empty() {
                    self.not_yet(span, "functions in a type that declares several interfaces (write them in `extend Type is I { … }`)");
                }
                for i in interfaces {
                    items.push(Item { attrs: vec![], is_pub: false, kind: ItemKind::Impl(ImplDecl { ty: self_ty.clone(), trait_: Some(i), bounds: vec![], items: vec![], is_unsafe: false }), span });
                }
            }
        }
        Ok(items)
    }

    /// The condition of a constrained type (ADR 0014 §2.2): `or` of `and` of `not` of
    /// comparisons of `it` with a constant, `it in a..b` / `it in a..=b`, or parentheses.
    fn constraint1(&mut self) -> PResult<Constraint> {
        let mut c = self.constraint_and1()?;
        while self.eat(&TokenKind::Or) {
            let r = self.constraint_and1()?;
            let span = c.span.to(r.span);
            c = Constraint { kind: ConstraintKind::Or(Box::new(c), Box::new(r)), span };
        }
        Ok(c)
    }

    fn constraint_and1(&mut self) -> PResult<Constraint> {
        let mut c = self.constraint_not1()?;
        while self.eat(&TokenKind::And) {
            let r = self.constraint_not1()?;
            let span = c.span.to(r.span);
            c = Constraint { kind: ConstraintKind::And(Box::new(c), Box::new(r)), span };
        }
        Ok(c)
    }

    fn constraint_not1(&mut self) -> PResult<Constraint> {
        let start = self.span();
        if self.eat(&TokenKind::Not) {
            let c = self.constraint_not1()?;
            let span = start.to(c.span);
            return Ok(Constraint { kind: ConstraintKind::Not(Box::new(c)), span });
        }
        if self.eat(&TokenKind::LParen) {
            let c = self.constraint1()?;
            self.expect(TokenKind::RParen, "`)`")?;
            return Ok(Constraint { kind: c.kind, span: start.to(self.prev_span()) });
        }
        // `it` on one side of a comparison, a constant on the other.
        let left_it = self.eat_word("it");
        let left = if left_it { None } else { Some(self.unary()?) };
        if left_it && self.eat(&TokenKind::In) {
            let lo = self.unary()?;
            let inclusive = if self.eat(&TokenKind::DotDotEq) {
                true
            } else {
                self.expect(TokenKind::DotDot, "`..` or `..=`")?;
                false
            };
            let hi = self.unary()?;
            return Ok(Constraint { kind: ConstraintKind::In { lo, hi, inclusive }, span: start.to(self.prev_span()) });
        }
        let op = match self.peek() {
            TokenKind::EqEq => BinOp::Eq,
            TokenKind::NotEq => BinOp::Ne,
            TokenKind::Lt => BinOp::Lt,
            TokenKind::Le => BinOp::Le,
            TokenKind::Gt => BinOp::Gt,
            TokenKind::Ge => BinOp::Ge,
            _ => {
                self.expected("a comparison of `it` with a constant (`it >= 0`, `it in 1..=9`)");
                return Err(());
            }
        };
        self.bump();
        let right_it = self.eat_word("it");
        let right = if right_it { None } else { Some(self.unary()?) };
        let span = start.to(self.prev_span());
        match (left, right) {
            (None, Some(value)) => Ok(Constraint { kind: ConstraintKind::Cmp { op, value, it_left: true }, span }),
            (Some(value), None) => Ok(Constraint { kind: ConstraintKind::Cmp { op, value, it_left: false }, span }),
            _ => {
                self.diags.emit(
                    Diagnostic::error("E0251", "a condition compares `it` with a constant")
                        .primary(span, if left_it { "`it` on both sides" } else { "`it` is missing" })
                        .help("write `it >= 0`, `it < 100` or `it in 0..=100`"),
                );
                Err(())
            }
        }
    }

    /// `.A | .B(T, U name) | indirect .C(Self)`, possibly over several lines starting with `|`.
    fn variants1(&mut self) -> PResult<Vec<VariantDecl>> {
        let mut variants = Vec::new();
        self.eat(&TokenKind::Pipe);
        loop {
            let start = self.span();
            let indirect = self.eat(&TokenKind::Indirect);
            self.expect(TokenKind::Dot, "`.` and a variant name (`.Name`)")?;
            let name = self.ident("a variant name")?;
            let mut fields = Vec::new();
            if self.eat(&TokenKind::LParen) {
                loop {
                    if self.eat(&TokenKind::RParen) {
                        break;
                    }
                    fields.push(self.type_expr()?);
                    // An optional name for the value (documentation only for now).
                    if matches!(self.peek(), TokenKind::Ident(_)) {
                        self.bump();
                    }
                    if !self.eat(&TokenKind::Comma) {
                        self.expect(TokenKind::RParen, "`,` or `)`")?;
                        break;
                    }
                }
            }
            variants.push(VariantDecl { indirect, name, fields, span: start.to(self.prev_span()) });
            if !self.eat(&TokenKind::Pipe) {
                break;
            }
        }
        Ok(variants)
    }

    /// `interface Name [is A and B] { fn signatures, with or without a body }`
    fn interface1(&mut self) -> PResult<TraitDecl> {
        self.bump(); // interface
        let name = self.ident("an interface name")?;
        let mut supertraits = Vec::new();
        if self.eat_word("is") {
            supertraits.push(self.type_expr()?);
            while self.eat(&TokenKind::And) {
                supertraits.push(self.type_expr()?);
            }
        }
        let items = self.fn_body_items("a function of the interface")?;
        Ok(TraitDecl { name, supertraits, items })
    }

    /// `extend Type [is I] { functions }`; a generic type names its parameters, with their
    /// bounds if any: `extend Paire[A is Display] is Display { … }`.
    fn extend1(&mut self) -> PResult<ImplDecl> {
        self.bump(); // extend
        let start = self.span();
        let (ty, bounds) = if matches!(self.peek(), TokenKind::Ident(_)) && !self.is_word("any") {
            let mut segments = vec![self.ident("a type name")?];
            while self.is(&TokenKind::Dot) {
                self.bump();
                segments.push(self.ident("a type name")?);
            }
            let (names, bounds) = self.generics1()?;
            let args = names.into_iter().map(|n| TypeExpr { span: n.span, kind: TypeExprKind::Path { segments: vec![n], args: vec![] } }).collect();
            (TypeExpr { kind: TypeExprKind::Path { segments, args }, span: start.to(self.prev_span()) }, bounds)
        } else {
            (self.type_expr()?, vec![])
        };
        let trait_ = if self.eat_word("is") { Some(self.type_expr()?) } else { None };
        let items = self.fn_body_items("a function")?;
        Ok(ImplDecl { ty, trait_, bounds, items, is_unsafe: false })
    }

    /// `{ [attributes] [pub] fn … }`
    fn fn_body_items(&mut self, what: &str) -> PResult<Vec<Item>> {
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
            let res = (|| -> PResult<Item> {
                let start = self.span();
                let attrs = self.attrs()?;
                let is_pub = self.eat(&TokenKind::Pub);
                if !matches!(self.peek(), TokenKind::Fn | TokenKind::Unsafe) {
                    self.expected(what);
                    return Err(());
                }
                self.member_fn = true;
                let f = self.fn_decl1()?;
                Ok(Item { attrs, is_pub, kind: ItemKind::Fn(f), span: start.to(self.prev_span()) })
            })();
            match res {
                Ok(it) => {
                    items.push(it);
                    if self.expect_terminator(&TokenKind::RBrace).is_err() {
                        self.recover_stmt();
                    }
                }
                Err(()) => self.recover_stmt(),
            }
        }
        Ok(items)
    }

    /// `test "description" [expects panic] { … }`: a function marked `@test`.
    fn test1(&mut self) -> PResult<FnDecl> {
        let start = self.bump().span; // test
        let (desc, dspan) = match self.peek().clone() {
            TokenKind::Str(s) => match s.parts.as_slice() {
                [StrPart::Text(t)] => (t.clone(), self.bump().span),
                _ => {
                    self.expected("a description in plain text, without `{…}`");
                    return Err(());
                }
            },
            _ => {
                self.expected("the description of the test, in quotes");
                return Err(());
            }
        };
        // `expects panic`: the test passes when it panics.
        self.test_panics = false;
        if self.eat_word("expects") {
            if !self.eat_word("panic") {
                self.expected("`panic` after `expects`");
                return Err(());
            }
            self.test_panics = true;
        }
        let body = self.block()?;
        let name = Ident { name: self.test_name(&desc), span: dspan };
        Ok(FnDecl { name, generics: vec![], bounds: vec![], params: vec![], ret: None, body: Some(body), is_unsafe: false, span: start.to(self.prev_span()) })
    }

    /// The name of the function of a test: its description in ASCII, words joined by `_`.
    fn test_name(&mut self, desc: &str) -> String {
        let mut s = String::new();
        for c in desc.chars() {
            let c = match c {
                'à' | 'â' | 'ä' | 'á' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'î' | 'ï' | 'í' => 'i',
                'ô' | 'ö' | 'ó' => 'o',
                'ù' | 'û' | 'ü' | 'ú' => 'u',
                'ç' => 'c',
                'À' | 'Â' | 'Ä' => 'A',
                'É' | 'È' | 'Ê' | 'Ë' => 'E',
                'Ç' => 'C',
                c => c,
            };
            if c.is_ascii_alphanumeric() {
                s.push(c);
            } else if !s.ends_with('_') {
                s.push('_');
            }
        }
        let s = s.trim_matches('_').to_string();
        let base = if s.is_empty() || s.as_bytes()[0].is_ascii_digit() { format!("test_{}", s) } else { s };
        let mut name = base.clone();
        let mut n = 2;
        while self.test_names.contains(&name) {
            name = format!("{}_{}", base, n);
            n += 1;
        }
        self.test_names.push(name.clone());
        name
    }

    /// `extern "C" fn [Type] name(Type a, …)`
    fn extern_fn1(&mut self) -> PResult<ExternFnDecl> {
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
        let ret = if self.typed_name_here() { Some(self.type_expr()?) } else { None };
        let name = self.ident("a function name")?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            let ty = self.type_expr()?;
            let pname = self.ident("a parameter name")?;
            params.push((pname, ty));
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
        Ok(ExternFnDecl { abi, abi_span, name, params, ret })
    }

    // -----------------------------------------------------------------------
    // Statements

    /// The statements of edition 1 that differ from 0.2; `None`: parse as in 0.2.
    pub(super) fn stmt1(&mut self) -> PResult<Option<StmtKind>> {
        let start = self.span();
        Ok(Some(match self.peek().clone() {
            TokenKind::Let => {
                self.bump();
                let ty = if self.typed_name_here() { Some(self.type_expr()?) } else { None };
                let pat = self.pattern()?;
                if self.is(&TokenKind::Colon) {
                    let span = self.span();
                    self.removed(span, "let name: Type", "the type before the name: `let Type name = …`");
                    return Err(());
                }
                self.expect(TokenKind::Eq, "`=` and a value")?;
                let init = self.expr()?;
                let init = self.fallback(init)?;
                StmtKind::Let { pat, ty, init }
            }
            TokenKind::Var => {
                self.bump();
                let ty = if self.typed_name_here() { Some(self.type_expr()?) } else { None };
                let name = self.ident("a variable name")?;
                if self.is(&TokenKind::Colon) {
                    let span = self.span();
                    self.removed(span, "var name: Type", "the type before the name: `var Type name = …`");
                    return Err(());
                }
                let init = if self.eat(&TokenKind::Eq) { Some(self.expr()?) } else { None };
                StmtKind::Var { name, ty, init }
            }
            TokenKind::Hash => {
                self.bump();
                let label = self.ident("a label name")?;
                match self.loop1(Some(label))? {
                    Some(k) => k,
                    None => {
                        self.expected("a loop after the label (`for`, `while`, `loop`, `repeat`)");
                        return Err(());
                    }
                }
            }
            TokenKind::For | TokenKind::While => self.loop1(None)?.expect("a loop"),
            TokenKind::Ident(w) if (w == "loop" || w == "repeat") && !matches!(self.peek_at(1), TokenKind::Dot | TokenKind::LParen | TokenKind::Eq) => self.loop1(None)?.expect("a loop"),
            TokenKind::Ident(_) if self.peek_at(1) == &TokenKind::Colon && matches!(self.peek_at(2), TokenKind::While | TokenKind::For) => {
                self.removed(start, "label: for", "`#label for …`");
                return Err(());
            }
            _ => return Ok(None),
        }))
    }

    /// `for`, `while`, `loop`, `repeat`, after an optional label.
    fn loop1(&mut self, label: Option<Ident>) -> PResult<Option<StmtKind>> {
        let start = self.span();
        Ok(Some(match self.peek().clone() {
            TokenKind::For => {
                self.bump();
                if self.typed_name_here() {
                    let span = self.span();
                    self.not_yet(span, "a type on the variable of a `for` loop");
                    return Err(());
                }
                let pat = self.pattern()?;
                let mutable = self.eat(&TokenKind::Bang);
                self.expect(TokenKind::In, "`in`")?;
                if self.is(&TokenKind::Amp) {
                    let span = self.span();
                    self.removed(span, "for x in &v", "`for x! in v`");
                    return Err(());
                }
                let first = self.expr()?;
                let iter = if self.is(&TokenKind::DotDot) || self.is(&TokenKind::DotDotEq) {
                    let inclusive = self.bump().kind == TokenKind::DotDotEq;
                    let end = self.expr()?;
                    ForIter::Range { start: first, end, inclusive }
                } else {
                    ForIter::Expr { expr: first, mutable }
                };
                let body = self.block()?;
                StmtKind::For { label, pat, iter, body }
            }
            TokenKind::While => {
                self.bump();
                if self.is(&TokenKind::Let) {
                    let span = self.span();
                    self.removed(span, "while let P = e", "`while e is P`");
                    return Err(());
                }
                let cond = self.expr()?;
                let body = self.block()?;
                match is_test(&cond) {
                    // `while e is P { body }` is `while true { if e is P { body } else { break } }`.
                    Some((pat, e, mutable)) => {
                        let span = start.to(body.span);
                        let brk = Expr { kind: ExprKind::Break(None), span };
                        let else_block = Block { stmts: vec![Stmt { kind: StmtKind::Expr(brk), span }], span };
                        let if_is = Expr {
                            kind: ExprKind::If { cond: Cond::Let(Box::new(pat), Box::new(e), mutable), then: body, else_: Some(Box::new(Expr { kind: ExprKind::Block(else_block), span })) },
                            span,
                        };
                        let outer = Block { stmts: vec![Stmt { kind: StmtKind::Expr(if_is), span }], span };
                        StmtKind::While { label, cond: Expr { kind: ExprKind::Lit(Lit::Bool(true)), span: start }, body: outer }
                    }
                    None => StmtKind::While { label, cond, body },
                }
            }
            TokenKind::Ident(w) if w == "loop" => {
                self.bump();
                let body = self.block()?;
                StmtKind::While { label, cond: Expr { kind: ExprKind::Lit(Lit::Bool(true)), span: start }, body }
            }
            TokenKind::Ident(w) if w == "repeat" => {
                // `repeat n { … }` is `for _ in 0..n { … }`.
                self.bump();
                let count = self.expr()?;
                let body = self.block()?;
                let zero = Expr { kind: ExprKind::Lit(Lit::Int(0)), span: start };
                StmtKind::For { label, pat: Pattern { kind: PatKind::Wild, span: start }, iter: ForIter::Range { start: zero, end: count, inclusive: false }, body }
            }
            _ => return Ok(None),
        }))
    }

    /// `value else fallback`, after the value of a `let` or of a statement (ADR 0010 §6):
    /// the fallback is a value or a block (`else { return }`).
    pub(super) fn fallback(&mut self, value: Expr) -> PResult<Expr> {
        if !self.eat(&TokenKind::Else) {
            return Ok(value);
        }
        let fb = if self.is(&TokenKind::LBrace) {
            let b = self.block()?;
            let span = b.span;
            Expr { kind: ExprKind::Block(b), span }
        } else {
            self.expr()?
        };
        let span = value.span.to(fb.span);
        Ok(Expr { kind: ExprKind::Fallback(Box::new(value), Box::new(fb)), span })
    }

    /// `x in a..b`, `x in a..=b` (bounds compared, `x` read once) and `x in c`
    /// (`c.contains(x)`), after `in`.
    pub(super) fn in_expr(&mut self, x: Expr) -> PResult<Expr> {
        let lo = self.binary(4)?;
        if !(self.is(&TokenKind::DotDot) || self.is(&TokenKind::DotDotEq)) {
            let span = x.span.to(lo.span);
            let method = Expr { kind: ExprKind::Field(Box::new(lo), Ident { name: "contains".into(), span }), span };
            let arg = Arg { label: None, inout: None, span: x.span, expr: x };
            return Ok(Expr { kind: ExprKind::Call(Box::new(method), vec![arg]), span });
        }
        let inclusive = self.bump().kind == TokenKind::DotDotEq;
        let hi = self.binary(4)?;
        let span = x.span.to(hi.span);
        // A value that is not a simple name is computed once, in a hidden variable.
        let simple = |e: &Expr| {
            let mut e = e;
            loop {
                match &e.kind {
                    ExprKind::Ident(_) | ExprKind::SelfValue | ExprKind::Lit(_) => return true,
                    ExprKind::Field(b, _) | ExprKind::TupleField(b, _, _) | ExprKind::Paren(b) => e = b,
                    _ => return false,
                }
            }
        };
        let (value, hidden) = if simple(&x) { (x, None) } else { (Expr { kind: ExprKind::Ident("<in>".into()), span: x.span }, Some(x)) };
        let bin = |op, a: Expr, b: Expr| Expr { kind: ExprKind::Binary(op, Box::new(a), Box::new(b)), span };
        let test = bin(BinOp::And, bin(BinOp::Le, lo, value.clone()), bin(if inclusive { BinOp::Le } else { BinOp::Lt }, value, hi));
        Ok(match hidden {
            None => test,
            Some(x) => {
                let pat = Pattern { kind: PatKind::Ident("<in>".into()), span: x.span };
                let stmts = vec![Stmt { kind: StmtKind::Let { pat, ty: None, init: x }, span }, Stmt { kind: StmtKind::Expr(test), span }];
                Expr { kind: ExprKind::Block(Block { stmts, span }), span }
            }
        })
    }

    // -----------------------------------------------------------------------
    // Expressions

    /// `when e[!] { pattern[, pattern] [if guard] then value … [else value] }`
    pub(super) fn when1(&mut self) -> PResult<Expr> {
        let start = self.bump().span; // when
        let scrutinee = self.expr_no_bang()?;
        let mutable = self.eat(&TokenKind::Bang);
        let open = self.expect(TokenKind::LBrace, "`{` after the value to examine")?;
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
                let is_else = self.eat(&TokenKind::Else);
                let pat = if is_else {
                    Pattern { kind: PatKind::Wild, span: astart }
                } else {
                    let first = self.pattern()?;
                    if self.is(&TokenKind::Comma) {
                        let mut alts = vec![first];
                        while self.eat(&TokenKind::Comma) {
                            alts.push(self.pattern()?);
                        }
                        let span = alts[0].span.to(alts.last().unwrap().span);
                        Pattern { kind: PatKind::Or(alts), span }
                    } else {
                        first
                    }
                };
                let guard = if !is_else && self.eat(&TokenKind::If) { Some(self.expr()?) } else { None };
                if !is_else && !self.eat_word("then") {
                    if self.is(&TokenKind::FatArrow) {
                        let span = self.span();
                        self.removed(span, "=>", "`then`");
                    } else {
                        self.expected("`then` and the value of this case");
                    }
                    return Err(());
                }
                let body = if self.is(&TokenKind::LBrace) {
                    let b = self.block()?;
                    let span = b.span;
                    Expr { kind: ExprKind::Block(b), span }
                } else {
                    let e = self.expr()?;
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
            if self.peek().is_terminator() {
                self.skip_newlines();
            } else if !self.is(&TokenKind::RBrace) {
                self.expected("a new line or `}` after a case");
                self.recover_stmt();
            }
        }
        Ok(Expr { kind: ExprKind::Match { scrutinee: Box::new(scrutinee), arms, mutable }, span: start.to(self.prev_span()) })
    }

    /// An expression, without taking a final `!` (the marker of `when e!`).
    fn expr_no_bang(&mut self) -> PResult<Expr> {
        self.no_bang = true;
        let r = self.expr();
        self.no_bang = false;
        r
    }

    /// `fn(Type x, y) => e` or `fn [Type](Type x) { … }`
    pub(super) fn closure1(&mut self) -> PResult<Expr> {
        let start = self.bump().span; // fn
        let ret = if self.is(&TokenKind::LParen) { None } else { Some(self.type_expr()?) };
        self.expect(TokenKind::LParen, "`(` and the parameters of the anonymous function")?;
        let mut params = Vec::new();
        loop {
            if self.eat(&TokenKind::RParen) {
                break;
            }
            let owned = self.eat(&TokenKind::Owned);
            let ty = if self.typed_name_here() { Some(self.type_expr()?) } else { None };
            let name = self.ident("a parameter name")?;
            let conv = if self.eat(&TokenKind::Bang) {
                Convention::Inout
            } else if owned {
                Convention::Owned
            } else {
                Convention::Borrow
            };
            params.push(ClosureParam { conv, name, ty });
            if !self.eat(&TokenKind::Comma) {
                self.expect(TokenKind::RParen, "`,` or `)`")?;
                break;
            }
        }
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

    /// `e is P`: a test that gives `true` or `false` (`match e { P => true, _ => false }`);
    /// as the whole condition of `if` or `while`, it binds the names of `P`.
    pub(super) fn is_expr(&mut self, lhs: Expr) -> PResult<Expr> {
        // `e! is P`: the names of `P` modify `e` in place.
        let mutable = self.toks[self.pos - 1].kind == TokenKind::Bang;
        self.bump(); // is
        let pat = self.pattern()?;
        let span = lhs.span.to(pat.span);
        let t = Expr { kind: ExprKind::Lit(Lit::Bool(true)), span: pat.span };
        let f = Expr { kind: ExprKind::Lit(Lit::Bool(false)), span: pat.span };
        let wild = Pattern { kind: PatKind::Wild, span: pat.span };
        let arms = vec![Arm { pat, guard: None, body: t, span }, Arm { pat: wild, guard: None, body: f, span }];
        Ok(Expr { kind: ExprKind::Match { scrutinee: Box::new(lhs), arms, mutable }, span })
    }

    /// `a |> f(b)` is `f(a, b)`; `a |> f` is `f(a)`.
    pub(super) fn pipe1(&mut self, lhs: Expr) -> PResult<Expr> {
        self.bump(); // |>
        let rhs = self.postfix()?;
        let span = lhs.span.to(rhs.span);
        let first = Arg { label: None, inout: None, span: lhs.span, expr: lhs };
        Ok(match rhs.kind {
            ExprKind::Call(f, mut args) => {
                args.insert(0, first);
                Expr { kind: ExprKind::Call(f, args), span }
            }
            _ => Expr { kind: ExprKind::Call(Box::new(rhs), vec![first]), span },
        })
    }
}

/// `match e { P => true, _ => false }` built by `is`: the pattern, the value, and whether
/// the bindings modify it (`e! is P`).
pub(super) fn is_test(e: &Expr) -> Option<(Pattern, Expr, bool)> {
    let ExprKind::Match { scrutinee, arms, mutable } = &e.kind else { return None };
    match arms.as_slice() {
        [a, b] if matches!(a.body.kind, ExprKind::Lit(Lit::Bool(true))) && matches!(b.body.kind, ExprKind::Lit(Lit::Bool(false))) && matches!(b.pat.kind, PatKind::Wild) && a.guard.is_none() => {
            Some((a.pat.clone(), (**scrutinee).clone(), *mutable))
        }
        _ => None,
    }
}
