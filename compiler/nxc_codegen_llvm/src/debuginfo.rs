//! Debug information (DWARF or CodeView) as LLVM metadata: source files, functions,
//! line tables, variables and their types. Debuggers (lldb, gdb, Visual Studio) then show
//! the NEXUL source and the values of the variables.

use crate::layout::LayoutCx;
use nxc_sema::defs::{AdtKind, Program};
use nxc_sema::ty::{FloatTy, IntTy, Ty, TyKind};
use std::collections::HashMap;
use std::fmt::Write;

pub struct DebugInfo {
    next: usize,
    /// Metadata definitions, `!N = ...`.
    pub defs: String,
    pub cu: usize,
    files: HashMap<String, usize>,
    types: HashMap<Ty, usize>,
    locations: HashMap<(usize, u32, u32), usize>,
    empty_tuple: usize,
    sub_type: usize,
    pub flags: Vec<usize>,
}

fn escape(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b == b'"' || b == b'\\' || !(0x20..0x7f).contains(&b) {
            let _ = write!(o, "\\{:02X}", b);
        } else {
            o.push(b as char);
        }
    }
    o
}

impl DebugInfo {
    pub fn new(main_file: &str, directory: &str, codeview: bool, optimized: bool) -> DebugInfo {
        let mut d = DebugInfo {
            next: 0,
            defs: String::new(),
            cu: 0,
            files: HashMap::new(),
            types: HashMap::new(),
            locations: HashMap::new(),
            empty_tuple: 0,
            sub_type: 0,
            flags: Vec::new(),
        };
        let file = d.file_with_dir(main_file, directory);
        d.empty_tuple = d.add("!{}".to_string());
        let cu = d.reserve();
        d.define(
            cu,
            format!(
                "distinct !DICompileUnit(language: DW_LANG_C99, file: !{}, producer: \"nxc {} (NEXUL)\", isOptimized: {}, runtimeVersion: 0, emissionKind: FullDebug, enums: !{})",
                file,
                env!("CARGO_PKG_VERSION"),
                optimized,
                d.empty_tuple
            ),
        );
        d.cu = cu;
        d.sub_type = d.add(format!("!DISubroutineType(types: !{})", d.empty_tuple));
        let v = d.add("!{i32 2, !\"Debug Info Version\", i32 3}".into());
        d.flags.push(v);
        if codeview {
            let c = d.add("!{i32 2, !\"CodeView\", i32 1}".into());
            d.flags.push(c);
        } else {
            let c = d.add("!{i32 7, !\"Dwarf Version\", i32 5}".into());
            d.flags.push(c);
        }
        d
    }

    fn reserve(&mut self) -> usize {
        let n = self.next;
        self.next += 1;
        n
    }

    fn define(&mut self, id: usize, body: String) {
        let _ = writeln!(self.defs, "!{} = {}", id, body);
    }

    fn add(&mut self, body: String) -> usize {
        let id = self.reserve();
        self.define(id, body);
        id
    }

    fn file_with_dir(&mut self, name: &str, dir: &str) -> usize {
        let key = format!("{}|{}", dir, name);
        if let Some(&f) = self.files.get(&key) {
            return f;
        }
        let f = self.add(format!("!DIFile(filename: \"{}\", directory: \"{}\")", escape(name), escape(dir)));
        self.files.insert(key, f);
        f
    }

    /// The metadata of a source file, by its name in the source map. Paths are relative to
    /// the directory of the program.
    pub fn file(&mut self, name: &str, root_dir: &str) -> usize {
        self.file_with_dir(name, root_dir)
    }

    pub fn subprogram(&mut self, name: &str, linkage: &str, file: usize, line: u32, optimized: bool) -> usize {
        let flags = if optimized { "DISPFlagDefinition | DISPFlagOptimized" } else { "DISPFlagDefinition" };
        self.add(format!(
            "distinct !DISubprogram(name: \"{}\", linkageName: \"{}\", scope: !{}, file: !{}, line: {}, type: !{}, scopeLine: {}, spFlags: {}, unit: !{}, retainedNodes: !{})",
            escape(name),
            escape(linkage),
            file,
            file,
            line,
            self.sub_type,
            line,
            flags,
            self.cu,
            self.empty_tuple
        ))
    }

    pub fn location(&mut self, scope: usize, line: u32, col: u32) -> usize {
        if let Some(&l) = self.locations.get(&(scope, line, col)) {
            return l;
        }
        let l = self.add(format!("!DILocation(line: {}, column: {}, scope: !{})", line, col, scope));
        self.locations.insert((scope, line, col), l);
        l
    }

    pub fn local_variable(&mut self, name: &str, arg: Option<usize>, scope: usize, file: usize, line: u32, ty: usize) -> usize {
        let arg = arg.map(|a| format!(", arg: {}", a)).unwrap_or_default();
        self.add(format!("!DILocalVariable(name: \"{}\"{}, scope: !{}, file: !{}, line: {}, type: !{})", escape(name), arg, scope, file, line, ty))
    }

    pub fn global_variable(&mut self, name: &str, file: usize, line: u32, ty: usize) -> usize {
        let v = self.add(format!(
            "distinct !DIGlobalVariable(name: \"{}\", scope: !{}, file: !{}, line: {}, type: !{}, isLocal: true, isDefinition: true)",
            escape(name),
            self.cu,
            file,
            line,
            ty
        ));
        self.add(format!("!DIGlobalVariableExpression(var: !{}, expr: !DIExpression())", v))
    }

    fn basic(&mut self, name: &str, bits: u64, encoding: &str) -> usize {
        self.add(format!("!DIBasicType(name: \"{}\", size: {}, encoding: {})", name, bits, encoding))
    }

    /// The debug type of a NEXUL type.
    pub fn ty(&mut self, lx: &LayoutCx, p: &Program, t: Ty) -> usize {
        if let Some(&d) = self.types.get(&t) {
            return d;
        }
        let tcx = &p.tcx;
        let name = p.ty_str(t);
        let d = match &*tcx.kind(t) {
            TyKind::Int(i) => {
                let bits = lx.int_bytes(*i) * 8;
                let enc = if i.signed() { "DW_ATE_signed" } else { "DW_ATE_unsigned" };
                let enc = if *i == IntTy::U8 { "DW_ATE_unsigned_char" } else { enc };
                self.basic(&name, bits, enc)
            }
            TyKind::Float(f) => self.basic(&name, if *f == FloatTy::F32 { 32 } else { 64 }, "DW_ATE_float"),
            TyKind::Bool => self.basic("bool", 8, "DW_ATE_boolean"),
            TyKind::Char => self.basic("char", 32, "DW_ATE_UTF"),
            TyKind::Unit | TyKind::Never | TyKind::Error => {
                let id = self.reserve();
                self.types.insert(t, id);
                self.define(id, format!("!DICompositeType(tag: DW_TAG_structure_type, name: \"()\", size: 0, elements: !{})", self.empty_tuple));
                id
            }
            TyKind::Ptr(e) => {
                let id = self.reserve();
                self.types.insert(t, id);
                let inner = self.ty(lx, p, *e);
                self.define(id, format!("!DIDerivedType(tag: DW_TAG_pointer_type, name: \"{}\", baseType: !{}, size: {})", escape(&name), inner, lx.ptr_bytes * 8));
                return id;
            }
            TyKind::Fn(..) => {
                let st = self.sub_type;
                self.add(format!("!DIDerivedType(tag: DW_TAG_pointer_type, name: \"{}\", baseType: !{}, size: {})", escape(&name), st, lx.ptr_bytes * 8))
            }
            TyKind::Array(e, n) => {
                let id = self.reserve();
                self.types.insert(t, id);
                let inner = self.ty(lx, p, *e);
                let range = self.add(format!("!DISubrange(count: {})", n));
                let elems = self.add(format!("!{{!{}}}", range));
                let size = lx.layout(t).size * 8;
                self.define(id, format!("!DICompositeType(tag: DW_TAG_array_type, baseType: !{}, size: {}, elements: !{})", inner, size, elems));
                return id;
            }
            TyKind::Closure(..) | TyKind::Dyn(_) => {
                let id = self.reserve();
                self.types.insert(t, id);
                let names: Vec<String> = if matches!(&*tcx.kind(t), TyKind::Dyn(_)) { vec!["data".into(), "vtable".into()] } else { vec!["code".into(), "env".into()] };
                let pu = tcx.ptr(tcx.unit);
                self.composite(lx, p, id, &name, t, &names, &[pu, pu]);
                return id;
            }
            TyKind::Tuple(elems) => {
                let id = self.reserve();
                self.types.insert(t, id);
                let names: Vec<String> = (0..elems.len()).map(|i| format!("{}", i)).collect();
                let tys = elems.clone();
                self.composite(lx, p, id, &name, t, &names, &tys);
                return id;
            }
            TyKind::Adt(aid, args) => {
                let id = self.reserve();
                self.types.insert(t, id);
                let adt = p.adt(*aid);
                match &adt.kind {
                    AdtKind::Struct { fields } => {
                        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
                        let tys: Vec<Ty> = fields.iter().map(|f| if f.indirect { tcx.ptr(tcx.subst(f.ty, args)) } else { tcx.subst(f.ty, args) }).collect();
                        self.composite(lx, p, id, &name, t, &names, &tys);
                    }
                    AdtKind::Enum { .. } => {
                        // The tag, then the raw payload bytes.
                        let shape = lx.enum_shape(t);
                        let tag = self.basic("tag", 32, "DW_ATE_unsigned");
                        let mut members = vec![self.add(format!("!DIDerivedType(tag: DW_TAG_member, name: \"tag\", baseType: !{}, size: 32, offset: 0)", tag))];
                        if shape.has_payload {
                            let byte = self.basic("u8", 8, "DW_ATE_unsigned_char");
                            let range = self.add(format!("!DISubrange(count: {})", shape.payload_size));
                            let elems = self.add(format!("!{{!{}}}", range));
                            let arr = self.add(format!("!DICompositeType(tag: DW_TAG_array_type, baseType: !{}, size: {}, elements: !{})", byte, shape.payload_size * 8, elems));
                            members.push(self.add(format!(
                                "!DIDerivedType(tag: DW_TAG_member, name: \"payload\", baseType: !{}, size: {}, offset: {})",
                                arr,
                                shape.payload_size * 8,
                                shape.payload_offset * 8
                            )));
                        }
                        let list = self.add(format!("!{{{}}}", members.iter().map(|m| format!("!{}", m)).collect::<Vec<_>>().join(", ")));
                        let l = lx.layout(t);
                        self.define(id, format!("!DICompositeType(tag: DW_TAG_structure_type, name: \"{}\", size: {}, align: {}, elements: !{})", escape(&name), l.size * 8, l.align * 8, list));
                    }
                }
                return id;
            }
            _ => self.basic("?", 8, "DW_ATE_unsigned"),
        };
        self.types.insert(t, d);
        d
    }

    #[allow(clippy::too_many_arguments)]
    fn composite(&mut self, lx: &LayoutCx, p: &Program, id: usize, name: &str, t: Ty, names: &[String], tys: &[Ty]) {
        let offsets = lx.struct_offsets(tys);
        let mut members = Vec::new();
        for (i, &ft) in tys.iter().enumerate() {
            let fd = self.ty(lx, p, ft);
            let size = lx.layout(ft).size * 8;
            members.push(self.add(format!(
                "!DIDerivedType(tag: DW_TAG_member, name: \"{}\", baseType: !{}, size: {}, offset: {})",
                escape(&names[i]),
                fd,
                size,
                offsets[i] * 8
            )));
        }
        let list = self.add(format!("!{{{}}}", members.iter().map(|m| format!("!{}", m)).collect::<Vec<_>>().join(", ")));
        let l = lx.layout(t);
        self.define(id, format!("!DICompositeType(tag: DW_TAG_structure_type, name: \"{}\", size: {}, align: {}, elements: !{})", escape(name), l.size * 8, l.align * 8, list));
    }

    /// The `!llvm.dbg.cu` and `!llvm.module.flags` lines.
    pub fn module_tail(&self, extra_flags: &[usize]) -> String {
        let mut flags: Vec<String> = self.flags.iter().map(|f| format!("!{}", f)).collect();
        flags.extend(extra_flags.iter().map(|f| format!("!{}", f)));
        format!("!llvm.dbg.cu = !{{!{}}}\n!llvm.module.flags = !{{{}}}\n", self.cu, flags.join(", "))
    }
}
