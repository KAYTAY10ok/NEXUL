//! Place rules of the memory model (spec chapter 05), checked on the THIR:
//! mutability (E0405, E0410), moves out of places (E0403) and the law of
//! exclusivity (E0420). Initialization and use-after-move are checked on the NIR.

use crate::defs::Program;
use crate::thir::{self, Block, Body, CallArg, Callee, Expr, ExprKind as T, LocalKind, Pat, PatKind, StmtKind as S};
use nxc_syntax::ast::Convention;
use nxc_syntax::{DiagSink, Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
enum Proj {
    Field(u32),
    Index,
}

#[derive(Clone, Debug)]
struct PlacePath {
    root: thir::LocalId,
    projs: Vec<Proj>,
}

fn place_path(e: &Expr) -> Option<PlacePath> {
    match &e.kind {
        T::Local(l) => Some(PlacePath { root: *l, projs: vec![] }),
        T::Field(b, i) => {
            let mut p = place_path(b)?;
            p.projs.push(Proj::Field(*i));
            Some(p)
        }
        T::Index(b, _) | T::IndexFn { base: b, .. } => {
            let mut p = place_path(b)?;
            p.projs.push(Proj::Index);
            Some(p)
        }
        _ => None,
    }
}

/// Two places overlap if one is a prefix of the other; indexed accesses to the same
/// collection always overlap (indices are not compared).
fn overlaps(a: &PlacePath, b: &PlacePath) -> bool {
    if a.root != b.root {
        return false;
    }
    for (x, y) in a.projs.iter().zip(b.projs.iter()) {
        if let (Proj::Field(i), Proj::Field(j)) = (x, y) {
            if i != j {
                return false;
            }
        }
    }
    true
}

struct Frozen {
    path: PlacePath,
    span: Span,
    why: &'static str,
    /// Borrowed for writing: even reads through other paths are forbidden.
    exclusive: bool,
}

struct Cx<'a> {
    p: &'a Program,
    body: &'a Body,
    diags: &'a DiagSink,
    frozen: Vec<Frozen>,
    /// The bindings of the arms whose guard is being checked. In a guard they are
    /// borrowed views, even when the scrutinee is a value: the parts are moved into them
    /// only once the guard is true (spec §5.6).
    guard_bindings: Vec<thir::LocalId>,
    /// The trait bounds of the function: a parameter `T: Copy` is copyable.
    bounds: &'a [crate::defs::Bound],
}

pub fn check_program(prog: &Program, diags: &DiagSink) {
    let mut ids: Vec<_> = prog.bodies.keys().copied().collect();
    ids.sort();
    for id in ids {
        let body = &prog.bodies[&id];
        let mut cx = Cx { p: prog, body, diags, frozen: Vec::new(), guard_bindings: Vec::new(), bounds: &prog.func(id).bounds };
        cx.block(&body.block);
    }
}

impl<'a> Cx<'a> {
    fn local_name(&self, l: thir::LocalId) -> &str {
        &self.body.locals[l as usize].name
    }

    fn describe(&self, e: &Expr) -> String {
        match &e.kind {
            T::Local(l) => format!("`{}`", self.local_name(*l)),
            T::Field(b, i) => {
                let bt = self.p.tcx.as_adt(b.ty);
                match bt {
                    Some((id, _)) if !self.p.adt(id).is_enum() => format!("field `{}` of {}", self.p.adt(id).fields()[*i as usize].name, self.describe(b)),
                    _ => format!("element {} of {}", i, self.describe(b)),
                }
            }
            T::Index(b, _) | T::IndexFn { base: b, .. } => format!("an element of {}", self.describe(b)),
            _ => "this value".into(),
        }
    }

    /// A write to a place (assignment, `inout`, move): checks mutability and freezing.
    fn write(&self, e: &Expr, span: Span, action: &str, check_mut: bool) {
        if let (Some(st), true) = (e.static_root(), check_mut) {
            let sd = &self.p.statics[st.idx()];
            if !sd.mutable {
                self.diags.emit(
                    Diagnostic::error("E0405", format!("cannot {} the immutable static `{}`", action, sd.name))
                        .primary(span, "not modifiable")
                        .secondary(sd.span, "declared here")
                        .help(format!("declare it with `static var {}` (its uses then require `unsafe`)", sd.name)),
                );
            }
            return;
        }
        let Some(path) = place_path(e) else { return };
        for f in self.frozen.iter().rev() {
            if overlaps(&f.path, &path) {
                let root = self.local_name(path.root);
                self.diags.emit(
                    Diagnostic::error("E0420", format!("cannot {} {} while `{}` is borrowed", action, self.describe(e), root))
                        .primary(span, format!("{} here", action))
                        .secondary(f.span, format!("`{}` is borrowed by this {} until its end", root, f.why))
                        .note("a value cannot change while it is being read (law of exclusivity, spec §5.4)"),
                );
                return;
            }
        }
        if !check_mut {
            return;
        }
        let decl = &self.body.locals[path.root as usize];
        if decl.is_mutable() || decl.kind == LocalKind::Hidden {
            return;
        }
        let (msg, help) = match decl.kind {
            LocalKind::Let => (format!("cannot {} {}: `{}` is immutable", action, self.describe(e), decl.name), Some(format!("declare it with `var {}`", decl.name))),
            LocalKind::Param(Convention::Borrow) => (
                format!("cannot {} {}: parameter `{}` is borrowed read-only", action, self.describe(e), decl.name),
                Some(if decl.name == "self" { "declare the method with `inout self`".to_string() } else { format!("declare the parameter `inout {}: ...` (callers then pass `&{}`)", decl.name, decl.name) }),
            ),
            LocalKind::Ref => (format!("cannot {} {}: `{}` is a read-only view of the value being iterated or matched", action, self.describe(e), decl.name), None),
            _ => (format!("cannot {} {}", action, self.describe(e)), None),
        };
        let mut d = Diagnostic::error("E0405", msg).primary(span, "not modifiable").secondary(decl.span, "declared here");
        if let Some(h) = help {
            d = d.help(h);
        }
        self.diags.emit(d);
    }

    /// A binding moved in the guard of its own arm (E0403). The guard only borrows the
    /// matched parts: if it moved one and then failed, the next arms would see a value
    /// already destroyed (use after free, double destruction).
    fn moved_in_guard(&self, l: thir::LocalId, e: &Expr) -> bool {
        let decl = &self.body.locals[l as usize];
        if !self.guard_bindings.contains(&l) || decl.is_ref() || self.p.is_copy_with(decl.ty, self.bounds) {
            return false;
        }
        let mut d = Diagnostic::error("E0403", format!("cannot move `{}` in the guard of its arm", decl.name))
            .primary(e.span, format!("`{}` is a resource and the guard only borrows it", self.p.ty_str(decl.ty)))
            .secondary(decl.span, "bound here")
            .note("the matched value is moved into the bindings only once the guard is true (spec §5.6)")
            .help("move it in the body of the arm instead");
        if self.p.is_string(decl.ty) {
            d = d.help("or use `.clone()` to make a copy");
        }
        self.diags.emit(d);
        true
    }

    /// An expression in an owned position: a resource stored in a place must be a
    /// whole owned variable to be moved (E0403).
    fn consume(&self, e: &Expr) {
        match &e.kind {
            T::Block(b) => {
                if let Some(t) = &b.tail {
                    self.consume(t);
                }
                return;
            }
            T::If { then, else_, .. } | T::IfLet { then, else_, .. } => {
                if let Some(t) = &then.tail {
                    self.consume(t);
                }
                if let Some(x) = else_ {
                    self.consume(x);
                }
                return;
            }
            T::Match { arms, .. } => {
                for a in arms {
                    self.consume(&a.body);
                }
                return;
            }
            _ => {}
        }
        if self.p.is_copy_with(e.ty, self.bounds) {
            return;
        }
        if !e.is_place() {
            // A part of a temporary value: moving it would leave the rest of the
            // temporary half-destroyed.
            if matches!(e.kind, T::Field(..) | T::Index(..) | T::IndexFn { .. }) {
                let tname = self.p.ty_str(e.ty);
                let mut d = Diagnostic::error("E0403", "cannot move a part out of a temporary value")
                    .primary(e.span, format!("`{}` is a resource stored inside a temporary", tname))
                    .help("store the temporary in a variable first, then move the whole variable or copy the part");
                if self.p.is_string(e.ty) {
                    d = d.help("use `.clone()` to make a copy");
                }
                self.diags.emit(d);
            }
            return;
        }
        let tname = self.p.ty_str(e.ty);
        let clone_help = if self.p.is_string(e.ty) { Some("use `.clone()` to make a copy".to_string()) } else { None };
        match &e.kind {
            T::Local(l) => {
                let decl = &self.body.locals[*l as usize];
                if self.moved_in_guard(*l, e) {
                    return;
                }
                if decl.is_ref() {
                    let mut d = Diagnostic::error("E0403", format!("cannot move out of `{}`, which is borrowed", decl.name))
                        .primary(e.span, format!("`{}` is a resource and only a borrowed view is available here", tname));
                    d = match decl.kind {
                        LocalKind::Param(_) => d.help(format!("take ownership with `owned {}: ...`, or copy the value", decl.name)),
                        _ => d.note(format!("`{}` is a read-only view into another value (a `for` element or a pattern binding)", decl.name)),
                    };
                    if let Some(h) = clone_help {
                        d = d.help(h);
                    }
                    self.diags.emit(d);
                } else {
                    self.write(e, e.span, "move", false);
                }
            }
            _ => {
                let mut d = Diagnostic::error("E0403", format!("cannot move {} out of its place", self.describe(e)))
                    .primary(e.span, format!("`{}` is a resource: moving it would leave a hole", tname))
                    .note("only a whole variable can be moved; a field or an element stays where it is");
                if let Some(h) = clone_help {
                    d = d.help(h);
                }
                if let Some(opt) = self.p.lang.option {
                    if self.p.tcx.as_adt(e.ty).map(|(id, _)| id) == Some(opt) {
                        d = d.help("use `.take()` to move the value out and leave `None`");
                    }
                }
                self.diags.emit(d);
            }
        }
    }

    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            match &s.kind {
                S::Let { pat, init } => {
                    self.expr(init);
                    // `let _ = place` binds nothing: the place is only read.
                    if !matches!(pat.kind, PatKind::Wild) {
                        self.consume(init);
                    }
                    self.check_destructure(pat);
                }
                S::Var { init, .. } => {
                    if let Some(e) = init {
                        self.expr(e);
                        self.consume(e);
                    }
                }
                S::Assign { place, value } => {
                    self.expr_place_subexprs(place);
                    self.expr(value);
                    self.consume(value);
                    self.write(place, place.span, "assign to", true);
                }
                S::CompoundAssign { place, value, .. } => {
                    self.expr_place_subexprs(place);
                    self.expr(value);
                    self.write(place, place.span, "modify", true);
                }
                S::Expr(e) => self.expr(e),
                S::While { cond, body, .. } => {
                    self.expr(cond);
                    self.block(body);
                }
                S::ForRange { start, end, body, .. } => {
                    self.expr(start);
                    self.expr(end);
                    self.block(body);
                }
                S::ForEach { iter, body, mutable, .. } => {
                    self.expr(iter);
                    if *mutable {
                        self.write(iter, iter.span, "modify", true);
                    }
                    let pushed = match place_path(iter) {
                        Some(path) => {
                            self.frozen.push(Frozen { path, span: iter.span, why: "`for` loop", exclusive: *mutable });
                            true
                        }
                        None => false,
                    };
                    self.block(body);
                    if pushed {
                        self.frozen.pop();
                    }
                }
            }
        }
        if let Some(t) = &b.tail {
            self.expr(t);
        }
    }

    fn place_subexprs(&mut self, e: &Expr) {
        match &e.kind {
            T::Field(b, _) => self.place_subexprs(b),
            T::Index(b, i) | T::IndexFn { base: b, index: i, .. } => {
                self.place_subexprs(b);
                self.expr(i);
            }
            _ => {}
        }
    }

    /// For an assignment target, only the index expressions are evaluated.
    fn expr_place_subexprs(&mut self, e: &Expr) {
        match &e.kind {
            T::Field(b, _) => self.expr_place_subexprs(b),
            T::Index(b, i) | T::IndexFn { base: b, index: i, .. } => {
                self.expr_place_subexprs(b);
                self.expr(i);
            }
            _ => {}
        }
    }

    /// A pattern that takes ownership of parts of a value whose type defines `drop`
    /// would prevent its destructor from running (E0431).
    fn check_destructure(&self, p: &Pat) {
        let moves_resource = |q: &Pat| {
            let mut bs = Vec::new();
            q.bindings(&mut bs);
            bs.iter().any(|&l| !self.p.is_copy_with(self.body.locals[l as usize].ty, self.bounds))
        };
        match &p.kind {
            PatKind::Struct { adt, fields, .. } | PatKind::Variant { adt, fields, .. } => {
                let a = self.p.adt(*adt);
                if a.drop_fn.is_some() && fields.iter().any(|f| moves_resource(f)) {
                    self.diags.emit(
                        Diagnostic::error("E0431", format!("`{}` defines `drop` and cannot be taken apart", a.name))
                            .primary(p.span, "this pattern moves parts out of the value")
                            .note("the destructor must run on the whole value")
                            .help("bind the whole value, or copy the parts you need"),
                    );
                    return;
                }
                fields.iter().for_each(|f| self.check_destructure(f));
            }
            PatKind::Tuple(ps) | PatKind::Or(ps) => ps.iter().for_each(|f| self.check_destructure(f)),
            _ => {}
        }
    }

    fn has_ref_binding(&self, p: &Pat) -> bool {
        let mut v = Vec::new();
        p.bindings(&mut v);
        v.iter().any(|&l| {
            let d = &self.body.locals[l as usize];
            d.kind == LocalKind::Ref && !self.p.is_copy_with(d.ty, self.bounds)
        })
    }

    fn with_frozen<F: FnOnce(&mut Self)>(&mut self, scrutinee: &Expr, pat: &Pat, mutable: bool, f: F) {
        // A view exists only while bindings hold it: an arm without bindings (`None => x = ...`)
        // may modify the matched place.
        let has_bindings = {
            let mut v = Vec::new();
            pat.bindings(&mut v);
            !v.is_empty()
        };
        let path = if (mutable && has_bindings) || self.has_ref_binding(pat) { place_path(scrutinee) } else { None };
        let pushed = match path {
            Some(path) => {
                self.frozen.push(Frozen { path, span: scrutinee.span, why: "pattern match", exclusive: mutable });
                true
            }
            None => false,
        };
        f(self);
        if pushed {
            self.frozen.pop();
        }
    }

    /// `method_call`: the first argument is the receiver of a method; a temporary
    /// receiver may be modified (it is destroyed after the statement).
    fn call_args(&mut self, args: &[CallArg], span: Span, method_call: bool) {
        for (i, a) in args.iter().enumerate() {
            self.expr(&a.expr);
            match a.conv {
                Convention::Owned => self.consume(&a.expr),
                Convention::Inout if i == 0 && method_call && !a.expr.is_place() => {}
                Convention::Inout => {
                    if !a.expr.is_place() {
                        if !matches!(a.expr.kind, T::Error) {
                            self.diags.emit(
                                Diagnostic::error("E0410", "an `inout` argument must be a variable, field or element")
                                    .primary(a.expr.span, "temporary value")
                                    .note("the function modifies this argument; a temporary would be lost"),
                            );
                        }
                    } else {
                        self.write(&a.expr, a.expr.span, "modify", true);
                    }
                }
                Convention::Borrow => {}
            }
        }
        // Law of exclusivity between the arguments of one call. An anonymous function
        // accesses the variables it captures during the call.
        // The last field tells whether the access is a capture of an anonymous function.
        let mut places: Vec<(usize, PlacePath, Convention, bool)> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            if a.conv != Convention::Owned {
                if let Some(p) = place_path(&a.expr) {
                    places.push((i, p, a.conv, false));
                }
            }
            if let T::Closure { captures, .. } = &a.expr.kind {
                for &(l, mutated) in captures {
                    places.push((i, PlacePath { root: l, projs: vec![] }, if mutated { Convention::Inout } else { Convention::Borrow }, true));
                }
            }
        }
        for (i, pi, ci, cap_i) in &places {
            if *ci != Convention::Inout {
                continue;
            }
            for (j, pj, cj, cap_j) in &places {
                if i == j || !overlaps(pi, pj) {
                    continue;
                }
                if *cj == Convention::Inout && j < i {
                    continue; // reported once for the pair
                }
                let root = self.local_name(pi.root);
                let (title, accessed, modified) = if *cap_i || *cap_j {
                    let by_fn = |what: &str| format!("{} by this anonymous function, which captures `{}`", what, root);
                    (
                        format!("`{}` is used twice by the same call, once to modify it", root),
                        if *cap_j { by_fn(if *cj == Convention::Inout { "modified" } else { "read" }) } else { "accessed here".to_string() },
                        if *cap_i { by_fn("modified") } else { "modified here (`inout`)".to_string() },
                    )
                } else {
                    (format!("{} is passed twice to the same call, once as `inout`", self.describe(&args[*i].expr)), "accessed here".to_string(), "modified here (`inout`)".to_string())
                };
                self.diags.emit(
                    Diagnostic::error("E0420", title)
                        .primary(args[*j].expr.span, accessed)
                        .secondary(args[*i].expr.span, modified)
                        .secondary(span, "during this call")
                        .note("a value cannot be modified while it is also read or modified elsewhere (law of exclusivity, spec §5.4)")
                        .help(if matches!(pi.projs.last(), Some(Proj::Index)) { "to exchange two elements, use `v.swap(i, j)`" } else { "copy one of the values into a variable first" }),
                );
            }
        }
    }

    /// A read of a place: forbidden while the place is borrowed for writing.
    fn read(&self, e: &Expr) {
        let Some(path) = place_path(e) else { return };
        for f in self.frozen.iter().rev() {
            if f.exclusive && overlaps(&f.path, &path) {
                let root = self.local_name(path.root);
                self.diags.emit(
                    Diagnostic::error("E0420", format!("cannot use {} while `{}` is borrowed for modification", self.describe(e), root))
                        .primary(e.span, "used here")
                        .secondary(f.span, format!("`{}` is modified through the bindings of this {} until its end", root, f.why))
                        .help("use the bindings instead, or finish the modification first")
                        .note("a value cannot be read while it is being modified elsewhere (law of exclusivity, spec §5.4)"),
                );
                return;
            }
        }
    }

    fn expr(&mut self, e: &Expr) {
        if e.is_place() {
            // Check the whole place once, then only the index expressions inside it.
            self.read(e);
            self.place_subexprs(e);
            return;
        }
        match &e.kind {
            T::Lit(_) | T::Local(_) | T::Const(_) | T::Static(_) | T::FnRef(_) | T::Error | T::Break(_) | T::Continue(_) => {}
            T::Unary(_, x) | T::Field(x, _) | T::Cast(x) | T::Try(x, _) => self.expr(x),
            T::ArrayRepeat(x, _) => {
                self.expr(x);
            }
            T::Move(x) => {
                if let T::Local(l) = &x.kind {
                    if self.moved_in_guard(*l, x) {
                        return;
                    }
                }
                self.write(x, e.span, "move", false);
            }
            T::Binary(_, a, b) | T::Index(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            T::IndexFn { base, index, .. } => {
                self.expr(base);
                self.expr(index);
            }
            T::Call { callee, args } => {
                if let Callee::Indirect(f) = callee {
                    self.expr(f);
                }
                let method_call = matches!(callee, Callee::Fn(f, _) if self.p.func(*f).has_self);
                self.call_args(args, e.span, method_call);
            }
            T::Adt { fields: xs, .. } | T::Tuple(xs) | T::Array(xs) => {
                for x in xs {
                    self.expr(x);
                    self.consume(x);
                }
            }
            T::If { cond, then, else_ } => {
                self.expr(cond);
                self.block(then);
                if let Some(x) = else_ {
                    self.expr(x);
                }
            }
            T::IfLet { pat, scrutinee, then, else_, mutable } => {
                self.expr(scrutinee);
                if *mutable {
                    self.write(scrutinee, scrutinee.span, "modify", true);
                } else if !scrutinee.is_place() {
                    self.consume(scrutinee);
                    self.check_destructure(pat);
                }
                self.with_frozen(scrutinee, pat, *mutable, |cx| cx.block(then));
                if let Some(x) = else_ {
                    self.expr(x);
                }
            }
            T::Match { scrutinee, arms, mutable } => {
                self.expr(scrutinee);
                if *mutable {
                    self.write(scrutinee, scrutinee.span, "modify", true);
                }
                if !scrutinee.is_place() {
                    for a in arms {
                        self.check_destructure(&a.pat);
                    }
                }
                for a in arms {
                    // A guard runs before the next arms are tested through the same
                    // place: it must never modify the matched value.
                    if let Some(g) = &a.guard {
                        let pushed = match place_path(scrutinee) {
                            Some(path) => {
                                self.frozen.push(Frozen { path, span: scrutinee.span, why: "pattern match", exclusive: *mutable });
                                true
                            }
                            None => false,
                        };
                        let mark = self.guard_bindings.len();
                        a.pat.bindings(&mut self.guard_bindings);
                        self.expr(g);
                        self.guard_bindings.truncate(mark);
                        if pushed {
                            self.frozen.pop();
                        }
                    }
                    self.with_frozen(scrutinee, &a.pat, *mutable, |cx| cx.expr(&a.body));
                }
            }
            T::Block(b) => self.block(b),
            T::Return(x) => {
                if let Some(x) = x {
                    self.expr(x);
                    self.consume(x);
                }
            }
            T::Interp(parts) => {
                for p in parts {
                    if let thir::InterpPart::Value(x, _) = p {
                        self.expr(x);
                    }
                }
            }
            T::Closure { captures, .. } => {
                for &(l, mutated) in captures {
                    let x = Expr { kind: T::Local(l), ty: self.body.locals[l as usize].ty, span: e.span };
                    if mutated {
                        self.write(&x, e.span, "modify", false);
                    } else {
                        self.read(&x);
                    }
                }
            }
            T::FnToClosure(x) => self.expr(x),
            T::ToDyn(x) => {
                self.expr(x);
                self.consume(x);
            }
        }
    }
}

#[allow(dead_code)]
fn pat_is_binding_free(p: &Pat) -> bool {
    !matches!(p.kind, PatKind::Binding(_))
}
