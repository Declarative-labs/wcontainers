//! Values of constants: enough evaluation to see that two constants, or a literal and a constant, are
//! the same fact. Anything this cannot evaluate (a `size_of`, a function call) has no value and is
//! never matched.

use std::any::type_name;
use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;
use std::time::Duration;

use syn::{BinOp, Expr, GenericArgument, Lit, PathArguments, Type, UnOp};

use crate::findings::{LOCATION_SEPARATOR, PATH_SEPARATOR};

/// A value that literals and constants can share.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Value {
    Int(i128),
    /// A byte literal, kept apart from integers so that `b' '` is not matched to every constant 32.
    Byte(u8),
    Str(String),
    Char(char),
}

/// How Rust writes a byte literal: `b'x'`.
const BYTE_PREFIX: &str = "b'";
const BYTE_QUOTE: char = '\'';

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{v}"),
            Value::Byte(b) => write!(f, "{BYTE_PREFIX}{}{BYTE_QUOTE}", b.escape_ascii()),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Char(c) => write!(f, "{c:?}"),
        }
    }
}

impl Value {
    /// The value of a literal, negated when it sits under a unary minus.
    pub fn of_lit(lit: &Lit, negative: bool) -> Option<Value> {
        let sign = |v: i128| if negative { v.checked_neg() } else { Some(v) };
        match lit {
            Lit::Int(i) => i
                .base10_parse::<u128>()
                .ok()
                .and_then(|v| i128::try_from(v).ok())
                .and_then(sign)
                .map(Value::Int),
            Lit::Byte(b) if !negative => Some(Value::Byte(b.value())),
            Lit::Str(s) => Some(Value::Str(s.value())),
            // A byte string of text is the same text (`b"etc/group"` and `"etc/group"`).
            Lit::ByteStr(s) => String::from_utf8(s.value()).ok().map(Value::Str),
            Lit::Char(c) => Some(Value::Char(c.value())),
            _ => None,
        }
    }

    /// The value as a number, for comparing a constant written as an integer with one written as a
    /// byte or character literal (`47` and `b'/'`).
    pub fn numeric(&self) -> Option<i128> {
        match self {
            Value::Int(v) => Some(*v),
            Value::Byte(b) => Some(i128::from(*b)),
            Value::Char(c) => Some(i128::from(u32::from(*c))),
            Value::Str(_) => None,
        }
    }

    /// The same fact by value: equal, or equal as numbers.
    pub fn same_as(&self, other: &Value) -> bool {
        self == other || self.numeric().is_some_and(|n| other.numeric() == Some(n))
    }

    /// Values that are plain arithmetic (the free numbers) and never matched to a constant.
    pub fn is_free(&self) -> bool {
        match self {
            Value::Int(v) => crate::scan::FREE_NUMBERS.contains(v),
            Value::Str(s) => s.is_empty(),
            Value::Byte(_) | Value::Char(_) => false,
        }
    }
}

/// A fixed-width primitive integer type. `usize` and `isize` are left out on purpose: their width
/// depends on the target (the kernel is built for wasm32 and for 64-bit hosts).
#[derive(Clone, Copy, Debug)]
pub struct IntType {
    pub name: &'static str,
    pub bits: u32,
    pub signed: bool,
}

macro_rules! int_types {
    ($($t:ty),*) => {
        vec![$(IntType { name: type_name::<$t>(), bits: <$t>::BITS, signed: <$t>::MIN != 0 }),*]
    };
}

/// Every fixed-width integer type, described from the standard library's own definitions.
pub static INT_TYPES: LazyLock<Vec<IntType>> =
    LazyLock::new(|| int_types!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128));

macro_rules! sizes {
    ($($t:ty),*) => {
        vec![$((type_name::<$t>(), size_of::<$t>())),*]
    };
}

/// The size of every primitive whose size does not depend on the target, from the standard library.
static PRIMITIVE_SIZES: LazyLock<Vec<(&'static str, usize)>> = LazyLock::new(|| {
    sizes!(
        u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, char, f32, f64
    )
});

/// The function that gives a type's size.
const SIZE_OF: &str = "size_of";
/// `Duration::from_*(n).as_*()`: a time unit conversion, evaluated with the standard library.
const DURATION_TYPE: &str = "Duration";
const FROM_SECS: &str = "from_secs";
const FROM_MILLIS: &str = "from_millis";
const FROM_MICROS: &str = "from_micros";
const FROM_NANOS: &str = "from_nanos";
const DURATION_FROM: &[&str] = &[FROM_SECS, FROM_MILLIS, FROM_MICROS, FROM_NANOS];
const AS_SECS: &str = "as_secs";
const AS_MILLIS: &str = "as_millis";
const AS_MICROS: &str = "as_micros";
const AS_NANOS: &str = "as_nanos";
const DURATION_AS: &[&str] = &[AS_SECS, AS_MILLIS, AS_MICROS, AS_NANOS];
/// Integer methods a constant expression may use, evaluated as the standard library does.
const TRAILING_ZEROS: &str = "trailing_zeros";
const COUNT_ONES: &str = "count_ones";
const POW: &str = "pow";
const NEXT_MULTIPLE_OF: &str = "next_multiple_of";
const DIV_CEIL: &str = "div_ceil";
const INT_METHODS: &[&str] = &[TRAILING_ZEROS, COUNT_ONES, POW, NEXT_MULTIPLE_OF, DIV_CEIL];
/// The path segment that names the type of the impl a constant is defined in.
const SELF_TYPE: &str = "Self";

/// The associated constants of the integer types that constant expressions use.
const MAX_NAME: &str = "MAX";
const MIN_NAME: &str = "MIN";
const BITS_NAME: &str = "BITS";

impl IntType {
    pub fn named(name: &str) -> Option<IntType> {
        INT_TYPES.iter().copied().find(|t| t.name == name)
    }

    fn of_type(ty: &Type) -> Option<IntType> {
        match ty {
            Type::Path(p) if p.qself.is_none() => p
                .path
                .get_ident()
                .and_then(|i| IntType::named(&i.to_string())),
            Type::Paren(p) => IntType::of_type(&p.elem),
            Type::Group(g) => IntType::of_type(&g.elem),
            _ => None,
        }
    }

    /// The value wrapped to this type, as an `as` cast does.
    fn wrap(self, v: i128) -> Option<i128> {
        // A 128-bit type holds every value evaluation produces, except a negative one in u128.
        let Some(modulus) = 1i128.checked_shl(self.bits).filter(|m| *m > 0) else {
            return (self.signed || v >= i128::default()).then_some(v);
        };
        let r = v.rem_euclid(modulus);
        Some(if self.signed && r >= modulus >> 1 {
            r - modulus
        } else {
            r
        })
    }

    fn max(self) -> Option<i128> {
        let magnitude_bits = if self.signed {
            self.bits - 1
        } else {
            self.bits
        };
        1i128
            .checked_shl(magnitude_bits)
            .filter(|m| *m > 0)
            .map(|m| m - 1)
    }

    fn min(self) -> Option<i128> {
        if self.signed {
            self.max().map(|m| -m - 1)
        } else {
            Some(i128::default())
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BinKind {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    And,
    Or,
    Xor,
}

/// A constant's initialiser, lowered to what evaluation needs, so it can leave the parsing thread.
#[derive(Clone, Debug)]
pub enum CExpr {
    Value(Value),
    /// A reference to another constant, by its last path segment and the segment before it
    /// (`general::EINVAL`, `Self::SIZE`, `Errno::EINVAL`).
    Ref {
        name: String,
        qualifier: Option<String>,
    },
    Neg(Box<CExpr>),
    Not(Box<CExpr>),
    Bin(BinKind, Box<CExpr>, Box<CExpr>),
    Cast(Option<IntType>, Box<CExpr>),
    SizeOf(TypeSize),
    Duration {
        from: String,
        amount: Box<CExpr>,
        to: String,
    },
    Method {
        name: String,
        receiver: Box<CExpr>,
        args: Vec<CExpr>,
    },
    Unknown,
}

/// The size of a type, as far as it does not depend on the target.
#[derive(Clone, Debug)]
pub enum TypeSize {
    Fixed(usize),
    Array(Box<TypeSize>, Box<CExpr>),
    Unknown,
}

impl TypeSize {
    fn of(ty: &Type) -> TypeSize {
        match ty {
            Type::Path(p) if p.qself.is_none() => p
                .path
                .get_ident()
                .and_then(|i| PRIMITIVE_SIZES.iter().find(|(n, _)| i == n))
                .map_or(TypeSize::Unknown, |(_, s)| TypeSize::Fixed(*s)),
            Type::Array(a) => TypeSize::Array(
                Box::new(TypeSize::of(&a.elem)),
                Box::new(CExpr::lower(&a.len)),
            ),
            Type::Paren(p) => TypeSize::of(&p.elem),
            Type::Group(g) => TypeSize::of(&g.elem),
            _ => TypeSize::Unknown,
        }
    }
}

/// The one type argument of a path's last segment (`size_of::<T>`).
fn turbofish_type(path: &syn::Path) -> Option<&Type> {
    let PathArguments::AngleBracketed(args) = &path.segments.last()?.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(t) if args.args.len() == 1 => Some(t),
        _ => None,
    }
}

fn path_names(path: &syn::Path) -> Vec<String> {
    path.segments.iter().map(|s| s.ident.to_string()).collect()
}

impl CExpr {
    pub fn lower(e: &Expr) -> CExpr {
        match e {
            Expr::Lit(l) => Value::of_lit(&l.lit, false).map_or(CExpr::Unknown, CExpr::Value),
            Expr::Paren(p) => CExpr::lower(&p.expr),
            Expr::Group(g) => CExpr::lower(&g.expr),
            Expr::Unary(u) => match u.op {
                UnOp::Neg(_) => CExpr::Neg(Box::new(CExpr::lower(&u.expr))),
                UnOp::Not(_) => CExpr::Not(Box::new(CExpr::lower(&u.expr))),
                _ => CExpr::Unknown,
            },
            Expr::Binary(b) => {
                let kind = match b.op {
                    BinOp::Add(_) => BinKind::Add,
                    BinOp::Sub(_) => BinKind::Sub,
                    BinOp::Mul(_) => BinKind::Mul,
                    BinOp::Div(_) => BinKind::Div,
                    BinOp::Rem(_) => BinKind::Rem,
                    BinOp::Shl(_) => BinKind::Shl,
                    BinOp::Shr(_) => BinKind::Shr,
                    BinOp::BitAnd(_) => BinKind::And,
                    BinOp::BitOr(_) => BinKind::Or,
                    BinOp::BitXor(_) => BinKind::Xor,
                    _ => return CExpr::Unknown,
                };
                CExpr::Bin(
                    kind,
                    Box::new(CExpr::lower(&b.left)),
                    Box::new(CExpr::lower(&b.right)),
                )
            }
            Expr::Cast(c) => CExpr::Cast(IntType::of_type(&c.ty), Box::new(CExpr::lower(&c.expr))),
            Expr::Call(c) if c.args.is_empty() => match &*c.func {
                Expr::Path(p) if path_names(&p.path).last().is_some_and(|n| n == SIZE_OF) => {
                    turbofish_type(&p.path)
                        .map_or(CExpr::Unknown, |t| CExpr::SizeOf(TypeSize::of(t)))
                }
                _ => CExpr::Unknown,
            },
            Expr::MethodCall(m) if DURATION_AS.contains(&m.method.to_string().as_str()) => {
                match &*m.receiver {
                    Expr::Call(c) if c.args.len() == 1 => match &*c.func {
                        Expr::Path(p) => match path_names(&p.path).as_slice() {
                            [.., ty, from]
                                if ty == DURATION_TYPE
                                    && DURATION_FROM.contains(&from.as_str()) =>
                            {
                                CExpr::Duration {
                                    from: from.clone(),
                                    amount: Box::new(
                                        c.args.first().map_or(CExpr::Unknown, CExpr::lower),
                                    ),
                                    to: m.method.to_string(),
                                }
                            }
                            _ => CExpr::Unknown,
                        },
                        _ => CExpr::Unknown,
                    },
                    _ => CExpr::Unknown,
                }
            }
            Expr::MethodCall(m) if INT_METHODS.contains(&m.method.to_string().as_str()) => {
                CExpr::Method {
                    name: m.method.to_string(),
                    receiver: Box::new(CExpr::lower(&m.receiver)),
                    args: m.args.iter().map(CExpr::lower).collect(),
                }
            }
            Expr::Path(p) if p.qself.is_none() => {
                let segments: Vec<String> = p
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                match segments.as_slice() {
                    [ty, name] if IntType::named(ty).is_some() => {
                        let t = IntType::named(ty).expect("checked above");
                        let v = match name.as_str() {
                            MAX_NAME => t.max(),
                            MIN_NAME => t.min(),
                            BITS_NAME => Some(i128::from(t.bits)),
                            _ => None,
                        };
                        v.map_or(CExpr::Unknown, |v| CExpr::Value(Value::Int(v)))
                    }
                    [.., qualifier, last] => CExpr::Ref {
                        name: last.clone(),
                        qualifier: Some(qualifier.clone()),
                    },
                    [last] => CExpr::Ref {
                        name: last.clone(),
                        qualifier: None,
                    },
                    [] => CExpr::Unknown,
                }
            }
            _ => CExpr::Unknown,
        }
    }
}

/// Where a constant is defined, which decides how it is matched and reported.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstScope {
    Code,
    Test,
    Generated,
    /// A constant of a reference crate (for example linux-raw-sys), with the module path it is shown as.
    Reference(String),
}

#[derive(Clone, Debug)]
pub struct ConstDef {
    /// The name as written (`EINVAL`, `PAGE_SIZE`, an enum variant's name).
    pub bare: String,
    /// The name with its type for associated constants and enum variants (`Errno::EINVAL`).
    pub qualified: String,
    pub path: String,
    /// The folder of the crate the file belongs to, for resolving references by name.
    pub crate_dir: String,
    pub line: usize,
    pub column: usize,
    pub item: String,
    pub expr: CExpr,
    pub ty: Option<IntType>,
    pub scope: ConstScope,
    /// An associated constant inside `impl Trait for Type`, which the trait requires and which is
    /// therefore not a redefinition of the same name elsewhere.
    pub trait_impl: bool,
    /// Other crates can import it (`pub`).
    pub public: bool,
    pub value: Option<Value>,
}

/// A qualifier that names a type (`Errno::`, `Self::`) rather than a module (`general::`).
fn names_type(qualifier: &str) -> bool {
    qualifier.chars().next().is_some_and(char::is_uppercase)
}

impl ConstDef {
    pub fn display(&self) -> String {
        match &self.scope {
            ConstScope::Reference(module) => format!("{module}{PATH_SEPARATOR}{}", self.bare),
            _ => format!(
                "{} ({}{LOCATION_SEPARATOR}{})",
                self.qualified, self.path, self.line
            ),
        }
    }

    /// An associated constant or an enum variant, as opposed to a free constant.
    pub fn associated(&self) -> bool {
        self.qualified != self.bare
    }

    /// The constant this one is a second name for, when its initialiser is only a path.
    pub fn alias(&self) -> Option<&str> {
        match &self.expr {
            CExpr::Ref { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The type an associated constant belongs to.
    fn owner_type(&self) -> Option<&str> {
        self.qualified.rsplit_once(PATH_SEPARATOR).map(|(t, _)| t)
    }

    fn namespace(&self) -> Option<&str> {
        match &self.scope {
            ConstScope::Reference(module) => Some(module),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Todo,
    Busy,
    Done,
}

/// Evaluates every constant's value in place.
pub fn evaluate(defs: &mut [ConstDef]) {
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, d) in defs.iter().enumerate() {
        by_name.entry(d.bare.clone()).or_default().push(i);
    }
    let mut state = vec![State::Todo; defs.len()];
    for i in 0..defs.len() {
        eval_def(i, defs, &by_name, &mut state);
    }
}

fn eval_def(
    i: usize,
    defs: &mut [ConstDef],
    by_name: &HashMap<String, Vec<usize>>,
    state: &mut [State],
) -> Option<Value> {
    match state[i] {
        State::Done => return defs[i].value.clone(),
        State::Busy => return None,
        State::Todo => {}
    }
    state[i] = State::Busy;
    let expr = defs[i].expr.clone();
    let mut value = eval_expr(&expr, i, defs, by_name, state);
    if let (Some(Value::Int(v)), Some(t)) = (&value, defs[i].ty) {
        value = t.wrap(*v).map(Value::Int);
    }
    defs[i].value = value.clone();
    state[i] = State::Done;
    value
}

fn eval_expr(
    e: &CExpr,
    at: usize,
    defs: &mut [ConstDef],
    by_name: &HashMap<String, Vec<usize>>,
    state: &mut [State],
) -> Option<Value> {
    let int = |e: &CExpr, defs: &mut [ConstDef], state: &mut [State]| match eval_expr(
        e, at, defs, by_name, state,
    ) {
        Some(Value::Int(v)) => Some(v),
        _ => None,
    };
    match e {
        CExpr::Value(v) => Some(v.clone()),
        CExpr::Unknown => None,
        CExpr::Ref { name, qualifier } => {
            resolve(name, qualifier.as_deref(), at, defs, by_name, state)
        }
        CExpr::SizeOf(t) => size_of_type(t, at, defs, by_name, state)
            .and_then(|s| i128::try_from(s).ok())
            .map(Value::Int),
        CExpr::Duration { from, amount, to } => {
            let amount = u64::try_from(int(amount, defs, state)?).ok()?;
            let d = match from.as_str() {
                FROM_SECS => Duration::from_secs(amount),
                FROM_MILLIS => Duration::from_millis(amount),
                FROM_MICROS => Duration::from_micros(amount),
                FROM_NANOS => Duration::from_nanos(amount),
                _ => return None,
            };
            let v = match to.as_str() {
                AS_SECS => u128::from(d.as_secs()),
                AS_MILLIS => d.as_millis(),
                AS_MICROS => d.as_micros(),
                AS_NANOS => d.as_nanos(),
                _ => return None,
            };
            i128::try_from(v).ok().map(Value::Int)
        }
        CExpr::Method {
            name,
            receiver,
            args,
        } => {
            let v = int(receiver, defs, state)?;
            let arg = |defs: &mut [ConstDef], state: &mut [State]| {
                args.first().and_then(|a| int(a, defs, state))
            };
            let ceil = |m: i128| {
                (m > i128::default() && v >= i128::default())
                    .then(|| v.div_euclid(m) + i128::from(v.rem_euclid(m) != i128::default()))
            };
            let r = match name.as_str() {
                TRAILING_ZEROS if v > i128::default() => Some(i128::from(v.trailing_zeros())),
                COUNT_ONES if v >= i128::default() => Some(i128::from(v.count_ones())),
                POW => arg(defs, state)
                    .and_then(|e| u32::try_from(e).ok())
                    .and_then(|e| v.checked_pow(e)),
                NEXT_MULTIPLE_OF => arg(defs, state).and_then(|m| ceil(m)?.checked_mul(m)),
                DIV_CEIL => arg(defs, state).and_then(ceil),
                _ => None,
            };
            r.map(Value::Int)
        }
        CExpr::Neg(inner) => int(inner, defs, state)
            .and_then(i128::checked_neg)
            .map(Value::Int),
        CExpr::Not(inner) => int(inner, defs, state).map(|v| Value::Int(!v)),
        CExpr::Cast(ty, inner) => match (eval_expr(inner, at, defs, by_name, state), ty) {
            (Some(Value::Int(v)), Some(t)) => t.wrap(v).map(Value::Int),
            (Some(Value::Byte(b)), Some(t)) => t.wrap(i128::from(b)).map(Value::Int),
            (Some(Value::Char(c)), Some(t)) => t.wrap(i128::from(u32::from(c))).map(Value::Int),
            (v, _) => v,
        },
        CExpr::Bin(kind, l, r) => {
            let (l, r) = (int(l, defs, state)?, int(r, defs, state)?);
            let shift = || u32::try_from(r).ok();
            let v = match kind {
                BinKind::Add => l.checked_add(r),
                BinKind::Sub => l.checked_sub(r),
                BinKind::Mul => l.checked_mul(r),
                BinKind::Div => l.checked_div(r),
                BinKind::Rem => l.checked_rem(r),
                BinKind::Shl => shift().and_then(|s| l.checked_shl(s)),
                BinKind::Shr => shift().and_then(|s| l.checked_shr(s)),
                BinKind::And => Some(l & r),
                BinKind::Or => Some(l | r),
                BinKind::Xor => Some(l ^ r),
            };
            v.map(Value::Int)
        }
    }
}

fn size_of_type(
    t: &TypeSize,
    at: usize,
    defs: &mut [ConstDef],
    by_name: &HashMap<String, Vec<usize>>,
    state: &mut [State],
) -> Option<u128> {
    match t {
        TypeSize::Fixed(s) => u128::try_from(*s).ok(),
        TypeSize::Array(elem, len) => {
            let len = match eval_expr(len, at, defs, by_name, state)? {
                Value::Int(v) => u128::try_from(v).ok()?,
                _ => return None,
            };
            size_of_type(elem, at, defs, by_name, state)?.checked_mul(len)
        }
        TypeSize::Unknown => None,
    }
}

/// Resolves a name the way a reader would: a type-qualified path (`Errno::EINVAL`, `Self::SIZE`) only
/// to that type's constant, any other path only to a free constant; then the same file first, the same
/// crate next, anywhere last, and only when every candidate at that level that has a value agrees on it.
fn resolve(
    name: &str,
    qualifier: Option<&str>,
    at: usize,
    defs: &mut [ConstDef],
    by_name: &HashMap<String, Vec<usize>>,
    state: &mut [State],
) -> Option<Value> {
    let wanted_type = match qualifier {
        Some(SELF_TYPE) => Some(defs[at].owner_type()?.to_string()),
        Some(q) if names_type(q) => Some(q.to_string()),
        _ => None,
    };
    let candidates: Vec<usize> = by_name
        .get(name)?
        .iter()
        .copied()
        .filter(|&c| c != at && defs[c].namespace() == defs[at].namespace())
        .filter(|&c| match &wanted_type {
            Some(t) => defs[c].owner_type() == Some(t.as_str()),
            None => !defs[c].associated(),
        })
        .collect();
    let same_file: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&c| defs[c].path == defs[at].path)
        .collect();
    let same_crate: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|&c| defs[c].crate_dir == defs[at].crate_dir)
        .collect();
    let tier = [same_file, same_crate, candidates]
        .into_iter()
        .find(|t| !t.is_empty())?;
    let mut agreed: Option<Value> = None;
    for c in tier {
        // A namesake this cannot evaluate (a function call, a usize-dependent size) is left out rather
        // than spoiling every constant that refers to the name.
        let Some(v) = eval_def(c, defs, by_name, state) else {
            continue;
        };
        match &agreed {
            Some(a) if *a != v => return None,
            _ => agreed = Some(v),
        }
    }
    agreed
}
