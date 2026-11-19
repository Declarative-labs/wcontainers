//! Walks one parsed Rust file and records every literal, with the item it sits in, and every constant
//! definition, with its initialiser.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::ToTokens;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, BinOp, Block, Expr, ExprCall, ExprMethodCall, Item, Lit, Member, Meta, Pat, Stmt,
    Token, Type, UnOp, UseTree,
};

use crate::RUST_EXTENSION;
use crate::eval::{CExpr, ConstDef, ConstScope, IntType, Value};
use crate::findings::{
    FILE_SCOPE, Finding, Kind, List, PATH_COMPONENT_SEPARATOR, PATH_SEPARATOR, clean_literal,
};
use crate::text::message_text;

/// Numbers that are not magic where they are plain arithmetic: nothing, the first, the next, the
/// previous, half and double.
pub const FREE_NUMBERS: [i128; 4] = [-1, 0, 1, 2];

/// Macros whose string arguments, from the first one on, are messages (format strings, panic text,
/// error text). A message string is still reported when it holds a number or a path.
const MESSAGE_MACROS: &[&str] = &[
    "format",
    "format_args",
    "print",
    "println",
    "eprint",
    "eprintln",
    "panic",
    "unreachable",
    "todo",
    "unimplemented",
    "compile_error",
    "anyhow",
    "bail",
    "eyre",
];
/// Macros whose first argument is a condition and the rest a message.
const CONDITION_MACROS: &[&str] = &["assert", "debug_assert", "ensure"];
/// Macros whose first two arguments are compared and the rest a message.
const COMPARISON_MACROS: &[&str] = &[
    "assert_eq",
    "assert_ne",
    "debug_assert_eq",
    "debug_assert_ne",
];
/// How many leading arguments of a condition or comparison macro are operands, not message.
const CONDITION_OPERANDS: usize = 1;
const COMPARISON_OPERANDS: usize = 2;
/// `write!` and `writeln!` write a message only inside a `Display`-style function, which takes a
/// [`FORMATTER_TYPE`]; anywhere else what they write is data (a /proc file, a sysctl answer).
const WRITE_MACROS: &[&str] = &["write", "writeln"];
const FORMATTER_TYPE: &str = "Formatter";
/// The crates whose prelude macros are named like these; a bare `error!` counts as a log macro only
/// when the file imports it from one of these crates.
const STANDARD_CRATES: &[&str] = &["std", "core", "alloc"];
const LOG_CRATES: &[&str] = &["log", "tracing"];
const LOG_MACROS: &[&str] = &["trace", "debug", "info", "warn", "error", "log", "event"];
/// `log::info!(target: "name", ...)`: the target is a value, not a message.
const LOG_TARGET: &str = "target";
/// `#[macro_use] extern crate log;` imports every log macro.
const MACRO_USE: &str = "macro_use";
/// Methods whose string argument is an error message.
const MESSAGE_METHODS: &[&str] = &["expect", "expect_err", "context", "wrap_err", "ok_or"];
/// Methods whose closure argument returns an error message (`map_err(|_| "too large")`).
const MESSAGE_CLOSURE_METHODS: &[&str] =
    &["map_err", "ok_or_else", "with_context", "wrap_err_with"];
/// Methods that compare their receiver with their argument (`uid.eq(&0)`), which so goes to the
/// receiver's name.
const COMPARISON_METHODS: &[&str] = &["eq", "ne", "lt", "le", "gt", "ge", "cmp", "partial_cmp"];
/// Joins string literals into one string literal (`concat!("HO", "ME")`).
const CONCAT_MACRO: &str = "concat";
/// `Err("...")` carries a message.
const ERR_CONSTRUCTOR: &str = "Err";
/// `...Error::new(.., "...")`, `...Error::other("...")` and `...Error::Variant("a sentence")` carry a
/// message; an error variant's string without a space is a key (a feature name) and is reported.
const ERROR_TYPE_SUFFIX: &str = "Error";
const ERROR_CONSTRUCTORS: &[&str] = &["new", "other"];
/// Macros that take a configuration predicate, which is attribute-like and skipped as attributes are.
const ATTRIBUTE_LIKE_MACROS: &[&str] = &["cfg"];
/// Attributes whose content is text or a configuration predicate, never a value of the program.
const SKIPPED_ATTRIBUTES: &[&str] = &["doc", "cfg"];
/// `#[cfg_attr(predicate, attributes...)]`: the attributes after the predicate are read like any other.
const CFG_ATTR: &str = "cfg_attr";
/// Turns its tokens into a string literal.
const STRINGIFY_MACRO: &str = "stringify";
/// Wrap one value without changing what it is given to (`chown(p, Some(0), Some(0))`).
const WRAPPERS: &[&str] = &["Some", "Ok"];
/// Macros whose body defines named constants (flag values).
const CONST_DEFINING_MACROS: &[&str] = &["bitflags"];
const MATCHES_MACRO: &str = "matches";
const MACRO_RULES: &str = "macro_rules";
/// Between `macro_rules` and the macro's name in an item path (`macro_rules! name`).
const MACRO_ITEM_SEPARATOR: &str = "! ";
/// Around a trait implementation's type and trait in an item path (`<Type as Trait>`).
const TRAIT_IMPL_OPEN: &str = "<";
const TRAIT_IMPL_AS: &str = " as ";
const TRAIT_IMPL_CLOSE: &str = ">";
/// `include!("file")` pulls Rust source into the file; it is scanned where it is included.
const INCLUDE_MACRO: &str = "include";
/// Macros that read a variable of the build's environment by a name written as a literal.
const ENVIRONMENT_MACROS: &[&str] = &["env", "option_env"];
/// The variables Cargo sets for a build: their names are Cargo's, and the macros take only literals.
const CARGO_VARIABLE_PREFIX: &str = "CARGO_";
const CARGO_OUT_DIR: &str = "OUT_DIR";
/// Method calls that are arithmetic, so 0 and 1 as their argument are plain arithmetic too.
const ARITHMETIC_METHOD_PREFIXES: &[&str] = &[
    "wrapping_",
    "checked_",
    "saturating_",
    "overflowing_",
    "unchecked_",
    "strict_",
    "fetch_",
];
/// Method calls that index or count, where 0 and 1 mean the first or the next.
const COUNTING_METHODS: &[&str] = &[
    "get",
    "get_mut",
    "get_unchecked",
    "nth",
    "skip",
    "take",
    "add",
    "sub",
    "offset",
    "min",
    "max",
];
/// Words that, in the name next to a free number, say the value is an id, a descriptor or a status.
const ROLE_WORDS: &[&str] = &[
    "uid", "gid", "euid", "egid", "suid", "sgid", "fsuid", "fsgid", "fd", "pid", "tid", "ppid",
    "pgid", "sid", "exit", "status", "stdin", "stdout", "stderr", "errno", "signal", "signo",
    "user", "owner", "chown", "lchown", "fchown", "fchownat", "code",
];
/// Written before a negated literal.
const NEGATIVE_SIGN: char = '-';
/// The C library's one-word accessors of an id (`getuid`, `setsid`, `getppid`).
const ROLE_ACCESSOR_PREFIXES: &[&str] = &["get", "set"];
/// Words that make a name a quantity or a set of ids rather than one id (`pid_count`, `signal_mask`).
const QUANTITY_WORDS: &[&str] = &[
    "count", "counter", "mask", "len", "length", "total", "size", "number",
];
/// The plural ending of a word in a name (`fds`).
const PLURAL_SUFFIX: char = 's';
/// Joins the names of a pattern's parts in a role hint.
const HINT_SEPARATOR: &str = " ";
/// Attributes that mark a test, a benchmark, or code compiled only for tests.
const TEST_ATTRIBUTES: &[&str] = &["test", "bench"];
const CFG_ATTRIBUTE: &str = "cfg";
const TEST_CFG: &str = "test";
const CFG_ALL: &str = "all";
const CFG_ANY: &str = "any";
/// Separates the predicates of `all(...)` and `any(...)`.
const PREDICATE_SEPARATOR: char = ',';
/// The name of an anonymous constant (`const _: () = ...;`), which defines nothing.
const UNNAMED: &str = "_";
const UNNAMED_SCOPE: &str = "const _";
/// Token-level markers of an attribute (`#[...]`, `#![...]`) inside a macro body.
const ATTRIBUTE_SIGIL: char = '#';
const INNER_ATTRIBUTE_MARK: char = '!';
/// Token-level shape of a constant definition inside a macro body: `const NAME = expr;`, and the
/// `struct NAME` that owns the flags of a `bitflags!` body.
const CONST_KEYWORD: &str = "const";
const STRUCT_KEYWORD: &str = "struct";
const ASSIGN: char = '=';
const TERMINATOR: char = ';';

/// What a file is, which decides the list its literals go to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileRole {
    Code,
    Test,
    Generated,
}

#[derive(Clone, Copy, PartialEq)]
enum TokenMode {
    /// Report every literal.
    All,
    /// As `All`, but strings are messages, reported only when they hold a value.
    Message,
}

/// Where a constant can be seen from: an implementation of a trait's constant, and other crates.
struct Exposure {
    trait_impl: bool,
    public: bool,
}

impl Exposure {
    /// A constant of a macro or an enum's discriminant: on its own, and taken as public.
    const PLAIN: Exposure = Exposure {
        trait_impl: false,
        public: true,
    };
}

struct ImplScope {
    self_name: String,
    trait_impl: bool,
}

pub struct FileScan {
    path: String,
    crate_dir: String,
    role: FileRole,
    reference: Option<String>,
    /// The folder the file is in, for `include!`.
    folder: PathBuf,
    scope: Vec<String>,
    impls: Vec<ImplScope>,
    enums: Vec<String>,
    const_depth: usize,
    test_depth: usize,
    formatter_depth: usize,
    /// Private modules around the scan (the file's own, when its declaration is private): other crates
    /// cannot import what is inside one.
    hidden_depth: usize,
    /// Log macros the file imports by their bare name.
    log_macros: Vec<String>,
    /// The name a match arm's pattern is compared with, for 0 and 1 in the pattern.
    pattern_hint: Option<String>,
    /// The function whose value a `return` gives.
    return_hint: Option<String>,
    /// The function whose value the next block's last expression gives.
    value_hint: Option<String>,
    /// The parameter names of the functions and closures the file defines, by name.
    params: HashMap<String, Vec<Option<String>>>,
    /// In the function being scanned, what each local name is given to (`exit` for `exit(code)`).
    uses: HashMap<String, String>,
    included: Vec<PathBuf>,
    pub findings: Vec<Finding>,
    pub consts: Vec<ConstDef>,
}

impl FileScan {
    pub fn new(
        path: String,
        crate_dir: String,
        role: FileRole,
        reference: Option<String>,
        folder: PathBuf,
        exposed: bool,
    ) -> FileScan {
        FileScan {
            path,
            crate_dir,
            role,
            reference,
            folder,
            scope: Vec::new(),
            impls: Vec::new(),
            enums: Vec::new(),
            const_depth: usize::default(),
            test_depth: usize::default(),
            formatter_depth: usize::default(),
            hidden_depth: usize::from(!exposed),
            log_macros: Vec::new(),
            pattern_hint: None,
            return_hint: None,
            value_hint: None,
            params: HashMap::new(),
            uses: HashMap::new(),
            included: Vec::new(),
            findings: Vec::new(),
            consts: Vec::new(),
        }
    }

    /// Scans a whole file.
    pub fn scan_file(&mut self, ast: &syn::File) {
        collect_log_imports(&ast.items, &mut self.log_macros);
        let mut signatures = Signatures::default();
        signatures.visit_file(ast);
        self.params = signatures.0;
        let test = ast.attrs.iter().any(is_test_attribute);
        self.test_depth += usize::from(test);
        self.visit_file(ast);
        self.test_depth -= usize::from(test);
    }

    fn item(&self) -> String {
        if self.scope.is_empty() {
            FILE_SCOPE.to_string()
        } else {
            self.scope.join(PATH_SEPARATOR)
        }
    }

    fn in_test(&self) -> bool {
        self.role == FileRole::Test || self.test_depth > usize::default()
    }

    fn list(&self) -> List {
        if self.role == FileRole::Generated {
            List::Generated
        } else if self.in_test() {
            List::Test
        } else if self.const_depth > usize::default() {
            List::Const
        } else {
            List::Code
        }
    }

    /// Records a literal. `hint` is the name a free number is given to or compared with, if any.
    fn report(&mut self, lit: &Lit, negative: bool, hint: Option<&str>) {
        self.record(lit, negative, hint, false);
    }

    /// Records a literal; `message` says it is text shown to a person that holds no value.
    fn record(&mut self, lit: &Lit, negative: bool, hint: Option<&str>, message: bool) {
        if self.reference.is_some() {
            return;
        }
        let Some(kind) = kind_of(lit) else { return };
        if is_empty_text(lit) {
            return;
        }
        let list = match self.list() {
            List::Code | List::Test if message => List::Message,
            List::Code if is_free(lit, negative) => {
                if hint.is_some_and(names_role) {
                    List::Role
                } else {
                    List::Free
                }
            }
            list => list,
        };
        let start = lit.span().start();
        let text = lit.to_token_stream().to_string();
        let literal = clean_literal(&if negative {
            format!("{NEGATIVE_SIGN}{text}")
        } else {
            text
        });
        let note = match (list, hint) {
            (List::Role, Some(h)) => format!("given to {h}"),
            _ => String::new(),
        };
        self.findings.push(Finding {
            list,
            path: self.path.clone(),
            line: start.line,
            column: start.column + 1,
            kind,
            item: self.item(),
            literal,
            value: Value::of_lit(lit, negative),
            note,
            crate_dir: self.crate_dir.clone(),
            in_const: self.const_depth > usize::default(),
        });
    }

    /// An operand of arithmetic, an end of a range, an element count: free numbers are skipped.
    fn arithmetic(&mut self, e: &Expr) {
        if free_lit(e, false).is_none() {
            self.visit_expr(e);
        }
    }

    /// An expression given to a name (an argument, a field, a variable, the other side of a
    /// comparison): a free number is reported with that name, anything else is visited.
    fn given(&mut self, e: &Expr, hint: Option<String>) {
        if let Some((lit, negative)) = free_lit(e, false) {
            return self.report(lit, negative, hint.as_deref());
        }
        let plain = expr_attrs(e).is_empty() && self.const_depth == usize::default();
        match e {
            // A wrapped value, and a value chosen by a branch, go to the same name.
            Expr::Call(c) if plain && c.args.len() == 1 && is_wrapper(&c.func) => {
                self.visit_expr(&c.func);
                c.args.iter().for_each(|a| self.given(a, hint.clone()));
            }
            Expr::If(i) if plain => {
                self.visit_expr(&i.cond);
                self.value_hint = hint.clone();
                self.visit_block(&i.then_branch);
                self.value_hint = None;
                if let Some((_, other)) = &i.else_branch {
                    self.given(other, hint);
                }
            }
            Expr::Block(b) if plain && b.label.is_none() => {
                self.value_hint = hint;
                self.visit_block(&b.block);
                self.value_hint = None;
            }
            Expr::Match(m) if plain => self.match_expr(m, hint),
            // Every element of an array goes to the name the array goes to (`let fds = [0, 1]`).
            Expr::Array(a) if plain => a.elems.iter().for_each(|x| self.given(x, hint.clone())),
            Expr::Reference(r) if plain => self.given(&r.expr, hint),
            _ => self.visit_expr(e),
        }
    }

    /// The name an argument of a call goes to: the function's, and the parameter's when the file
    /// defines the function or closure.
    fn argument_hint(&self, callee: Option<String>, index: usize) -> Option<String> {
        let parameter = callee
            .as_deref()
            .and_then(|c| c.rsplit(HINT_SEPARATOR).next())
            .and_then(|name| self.params.get(name))
            .and_then(|p| p.get(index).cloned().flatten());
        match (callee, parameter) {
            (Some(c), Some(p)) => Some([c, p].join(HINT_SEPARATOR)),
            (c, p) => c.or(p),
        }
    }

    fn match_expr(&mut self, m: &syn::ExprMatch, body_hint: Option<String>) {
        self.visit_expr(&m.expr);
        let hint = hint_of(&m.expr);
        for arm in &m.arms {
            self.with_test(&arm.attrs, |s| {
                s.pattern_hint = hint.clone();
                s.visit_pat(&arm.pat);
                s.pattern_hint = None;
                if let Some((_, g)) = &arm.guard {
                    s.visit_expr(g);
                }
                s.given(&arm.body, body_hint.clone());
            });
        }
    }

    /// A value bound to a pattern: a tuple goes element by element (`let (uid, gid) = (0, 0)`).
    fn bound(&mut self, pat: &Pat, e: &Expr) {
        match (pat, e) {
            (Pat::Type(t), _) => self.bound(&t.pat, e),
            (Pat::Tuple(p), Expr::Tuple(t)) if p.elems.len() == t.elems.len() => {
                p.elems
                    .iter()
                    .zip(&t.elems)
                    .for_each(|(p, e)| self.bound(p, e));
            }
            _ => {
                // A local name also goes to what it is later given to (`exit(code)`).
                let hint = hint_of_pat(pat).map(|name| match self.uses.get(&name) {
                    Some(used) => [name, used.clone()].join(HINT_SEPARATOR),
                    None => name,
                });
                self.given(e, hint)
            }
        }
    }

    /// A string in a message position: gated when it holds a number or a path, when it is data (no
    /// space and not one plain word), or (found later) when it writes out a named constant's value.
    fn message(&mut self, e: &Expr) {
        match e {
            Expr::Lit(l) => match &l.lit {
                Lit::Str(s) => self.record(&l.lit, false, None, message_text(&s.value())),
                lit => self.report(lit, false, None),
            },
            _ => self.visit_expr(e),
        }
    }

    fn with_scope(&mut self, name: String, f: impl FnOnce(&mut Self)) {
        self.scope.push(name);
        f(self);
        self.scope.pop();
    }

    fn with_test(&mut self, attrs: &[Attribute], f: impl FnOnce(&mut Self)) {
        let test = attrs.iter().any(is_test_attribute);
        self.test_depth += usize::from(test);
        f(self);
        self.test_depth -= usize::from(test);
    }

    /// Code that sits inside a constant's initialiser but is not part of the value being defined (an
    /// item, a closure, a loop, a statement): its literals belong to the code list.
    fn outside_const(&mut self, f: impl FnOnce(&mut Self)) {
        let depth = std::mem::take(&mut self.const_depth);
        f(self);
        self.const_depth = depth;
    }

    fn record_const(
        &mut self,
        bare: String,
        qualified: String,
        ty: Option<&Type>,
        expr: Option<&Expr>,
        exposure: Exposure,
        at: proc_macro2::Span,
    ) {
        let Some(expr) = expr else { return };
        let scope = match (&self.reference, self.role, self.in_test()) {
            (Some(module), ..) => ConstScope::Reference(module.clone()),
            (None, FileRole::Generated, _) => ConstScope::Generated,
            (None, _, true) => ConstScope::Test,
            (None, _, false) => ConstScope::Code,
        };
        let start = at.start();
        self.consts.push(ConstDef {
            bare,
            qualified,
            path: self.path.clone(),
            crate_dir: self.crate_dir.clone(),
            line: start.line,
            column: start.column + 1,
            item: self.item(),
            expr: CExpr::lower(expr),
            ty: ty.and_then(int_type_of),
            scope,
            trait_impl: exposure.trait_impl,
            public: exposure.public && self.hidden_depth == usize::default(),
            value: None,
        });
    }

    fn const_item(
        &mut self,
        ident: &syn::Ident,
        ty: &Type,
        expr: Option<&Expr>,
        public: bool,
        visit_rest: impl FnOnce(&mut Self),
    ) {
        let bare = ident.to_string();
        if bare == UNNAMED {
            // `const _: () = { ... };` names nothing: what it holds is code (an assertion, an impl).
            self.with_scope(UNNAMED_SCOPE.to_string(), |s| s.outside_const(visit_rest));
            return;
        }
        let (qualified, trait_impl) = match self.impls.last() {
            Some(i) => (
                format!("{}{PATH_SEPARATOR}{bare}", i.self_name),
                i.trait_impl,
            ),
            None => (bare.clone(), false),
        };
        self.with_scope(bare.clone(), |s| {
            s.record_const(
                bare,
                qualified,
                Some(ty),
                expr,
                Exposure { trait_impl, public },
                ident.span(),
            );
            s.const_depth += 1;
            visit_rest(s);
            s.const_depth -= 1;
        });
    }

    fn method_call(&mut self, m: &ExprMethodCall) {
        let method = m.method.to_string();
        let arithmetic = ARITHMETIC_METHOD_PREFIXES
            .iter()
            .any(|p| method.starts_with(p));
        let counting = arithmetic || COUNTING_METHODS.contains(&method.as_str());
        // The receiver of arithmetic written as a method (`1u64.wrapping_shl(n)`) is an operand too.
        if arithmetic {
            self.arithmetic(&m.receiver)
        } else {
            self.visit_expr(&m.receiver)
        }
        if let Some(turbofish) = &m.turbofish {
            self.visit_angle_bracketed_generic_arguments(turbofish);
        }
        let message = MESSAGE_METHODS.contains(&method.as_str());
        let closure_message = MESSAGE_CLOSURE_METHODS.contains(&method.as_str());
        let hint = match COMPARISON_METHODS.contains(&method.as_str()) {
            true => hint_of(&m.receiver),
            false => Some(method.clone()),
        };
        for arg in &m.args {
            if message && is_str_lit(arg) {
                self.message(arg);
            } else if let (true, Expr::Closure(c)) = (closure_message, arg)
                && is_str_lit(unwrapped(&c.body))
            {
                self.visit_pat_list(c);
                self.message(unwrapped(&c.body));
            } else if counting {
                self.arithmetic(arg);
            } else {
                self.given(arg, hint.clone());
            }
        }
    }

    /// The parameters of a closure whose body is read apart.
    fn visit_pat_list(&mut self, c: &syn::ExprClosure) {
        c.inputs.iter().for_each(|p| self.visit_pat(p));
    }

    fn call(&mut self, c: &ExprCall) {
        self.visit_expr(&c.func);
        // A message constructor; for an error variant only a sentence (text with a space) is a message.
        let (message, sentence_only) = match &*c.func {
            Expr::Path(p) => {
                let names: Vec<String> = p
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                match names.as_slice() {
                    [.., only] if only == ERR_CONSTRUCTOR => (true, false),
                    [.., ty, ctor] if ty.ends_with(ERROR_TYPE_SUFFIX) => {
                        (true, !ERROR_CONSTRUCTORS.contains(&ctor.as_str()))
                    }
                    _ => (false, false),
                }
            }
            _ => (false, false),
        };
        let hint = hint_of(&c.func);
        for (i, arg) in c.args.iter().enumerate() {
            let sentence = matches!(arg, Expr::Lit(l) if matches!(&l.lit, Lit::Str(s) if s.value().contains(char::is_whitespace)));
            if message && is_str_lit(arg) && (sentence || !sentence_only) {
                self.message(arg);
            } else {
                self.given(arg, self.argument_hint(hint.clone(), i));
            }
        }
    }

    /// Where a macro's message arguments start, when it is a message macro.
    fn message_start(&self, mac: &syn::Macro) -> Option<usize> {
        let names: Vec<String> = mac
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        let (first, name) = (names.first()?, names.last()?);
        let bare = names.len() == 1 || STANDARD_CRATES.contains(&first.as_str());
        let log = LOG_MACROS.contains(&name.as_str())
            && ((names.len() > 1 && LOG_CRATES.contains(&first.as_str()))
                || (names.len() == 1 && self.log_macros.contains(name)));
        if log {
            return Some(usize::default());
        }
        if !bare {
            return None;
        }
        if MESSAGE_MACROS.contains(&name.as_str()) {
            Some(usize::default())
        } else if CONDITION_MACROS.contains(&name.as_str()) {
            Some(CONDITION_OPERANDS)
        } else if COMPARISON_MACROS.contains(&name.as_str()) {
            Some(COMPARISON_OPERANDS)
        } else if WRITE_MACROS.contains(&name.as_str()) && self.formatter_depth > usize::default() {
            // write!(f, "..."): the formatter is an operand, the rest a message.
            Some(CONDITION_OPERANDS)
        } else {
            None
        }
    }

    /// `env!("CARGO_PKG_VERSION")`: a variable Cargo defines, named the only way the macro allows.
    fn cargo_variable(mac: &syn::Macro) -> bool {
        let Some(last) = mac.path.segments.last() else {
            return false;
        };
        if !ENVIRONMENT_MACROS.contains(&last.ident.to_string().as_str()) {
            return false;
        }
        match mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
            Ok(args) => matches!(args.first(), Some(Expr::Lit(l)) if matches!(&l.lit, Lit::Str(v)
                if v.value().starts_with(CARGO_VARIABLE_PREFIX) || v.value() == CARGO_OUT_DIR)),
            Err(_) => false,
        }
    }

    fn include(&mut self, mac: &syn::Macro) -> bool {
        let Ok(Lit::Str(file)) = mac.parse_body::<Lit>() else {
            return false;
        };
        let path = self.folder.join(file.value());
        // A Rust file is scanned on its own where it lies; only other text is scanned where included.
        if path.extension().is_some_and(|e| e == RUST_EXTENSION) {
            return true;
        }
        if self.included.contains(&path) {
            return true;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        // The included file's findings carry its own path, next to the including file's.
        let folder = self
            .path
            .rsplit_once(PATH_COMPONENT_SEPARATOR)
            .map(|(f, _)| f.to_string());
        let rel = folder.map_or(file.value(), |f| {
            [f, file.value()].join(PATH_COMPONENT_SEPARATOR)
        });
        let including = std::mem::replace(&mut self.path, rel);
        self.included.push(path.clone());
        if let Ok(e) = syn::parse_str::<Expr>(&text) {
            self.visit_expr(&e);
        } else if let Ok(f) = syn::parse_file(&text) {
            f.items.iter().for_each(|i| self.visit_item(i));
        }
        self.included.retain(|p| *p != path);
        self.path = including;
        true
    }

    /// `concat!` of literals only: the string it makes is one literal like any other.
    fn concat(&mut self, mac: &syn::Macro, span: proc_macro2::Span) -> bool {
        let Ok(parts) = mac.parse_body_with(Punctuated::<Lit, Token![,]>::parse_terminated) else {
            return false;
        };
        let mut text = String::new();
        for part in &parts {
            match part {
                Lit::Str(s) => text.push_str(&s.value()),
                Lit::Char(c) => text.push(c.value()),
                Lit::Int(i) => text.push_str(i.base10_digits()),
                Lit::Float(f) => text.push_str(f.base10_digits()),
                Lit::Bool(b) => text.push_str(&b.value.to_string()),
                _ => return false,
            }
        }
        if !text.is_empty() {
            self.report(&Lit::Str(syn::LitStr::new(&text, span)), false, None);
        }
        true
    }

    fn mac(&mut self, mac: &syn::Macro) {
        let Some(last) = mac.path.segments.last() else {
            return;
        };
        let name = last.ident.to_string();
        if ATTRIBUTE_LIKE_MACROS.contains(&name.as_str()) {
            return;
        }
        if name == INCLUDE_MACRO && self.include(mac) {
            return;
        }
        if FileScan::cargo_variable(mac) {
            return;
        }
        if name == CONCAT_MACRO && self.concat(mac, last.ident.span()) {
            return;
        }
        if name == STRINGIFY_MACRO {
            // The tokens are the string it makes: a key written without quotes is still a key.
            let text = mac.tokens.to_string();
            if !text.is_empty() {
                let lit = Lit::Str(syn::LitStr::new(&text, last.ident.span()));
                self.report(&lit, false, None);
            }
            return;
        }
        if let Some(start) = self.message_start(mac) {
            match mac.parse_body_with(parse_message_args) {
                Ok((target, args)) => {
                    target.iter().for_each(|t| self.visit_expr(t));
                    let compared = COMPARISON_MACROS.contains(&name.as_str());
                    let operands: Vec<&Expr> = args.iter().take(start).collect();
                    // The first message argument is the text; the ones after it are values.
                    for (i, a) in args.iter().enumerate() {
                        if i == start {
                            self.message(a);
                        } else if i > start {
                            self.visit_expr(a);
                        } else if let (true, [left, right]) = (compared, operands.as_slice()) {
                            let other = if i == usize::default() { right } else { left };
                            self.given(a, hint_of(other));
                        } else {
                            self.visit_expr(a);
                        }
                    }
                }
                Err(_) => self.tokens(mac.tokens.clone(), TokenMode::Message),
            }
            return;
        }
        if CONST_DEFINING_MACROS.contains(&name.as_str()) {
            self.const_depth += 1;
            self.tokens(mac.tokens.clone(), TokenMode::All);
            self.const_depth -= 1;
            return;
        }
        if name == MATCHES_MACRO
            && let Ok((e, p, guard)) = mac.parse_body_with(parse_matches)
        {
            self.visit_expr(&e);
            self.pattern_hint = hint_of(&e);
            self.visit_pat(&p);
            self.pattern_hint = None;
            if let Some(g) = guard {
                self.visit_expr(&g);
            }
            return;
        }
        if let Ok(args) = mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
            args.iter().for_each(|a| self.visit_expr(a));
            return;
        }
        let bracketed = TokenStream::from(TokenTree::Group(Group::new(
            Delimiter::Bracket,
            mac.tokens.clone(),
        )));
        if let Ok(e) = syn::parse2::<Expr>(bracketed) {
            self.visit_expr(&e);
            return;
        }
        if let Ok(stmts) = mac.parse_body_with(Block::parse_within) {
            stmts.iter().for_each(|s| self.visit_stmt(s));
            return;
        }
        self.tokens(mac.tokens.clone(), TokenMode::All);
    }

    /// Raw tokens of a macro body that does not parse as Rust expressions or statements. A
    /// `const NAME = expr;` inside (as `bitflags!` writes flags) is read as a constant definition.
    /// A 0 or 1 here cannot be placed, so it goes to the unit list.
    fn tokens(&mut self, ts: TokenStream, mode: TokenMode) {
        self.tokens_owned(ts, mode, None);
    }

    fn tokens_owned(&mut self, ts: TokenStream, mode: TokenMode, mut owner: Option<String>) {
        let tokens: Vec<TokenTree> = ts.into_iter().collect();
        let mut after_sigil = false;
        let mut i = usize::default();
        while let Some(tt) = tokens.get(i) {
            let next = tokens.get(i + 1);
            match tt {
                TokenTree::Punct(p) => {
                    let c = p.as_char();
                    after_sigil =
                        c == ATTRIBUTE_SIGIL || (after_sigil && c == INNER_ATTRIBUTE_MARK);
                }
                TokenTree::Group(g) => {
                    if after_sigil && g.delimiter() == Delimiter::Bracket {
                        self.attribute_numbers(g.stream());
                    } else {
                        self.tokens_owned(g.stream(), mode, owner.clone());
                    }
                    after_sigil = false;
                }
                TokenTree::Literal(l) => {
                    let lit = Lit::new(l.clone());
                    let message = mode == TokenMode::Message
                        && matches!(&lit, Lit::Str(s) if message_text(&s.value()));
                    self.record(&lit, false, None, message);
                    after_sigil = false;
                }
                TokenTree::Ident(id) => {
                    after_sigil = false;
                    if *id == STRUCT_KEYWORD
                        && let Some(TokenTree::Ident(name)) = next
                    {
                        owner = Some(name.to_string());
                    }
                    if *id == CONST_KEYWORD
                        && let Some(end) = self.const_in_tokens(&tokens, i, owner.as_deref())
                    {
                        i = end;
                    }
                }
            }
            i += 1;
        }
    }

    /// The literals inside an attribute (`#[repr(align(4096))]`). Its strings must be literals, so
    /// they are listed as messages, gated only when they write out the value of a string constant
    /// (`#[serde(rename = "st_uid")]` where a constant holds "st_uid").
    fn attribute(&mut self, meta: &Meta) {
        if SKIPPED_ATTRIBUTES.iter().any(|a| meta.path().is_ident(a)) {
            return;
        }
        if meta.path().is_ident(CFG_ATTR) {
            let inner = meta
                .require_list()
                .and_then(|l| l.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated));
            if let Ok(inner) = inner {
                inner.iter().skip(1).for_each(|m| self.attribute(m));
            }
            return;
        }
        self.attribute_numbers(meta.to_token_stream());
    }

    fn attribute_numbers(&mut self, ts: TokenStream) {
        for tt in ts {
            match tt {
                TokenTree::Group(g) => self.attribute_numbers(g.stream()),
                TokenTree::Literal(l) => {
                    let lit = Lit::new(l);
                    match &lit {
                        Lit::Int(_) | Lit::Float(_) => self.report(&lit, false, None),
                        Lit::Str(_) => self.record(&lit, false, None, true),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    /// `const NAME = expr ;` starting at `at`: records and visits it, and returns the index of the `;`.
    fn const_in_tokens(
        &mut self,
        tokens: &[TokenTree],
        at: usize,
        owner: Option<&str>,
    ) -> Option<usize> {
        let [TokenTree::Ident(name), TokenTree::Punct(eq), rest @ ..] = &tokens[at + 1..] else {
            return None;
        };
        if eq.as_char() != ASSIGN {
            return None;
        }
        let length = rest
            .iter()
            .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == TERMINATOR))?;
        let start = tokens.len() - rest.len();
        let end = start + length;
        let expr = syn::parse2::<Expr>(rest[..length].iter().cloned().collect()).ok()?;
        let bare = name.to_string();
        let qualified = owner.map_or(bare.clone(), |o| format!("{o}{PATH_SEPARATOR}{bare}"));
        self.with_scope(qualified.clone(), |s| {
            s.record_const(
                bare,
                qualified,
                None,
                Some(&expr),
                Exposure::PLAIN,
                name.span(),
            );
            s.const_depth += 1;
            s.visit_expr(&expr);
            s.const_depth -= 1;
        });
        Some(end)
    }

    fn function(
        &mut self,
        attrs: &[Attribute],
        sig: &syn::Signature,
        body: Option<&Block>,
        f: impl FnOnce(&mut Self),
    ) {
        let mut uses = Uses::default();
        if let Some(b) = body {
            uses.visit_block(b);
        }
        let outer = std::mem::replace(&mut self.uses, uses.0);
        let formatter = sig.inputs.iter().any(|i| match i {
            syn::FnArg::Typed(t) => mentions(&t.ty.to_token_stream(), FORMATTER_TYPE),
            syn::FnArg::Receiver(_) => false,
        });
        let returns = self.return_hint.replace(sig.ident.to_string());
        self.value_hint = Some(sig.ident.to_string());
        self.with_test(attrs, |s| {
            s.with_scope(sig.ident.to_string(), |s| {
                s.formatter_depth += usize::from(formatter);
                f(s);
                s.formatter_depth -= usize::from(formatter);
            })
        });
        self.return_hint = returns;
        self.value_hint = None;
        self.uses = outer;
    }
}

/// The parameter names of every function a file defines.
#[derive(Default)]
struct Signatures(HashMap<String, Vec<Option<String>>>);

impl Signatures {
    fn add(&mut self, sig: &syn::Signature) {
        let names = sig
            .inputs
            .iter()
            .filter_map(|i| match i {
                syn::FnArg::Typed(t) => Some(hint_of_pat(&t.pat)),
                syn::FnArg::Receiver(_) => None,
            })
            .collect();
        self.0.insert(sig.ident.to_string(), names);
    }
}

impl<'ast> Visit<'ast> for Signatures {
    fn visit_signature(&mut self, sig: &'ast syn::Signature) {
        self.add(sig);
    }

    fn visit_local(&mut self, l: &'ast syn::Local) {
        if let (Pat::Ident(name), Some(init)) = (&l.pat, &l.init)
            && let Expr::Closure(c) = &*init.expr
        {
            let names = c.inputs.iter().map(hint_of_pat).collect();
            self.0.insert(name.ident.to_string(), names);
        }
        visit::visit_local(self, l);
    }
}

/// What each bare local name in a function body is given to as an argument.
#[derive(Default)]
struct Uses(HashMap<String, String>);

impl Uses {
    fn add(&mut self, arg: &Expr, callee: Option<String>) {
        // An element of a local array or a reference to a local goes with it (`setuid(ids[0])`).
        let arg = match unwrapped(arg) {
            Expr::Index(i) => unwrapped(&i.expr),
            Expr::Reference(r) => unwrapped(&r.expr),
            other => other,
        };
        if let (Expr::Path(p), Some(callee)) = (arg, callee)
            && let Some(name) = p.path.get_ident()
        {
            self.0.entry(name.to_string()).or_insert(callee);
        }
    }
}

impl<'ast> Visit<'ast> for Uses {
    fn visit_expr_call(&mut self, c: &'ast ExprCall) {
        c.args.iter().for_each(|a| self.add(a, hint_of(&c.func)));
        visit::visit_expr_call(self, c);
    }

    fn visit_expr_method_call(&mut self, m: &'ast ExprMethodCall) {
        m.args
            .iter()
            .for_each(|a| self.add(a, Some(m.method.to_string())));
        visit::visit_expr_method_call(self, m);
    }
}

/// `target: expr,` (log macros) and then the comma-separated arguments.
fn parse_message_args(
    input: ParseStream,
) -> syn::Result<(Option<Expr>, Punctuated<Expr, Token![,]>)> {
    let fork = input.fork();
    let target = match (fork.parse::<syn::Ident>(), fork.parse::<Token![:]>()) {
        (Ok(i), Ok(_)) if i == LOG_TARGET => {
            input.parse::<syn::Ident>()?;
            input.parse::<Token![:]>()?;
            let e: Expr = input.parse()?;
            input.parse::<Option<Token![,]>>()?;
            Some(e)
        }
        _ => None,
    };
    Ok((target, Punctuated::parse_terminated(input)?))
}

fn parse_matches(input: ParseStream) -> syn::Result<(Expr, Pat, Option<Expr>)> {
    let e: Expr = input.parse()?;
    input.parse::<Token![,]>()?;
    let p = Pat::parse_multi_with_leading_vert(input)?;
    let guard = if input.peek(Token![if]) {
        input.parse::<Token![if]>()?;
        Some(input.parse::<Expr>()?)
    } else {
        None
    };
    input.parse::<Option<Token![,]>>()?;
    Ok((e, p, guard))
}

fn mentions(ts: &TokenStream, name: &str) -> bool {
    ts.clone().into_iter().any(|t| match t {
        TokenTree::Ident(i) => i == name,
        TokenTree::Group(g) => mentions(&g.stream(), name),
        _ => false,
    })
}

fn collect_log_imports(items: &[Item], out: &mut Vec<String>) {
    fn tree(t: &UseTree, root: Option<&str>, out: &mut Vec<String>) {
        let from_log = |r: Option<&str>| r.is_some_and(|r| LOG_CRATES.contains(&r));
        match t {
            UseTree::Path(p) => {
                let name = p.ident.to_string();
                tree(&p.tree, Some(root.unwrap_or(&name)), out);
            }
            UseTree::Name(n)
                if from_log(root) && LOG_MACROS.contains(&n.ident.to_string().as_str()) =>
            {
                out.push(n.ident.to_string())
            }
            UseTree::Rename(r)
                if from_log(root) && LOG_MACROS.contains(&r.ident.to_string().as_str()) =>
            {
                out.push(r.rename.to_string())
            }
            UseTree::Glob(_) if from_log(root) => {
                out.extend(LOG_MACROS.iter().map(|m| m.to_string()))
            }
            UseTree::Group(g) => g.items.iter().for_each(|i| tree(i, root, out)),
            _ => {}
        }
    }
    for item in items {
        match item {
            Item::Use(u) => tree(&u.tree, None, out),
            Item::ExternCrate(e)
                if LOG_CRATES.contains(&e.ident.to_string().as_str())
                    && e.attrs.iter().any(|a| a.path().is_ident(MACRO_USE)) =>
            {
                out.extend(LOG_MACROS.iter().map(|m| m.to_string()));
            }
            Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_log_imports(inner, out);
                }
            }
            _ => {}
        }
    }
}

fn kind_of(lit: &Lit) -> Option<Kind> {
    Some(match lit {
        Lit::Str(_) => Kind::Str,
        Lit::ByteStr(_) => Kind::ByteStr,
        Lit::CStr(_) => Kind::CStr,
        Lit::Byte(_) => Kind::Byte,
        Lit::Char(_) => Kind::Char,
        Lit::Int(_) => Kind::Int,
        Lit::Float(_) => Kind::Float,
        Lit::Bool(_) => return None,
        _ => Kind::Other,
    })
}

fn is_empty_text(lit: &Lit) -> bool {
    match lit {
        Lit::Str(s) => s.value().is_empty(),
        Lit::ByteStr(s) => s.value().is_empty(),
        Lit::CStr(s) => s.value().as_bytes().is_empty(),
        _ => false,
    }
}

fn is_str_lit(e: &Expr) -> bool {
    matches!(e, Expr::Lit(l) if matches!(l.lit, Lit::Str(_)))
}

/// A literal (negated when under a unary minus) whose value is one of [`FREE_NUMBERS`].
pub fn is_free(lit: &Lit, negative: bool) -> bool {
    let sign = |v: f64| if negative { -v } else { v };
    match lit {
        Lit::Int(i) => i
            .base10_parse::<u128>()
            .ok()
            .and_then(|v| i128::try_from(v).ok())
            .is_some_and(|v| FREE_NUMBERS.contains(&if negative { -v } else { v })),
        Lit::Float(f) => f
            .base10_parse::<f64>()
            .is_ok_and(|v| FREE_NUMBERS.iter().any(|&u| u as f64 == sign(v))),
        _ => false,
    }
}

/// The expression inside any parentheses and invisible groups.
fn unwrapped(e: &Expr) -> &Expr {
    match e {
        Expr::Paren(p) => unwrapped(&p.expr),
        Expr::Group(g) => unwrapped(&g.expr),
        _ => e,
    }
}

/// Other crates can import it.
pub fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// `Some` or `Ok`, which wrap a value without changing what it is given to.
fn is_wrapper(func: &Expr) -> bool {
    matches!(func, Expr::Path(p) if WRAPPERS.iter().any(|w| p.path.is_ident(w)))
}

/// The free literal of an expression that is only that, possibly in parentheses, negated or cast.
fn free_lit(e: &Expr, negative: bool) -> Option<(&Lit, bool)> {
    match e {
        Expr::Paren(p) => free_lit(&p.expr, negative),
        Expr::Group(g) => free_lit(&g.expr, negative),
        Expr::Cast(c) => free_lit(&c.expr, negative),
        Expr::Reference(r) => free_lit(&r.expr, negative),
        Expr::Unary(u) if matches!(u.op, UnOp::Neg(_)) => free_lit(&u.expr, !negative),
        Expr::Lit(l) if is_free(&l.lit, negative) => Some((&l.lit, negative)),
        _ => None,
    }
}

/// Arithmetic operators, whose 0 and 1 operands are plain arithmetic; the rest compare or combine
/// booleans.
fn is_arithmetic(op: &BinOp) -> bool {
    !matches!(
        op,
        BinOp::Eq(_)
            | BinOp::Ne(_)
            | BinOp::Lt(_)
            | BinOp::Le(_)
            | BinOp::Gt(_)
            | BinOp::Ge(_)
            | BinOp::And(_)
            | BinOp::Or(_)
    )
}

fn is_comparison(op: &BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq(_) | BinOp::Ne(_) | BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_)
    )
}

/// The name an expression goes by: a variable, a field, a function or method.
fn hint_of(e: &Expr) -> Option<String> {
    match e {
        // The function and the types it belongs to (`ExitCode::from` is given an exit status), not the
        // modules it sits in.
        Expr::Path(p) => {
            let last = p.path.segments.len().checked_sub(1);
            Some(
                p.path
                    .segments
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (i, s.ident.to_string()))
                    .filter(|(i, s)| Some(*i) == last || s.starts_with(char::is_uppercase))
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>()
                    .join(HINT_SEPARATOR),
            )
        }
        Expr::Field(f) => match &f.member {
            Member::Named(i) => Some(i.to_string()),
            Member::Unnamed(_) => hint_of(&f.base),
        },
        Expr::Unary(u) => hint_of(&u.expr),
        Expr::Paren(p) => hint_of(&p.expr),
        Expr::Group(g) => hint_of(&g.expr),
        Expr::Reference(r) => hint_of(&r.expr),
        Expr::Cast(c) => hint_of(&c.expr),
        Expr::Index(i) => hint_of(&i.expr),
        Expr::MethodCall(m) => Some(m.method.to_string()),
        Expr::Call(c) => hint_of(&c.func),
        _ => None,
    }
}

fn hint_of_pat(p: &Pat) -> Option<String> {
    let join = |parts: Vec<String>| (!parts.is_empty()).then(|| parts.join(HINT_SEPARATOR));
    match p {
        Pat::Ident(i) => Some(i.ident.to_string()),
        Pat::Type(t) => hint_of_pat(&t.pat),
        Pat::Reference(r) => hint_of_pat(&r.pat),
        Pat::Paren(p) => hint_of_pat(&p.pat),
        Pat::Tuple(t) => join(t.elems.iter().filter_map(hint_of_pat).collect()),
        Pat::TupleStruct(t) => join(t.elems.iter().filter_map(hint_of_pat).collect()),
        Pat::Struct(s) => join(
            s.fields
                .iter()
                .filter_map(|f| hint_of_pat(&f.pat))
                .collect(),
        ),
        _ => None,
    }
}

/// The words of a name (`stdout_fd`, `ownerPid`), lower case.
pub fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        let boundary = !c.is_alphanumeric() || (c.is_uppercase() && prev_lower);
        if boundary && !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        if c.is_alphanumeric() {
            word.extend(c.to_lowercase());
        }
        prev_lower = c.is_lowercase();
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// A name whose words say the value is an id, a descriptor or a status (`uid`, `stdout_fd`, and the
/// C library's one-word `getuid` and `setpgid`), and not a quantity or a set of them (`pid_count`,
/// `signal_mask`).
fn names_role(hint: &str) -> bool {
    let words = words(hint);
    let role = |w: &str| {
        ROLE_WORDS.contains(&w)
            || w.strip_suffix(PLURAL_SUFFIX)
                .is_some_and(|s| ROLE_WORDS.contains(&s))
    };
    let accessor = |w: &str| {
        ROLE_ACCESSOR_PREFIXES
            .iter()
            .any(|p| w.strip_prefix(p).is_some_and(role))
    };
    !words.iter().any(|w| QUANTITY_WORDS.contains(&w.as_str()))
        && words.iter().any(|w| role(w) || accessor(w))
}

fn int_type_of(ty: &Type) -> Option<IntType> {
    match ty {
        Type::Path(p) => p
            .path
            .get_ident()
            .and_then(|i| IntType::named(&i.to_string())),
        _ => None,
    }
}

/// The configuration predicate holds only when `test` is set.
fn requires_test(ts: &TokenStream) -> bool {
    let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
    match tokens.as_slice() {
        [TokenTree::Ident(i)] => i == TEST_CFG,
        [TokenTree::Ident(i), TokenTree::Group(g)] if i == CFG_ALL => {
            split_predicates(&g.stream()).iter().any(requires_test)
        }
        [TokenTree::Ident(i), TokenTree::Group(g)] if i == CFG_ANY => {
            let inner = split_predicates(&g.stream());
            !inner.is_empty() && inner.iter().all(requires_test)
        }
        _ => false,
    }
}

fn split_predicates(ts: &TokenStream) -> Vec<TokenStream> {
    let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();
    tokens
        .split(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == PREDICATE_SEPARATOR))
        .filter(|p| !p.is_empty())
        .map(|p| p.iter().cloned().collect())
        .collect()
}

/// `#[test]`, `#[bench]`, `#[tokio::test]`, and a cfg that holds only under test: `#[cfg(test)]`,
/// `#[cfg(all(test, not(miri)))]`; not `#[cfg(any(test, feature = "x"))]` or `#[cfg(not(test))]`.
pub fn is_test_attribute(attr: &Attribute) -> bool {
    let path = attr.path();
    if path
        .segments
        .last()
        .is_some_and(|s| TEST_ATTRIBUTES.contains(&s.ident.to_string().as_str()))
    {
        return true;
    }
    if !path.is_ident(CFG_ATTRIBUTE) {
        return false;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    requires_test(&list.tokens)
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn expr_attrs(e: &Expr) -> &[Attribute] {
    match e {
        Expr::Block(x) => &x.attrs,
        Expr::Unsafe(x) => &x.attrs,
        Expr::Closure(x) => &x.attrs,
        Expr::Macro(x) => &x.attrs,
        Expr::Call(x) => &x.attrs,
        Expr::MethodCall(x) => &x.attrs,
        Expr::Struct(x) => &x.attrs,
        Expr::Tuple(x) => &x.attrs,
        Expr::Array(x) => &x.attrs,
        Expr::If(x) => &x.attrs,
        Expr::Match(x) => &x.attrs,
        Expr::Loop(x) => &x.attrs,
        Expr::While(x) => &x.attrs,
        Expr::ForLoop(x) => &x.attrs,
        Expr::Assign(x) => &x.attrs,
        _ => &[],
    }
}

/// Source text of a type or path, without the spaces the token printer puts in.
fn compact(t: &impl ToTokens) -> String {
    t.to_token_stream().to_string().split_whitespace().collect()
}

impl<'ast> Visit<'ast> for FileScan {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.attribute(&attr.meta);
    }

    fn visit_abi(&mut self, _: &'ast syn::Abi) {}

    fn visit_lit(&mut self, lit: &'ast Lit) {
        self.report(lit, false, None);
    }

    fn visit_block(&mut self, b: &'ast Block) {
        if self.const_depth == usize::default() {
            // A block's last expression is the value of what holds the block: the function, or the
            // branch of an `if` that is that value.
            let value = self.value_hint.take();
            let last = b.stmts.len().checked_sub(1);
            for (i, st) in b.stmts.iter().enumerate() {
                match st {
                    Stmt::Expr(e, None) if Some(i) == last && value.is_some() => {
                        self.value_hint = value.clone();
                        self.given(e, value.clone());
                        self.value_hint = None;
                    }
                    _ => self.visit_stmt(st),
                }
            }
            return;
        }
        // In a constant's initialiser only the block's value is the definition; its statements compute.
        let last = b.stmts.len().checked_sub(1);
        for (i, s) in b.stmts.iter().enumerate() {
            if Some(i) == last && matches!(s, Stmt::Expr(_, None)) {
                self.visit_stmt(s);
            } else {
                self.outside_const(|me| me.visit_stmt(s));
            }
        }
    }

    fn visit_expr(&mut self, e: &'ast Expr) {
        let attrs = expr_attrs(e);
        if attrs.iter().any(is_test_attribute) {
            return self.with_test(attrs, |s| s.expr(e));
        }
        self.expr(e);
    }

    fn visit_pat(&mut self, p: &'ast Pat) {
        match p {
            // A match arm compares: a free number there goes with the name being matched.
            Pat::Lit(l) if is_free(&l.lit, false) => {
                let hint = self.pattern_hint.clone();
                self.report(&l.lit, false, hint.as_deref());
            }
            Pat::Range(r) => {
                r.start.iter().for_each(|s| self.arithmetic(s));
                r.end.iter().for_each(|s| self.arithmetic(s));
            }
            _ => visit::visit_pat(self, p),
        }
    }

    fn visit_local(&mut self, l: &'ast syn::Local) {
        self.with_test(&l.attrs, |s| {
            s.visit_pat(&l.pat);
            if let Some(init) = &l.init {
                s.bound(&l.pat, &init.expr);
                if let Some((_, diverge)) = &init.diverge {
                    s.visit_expr(diverge);
                }
            }
        });
    }

    fn visit_stmt_macro(&mut self, m: &'ast syn::StmtMacro) {
        self.with_test(&m.attrs, |s| s.mac(&m.mac));
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.mac(mac);
    }

    fn visit_item(&mut self, item: &'ast Item) {
        self.with_test(item_attrs(item), |s| {
            s.outside_const(|s| visit::visit_item(s, item))
        });
    }

    fn visit_item_macro(&mut self, m: &'ast syn::ItemMacro) {
        match &m.ident {
            Some(name) if m.mac.path.is_ident(MACRO_RULES) => {
                self.with_scope(
                    [MACRO_RULES, MACRO_ITEM_SEPARATOR, &name.to_string()].concat(),
                    |s| s.tokens(m.mac.tokens.clone(), TokenMode::All),
                );
            }
            _ => self.mac(&m.mac),
        }
    }

    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.function(&[], &f.sig, Some(&f.block), |s| visit::visit_item_fn(s, f));
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.function(&f.attrs, &f.sig, Some(&f.block), |s| {
            visit::visit_impl_item_fn(s, f)
        });
    }

    fn visit_trait_item_fn(&mut self, f: &'ast syn::TraitItemFn) {
        self.function(&f.attrs, &f.sig, f.default.as_ref(), |s| {
            visit::visit_trait_item_fn(s, f)
        });
    }

    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        let private = usize::from(!is_public(&m.vis));
        self.hidden_depth += private;
        self.with_scope(m.ident.to_string(), |s| visit::visit_item_mod(s, m));
        self.hidden_depth -= private;
    }

    fn visit_item_trait(&mut self, t: &'ast syn::ItemTrait) {
        self.with_scope(t.ident.to_string(), |s| visit::visit_item_trait(s, t));
    }

    fn visit_item_struct(&mut self, t: &'ast syn::ItemStruct) {
        self.with_scope(t.ident.to_string(), |s| visit::visit_item_struct(s, t));
    }

    fn visit_item_union(&mut self, t: &'ast syn::ItemUnion) {
        self.with_scope(t.ident.to_string(), |s| visit::visit_item_union(s, t));
    }

    fn visit_item_type(&mut self, t: &'ast syn::ItemType) {
        self.with_scope(t.ident.to_string(), |s| visit::visit_item_type(s, t));
    }

    fn visit_item_enum(&mut self, e: &'ast syn::ItemEnum) {
        self.enums.push(e.ident.to_string());
        self.with_scope(e.ident.to_string(), |s| visit::visit_item_enum(s, e));
        self.enums.pop();
    }

    fn visit_variant(&mut self, v: &'ast syn::Variant) {
        v.attrs.iter().for_each(|a| self.visit_attribute(a));
        self.visit_fields(&v.fields);
        let Some((_, expr)) = &v.discriminant else {
            return;
        };
        let bare = v.ident.to_string();
        let qualified = match self.enums.last() {
            Some(e) => format!("{e}{PATH_SEPARATOR}{bare}"),
            None => bare.clone(),
        };
        self.with_scope(bare.clone(), |s| {
            s.record_const(
                bare,
                qualified,
                None,
                Some(expr),
                Exposure::PLAIN,
                v.ident.span(),
            );
            s.const_depth += 1;
            s.visit_expr(expr);
            s.const_depth -= 1;
        });
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let self_name = compact(&i.self_ty);
        let name = match &i.trait_ {
            Some((_, t, _)) => [
                TRAIT_IMPL_OPEN,
                &self_name,
                TRAIT_IMPL_AS,
                &compact(t),
                TRAIT_IMPL_CLOSE,
            ]
            .concat(),
            None => self_name.clone(),
        };
        self.impls.push(ImplScope {
            self_name,
            trait_impl: i.trait_.is_some(),
        });
        self.with_scope(name, |s| visit::visit_item_impl(s, i));
        self.impls.pop();
    }

    fn visit_item_const(&mut self, c: &'ast syn::ItemConst) {
        let outer = std::mem::take(&mut self.impls);
        self.const_item(&c.ident, &c.ty, Some(&c.expr), is_public(&c.vis), |s| {
            visit::visit_item_const(s, c)
        });
        self.impls = outer;
    }

    fn visit_item_static(&mut self, c: &'ast syn::ItemStatic) {
        let outer = std::mem::take(&mut self.impls);
        self.const_item(&c.ident, &c.ty, Some(&c.expr), is_public(&c.vis), |s| {
            visit::visit_item_static(s, c)
        });
        self.impls = outer;
    }

    fn visit_impl_item_const(&mut self, c: &'ast syn::ImplItemConst) {
        self.with_test(&c.attrs, |s| {
            s.const_item(&c.ident, &c.ty, Some(&c.expr), is_public(&c.vis), |s| {
                visit::visit_impl_item_const(s, c)
            })
        });
    }

    fn visit_trait_item_const(&mut self, c: &'ast syn::TraitItemConst) {
        let default = c.default.as_ref().map(|(_, e)| e);
        self.const_item(&c.ident, &c.ty, default, true, |s| {
            visit::visit_trait_item_const(s, c)
        });
    }
}

impl FileScan {
    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Binary(b) if is_arithmetic(&b.op) => {
                self.arithmetic(&b.left);
                self.arithmetic(&b.right);
            }
            Expr::Binary(b) if is_comparison(&b.op) => {
                self.given(&b.left, hint_of(&b.right));
                self.given(&b.right, hint_of(&b.left));
            }
            Expr::Unary(u) if matches!(u.op, UnOp::Neg(_)) => match unwrapped(&u.expr) {
                Expr::Lit(l) => self.report(&l.lit, true, None),
                _ => self.visit_expr(&u.expr),
            },
            Expr::Index(i) => {
                self.visit_expr(&i.expr);
                self.given(&i.index, hint_of(&i.expr));
            }
            Expr::Range(r) => {
                r.start.iter().for_each(|s| self.arithmetic(s));
                r.end.iter().for_each(|s| self.arithmetic(s));
            }
            Expr::Assign(a) => {
                self.visit_expr(&a.left);
                self.given(&a.right, hint_of(&a.left));
            }
            Expr::Repeat(r) => {
                self.arithmetic(&r.expr);
                self.visit_expr(&r.len);
            }
            Expr::Struct(st) => {
                self.visit_path(&st.path);
                for f in &st.fields {
                    let hint = match &f.member {
                        Member::Named(i) => Some(i.to_string()),
                        Member::Unnamed(_) => None,
                    };
                    self.with_test(&f.attrs, |s| s.given(&f.expr, hint));
                }
                if let Some(rest) = &st.rest {
                    self.visit_expr(rest);
                }
            }
            Expr::Match(m) => self.match_expr(m, None),
            Expr::Return(r) => {
                if let Some(value) = &r.expr {
                    self.given(value, self.return_hint.clone());
                }
            }
            Expr::Closure(_) => {
                let returns = self.return_hint.take();
                self.outside_const(|s| visit::visit_expr(s, e));
                self.return_hint = returns;
            }
            Expr::While(_) | Expr::ForLoop(_) | Expr::Loop(_) => {
                self.outside_const(|s| visit::visit_expr(s, e))
            }
            Expr::MethodCall(m) => self.method_call(m),
            Expr::Call(c) => self.call(c),
            _ => visit::visit_expr(self, e),
        }
    }
}

/// The folder a source file is in.
pub fn folder_of(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}
