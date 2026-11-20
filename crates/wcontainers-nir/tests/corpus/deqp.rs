//! dEQP's shader library files (VK-GL-CTS `data/gles*/shaders/*.test`): each case read from its
//! file and made into a program as dEQP's shader library case specializes it
//! (`glsShaderLibraryCase.cpp`), except where dEQP compares the outputs in the shader: here every
//! output is written to a colour target, so that llvmpipe and the route are compared on the values
//! themselves.

use crate::glsl::{
    ES_ONLY_VERSION, ES_PROFILE as ES, EXTENSION_DIRECTIVE, FRAG_COLOR as LEGACY_FRAG_COLOR,
    VERSION_DIRECTIVE,
};
use std::path::{Path, PathBuf};
use wcontainers_nir::mesa::Stage;

/// The extension of dEQP's shader library files.
pub const EXTENSION: &str = "test";
/// Between a library file's path and a case's name in a corpus item.
const CASE_SEPARATOR: char = '#';
/// Between a group's name and what it holds in a case's name, as dEQP names its cases.
const GROUP_SEPARATOR: char = '.';
/// The two cases a `both` source gives, by the suffix dEQP adds to the case's name.
const VERTEX_SUFFIX: &str = "_vertex";
const FRAGMENT_SUFFIX: &str = "_fragment";

/// The format's keywords (`gluShaderLibrary.cpp`).
const GROUP: &str = "group";
const CASE: &str = "case";
const END: &str = "end";
const IMPORT: &str = "import";
const DESC: &str = "desc";
const EXPECT: &str = "expect";
const VALUES: &str = "values";
const VERSION: &str = "version";
const REQUIRE: &str = "require";
const OUTPUT_COLOR: &str = "output_color";
const PIPELINE_PROGRAM: &str = "pipeline_program";
const BOTH: &str = "both";
const VERTEX: &str = "vertex";
const FRAGMENT: &str = "fragment";
const OTHER_STAGES: [&str; 3] = [
    "geometry",
    "tessellation_control",
    "tessellation_evaluation",
];
const INPUT: &str = "input";
const OUTPUT: &str = "output";
const UNIFORM: &str = "uniform";
const EXTENSION_REQUIREMENT: &str = "extension";
const LIMIT_REQUIREMENT: &str = "limit";
/// The results a case may expect in which a program is built and drawn.
const BUILT: [&str; 2] = ["pass", "build_successful"];
/// The version a case has when it names none (`DEFAULT_GLSL_VERSION`).
const DEFAULT_VERSION: Version = Version {
    number: ES_ONLY_VERSION,
    es: true,
};
/// A tab's width in a source's indentation, as dEQP counts it.
const TAB_WIDTH: usize = 4;

/// dEQP's names in the programs it specializes.
const POSITION_ATTRIBUTE: &str = "dEQP_Position";
const FRAG_COLOR: &str = "dEQP_FragColor";
const ATTRIBUTE_PREFIX: &str = "a_";
const VARYING_PREFIX: &str = "v_";
/// The colour targets after the first, written here with the outputs dEQP compares in the shader.
const OUTPUT_PREFIX: &str = "wc_output_";
/// The components of one colour target.
const TARGET_COMPONENTS: usize = 4;
/// The colour targets a program may write: the driver's draw buffers (`WC_GL_DRAW_BUFFERS`).
const DRAW_BUFFERS: usize = 8;

/// A case's GLSL version.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Version {
    number: u32,
    es: bool,
}

impl Version {
    /// Whether stage values are `in` and `out` (every version but GLSL ES 1.00).
    fn in_out(self) -> bool {
        self != DEFAULT_VERSION
    }

    fn directive(self) -> String {
        if self.es && self.in_out() {
            format!("{VERSION_DIRECTIVE} {} {ES}", self.number)
        } else {
            format!("{VERSION_DIRECTIVE} {}", self.number)
        }
    }
}

/// Where a stage value is declared, which gives its qualifier.
#[derive(Clone, Copy)]
enum Qualifier {
    VertexInput,
    VertexOutput,
    FragmentInput,
}

impl Qualifier {
    /// A stage value's declaration, `rest` after its qualifier in the version's words.
    fn declared(self, version: Version, rest: &str) -> String {
        match (self, version.in_out()) {
            (Qualifier::VertexInput | Qualifier::FragmentInput, true) => format!("in {rest}"),
            (Qualifier::VertexOutput, true) => format!("out {rest}"),
            (Qualifier::VertexInput, false) => format!("attribute {rest}"),
            (Qualifier::VertexOutput | Qualifier::FragmentInput, false) => {
                format!("varying {rest}")
            }
        }
    }
}

/// A scalar kind of the format's types.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Float,
    Int,
    Uint,
    Bool,
}

/// One of the format's types: its kind, and its rows and columns (one column but for matrices).
#[derive(Clone, Copy, Debug, PartialEq)]
struct ValueType {
    kind: Kind,
    rows: u32,
    columns: u32,
}

/// The scalar types' names and the prefix of their vectors' names.
const SCALARS: [(&str, &str, Kind); 4] = [
    ("float", "vec", Kind::Float),
    ("int", "ivec", Kind::Int),
    ("uint", "uvec", Kind::Uint),
    ("bool", "bvec", Kind::Bool),
];
const MATRIX_PREFIX: &str = "mat";
const MATRIX_SHAPE: char = 'x';

impl ValueType {
    fn parse(name: &str) -> Option<ValueType> {
        let one = |kind, rows, columns| {
            Some(ValueType {
                kind,
                rows,
                columns,
            })
        };
        for (scalar, vector, kind) in SCALARS {
            if name == scalar {
                return one(kind, 1, 1);
            }
            if let Some(size) = name.strip_prefix(vector) {
                return one(kind, size.parse().ok()?, 1);
            }
        }
        let shape = name.strip_prefix(MATRIX_PREFIX)?;
        match shape.split_once(MATRIX_SHAPE) {
            Some((columns, rows)) => one(Kind::Float, rows.parse().ok()?, columns.parse().ok()?),
            None => {
                let size = shape.parse().ok()?;
                one(Kind::Float, size, size)
            }
        }
    }

    fn name(self) -> String {
        let (scalar, vector, _) = SCALARS
            .iter()
            .find(|(_, _, k)| *k == self.kind)
            .expect("every kind has a name");
        match (self.columns, self.rows) {
            (1, 1) => (*scalar).to_owned(),
            (1, rows) => format!("{vector}{rows}"),
            (columns, rows) if columns == rows => format!("{MATRIX_PREFIX}{columns}"),
            (columns, rows) => format!("{MATRIX_PREFIX}{columns}{MATRIX_SHAPE}{rows}"),
        }
    }

    /// The type of the same shape with float components, as dEQP passes values between stages.
    fn float_type(self) -> ValueType {
        ValueType {
            kind: Kind::Float,
            ..self
        }
    }

    /// Each component of a value of this type, by its name, column by column.
    fn components(self, name: &str) -> Vec<String> {
        match (self.columns, self.rows) {
            (1, 1) => vec![name.to_owned()],
            (1, rows) => (0..rows).map(|r| format!("{name}[{r}]")).collect(),
            (columns, rows) => (0..columns)
                .flat_map(|c| (0..rows).map(move |r| format!("{name}[{c}][{r}]")))
                .collect(),
        }
    }
}

/// Where a value of a case's `values` block goes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Storage {
    Input,
    Output,
    Uniform,
}

#[derive(Clone, Debug)]
struct Value {
    storage: Storage,
    ty: ValueType,
    name: String,
}

/// An extension a case requires: its alternatives, and the stages it is required in (all when
/// none are named).
#[derive(Clone, Debug)]
struct Requirement {
    alternatives: Vec<String>,
    stages: Option<Vec<String>>,
}

/// One case of a library file, as read.
#[derive(Clone, Debug, Default)]
struct Case {
    name: String,
    version: Option<Version>,
    expect: Option<String>,
    values: Vec<Value>,
    both: Option<String>,
    vertex: Vec<String>,
    fragment: Vec<String>,
    /// Stages the common form lacks, by the format's name.
    others: Vec<String>,
    pipeline: bool,
    requirements: Vec<Requirement>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Token<'a> {
    Word(&'a str),
    Number(&'a str),
    /// A shader source between `""` and `""`, its quotes included.
    Source(&'a str),
    /// A string between quotes, its quotes included.
    Text(&'a str),
    Symbol(char),
    End,
}

const SOURCE_QUOTES: &str = "\"\"";
const QUOTES: [char; 2] = ['"', '\''];
const ESCAPE: char = '\\';
pub const COMMENT: char = '#';
const SYMBOLS: &str = "=+-,|;()[]{}>";
const FRACTION_MARK: char = '.';
const EXPONENTS: [char; 2] = ['e', 'E'];
const SIGNS: [char; 2] = ['+', '-'];

/// Whether `c` continues a word (`isCaseNameChar`).
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
}

struct Lexer<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Lexer<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn next(&mut self) -> Result<Token<'a>, String> {
        loop {
            let rest = self.rest();
            let trimmed = rest.trim_start();
            self.at += rest.len() - trimmed.len();
            if trimmed.starts_with(COMMENT) {
                self.at += trimmed.find('\n').unwrap_or(trimmed.len());
            } else {
                break;
            }
        }
        let rest = self.rest();
        let Some(first) = rest.chars().next() else {
            return Ok(Token::End);
        };
        let (token, length) = if first.is_ascii_alphabetic() {
            let n = rest.find(|c| !is_word_char(c)).unwrap_or(rest.len());
            (Token::Word(&rest[..n]), n)
        } else if first.is_ascii_digit() {
            let digits = |s: &str| s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
            let mut n = digits(rest);
            if rest[n..].starts_with(FRACTION_MARK) {
                n += 1;
                n += digits(&rest[n..]);
                if rest[n..].starts_with(EXPONENTS) {
                    n += 1;
                    if rest[n..].starts_with(SIGNS) {
                        n += 1;
                    }
                    n += digits(&rest[n..]);
                }
            }
            (Token::Number(&rest[..n]), n)
        } else if rest.starts_with(SOURCE_QUOTES) {
            let n = quoted_end(rest, SOURCE_QUOTES.len(), |s| s.starts_with(SOURCE_QUOTES))
                .ok_or("a shader source without its closing quotes")?
                + SOURCE_QUOTES.len();
            (Token::Source(&rest[..n]), n)
        } else if QUOTES.contains(&first) {
            let n = quoted_end(rest, first.len_utf8(), |s| s.starts_with(first))
                .ok_or("a string without its closing quote")?
                + first.len_utf8();
            (Token::Text(&rest[..n]), n)
        } else if SYMBOLS.contains(first) {
            (Token::Symbol(first), first.len_utf8())
        } else {
            return Err(format!("an unexpected character {first:?}"));
        };
        self.at += length;
        Ok(token)
    }

    fn word(&mut self) -> Result<&'a str, String> {
        match self.next()? {
            Token::Word(w) => Ok(w),
            other => Err(format!("a word expected, {other:?} found")),
        }
    }

    fn symbol(&mut self, symbol: char) -> Result<(), String> {
        match self.next()? {
            Token::Symbol(s) if s == symbol => Ok(()),
            other => Err(format!("{symbol:?} expected, {other:?} found")),
        }
    }

    fn text(&mut self) -> Result<String, String> {
        match self.next()? {
            Token::Text(t) => Ok(unescaped(&t[1..t.len() - 1])),
            other => Err(format!("a string expected, {other:?} found")),
        }
    }

    fn source(&mut self) -> Result<String, String> {
        match self.next()? {
            Token::Source(s) => Ok(shader_source(
                &s[SOURCE_QUOTES.len()..s.len() - SOURCE_QUOTES.len()],
            )),
            other => Err(format!("a shader source expected, {other:?} found")),
        }
    }

    /// The tokens up to and including `last`.
    fn skip_past(&mut self, last: char) -> Result<(), String> {
        loop {
            match self.next()? {
                Token::Symbol(s) if s == last => return Ok(()),
                Token::End => return Err(format!("{last:?} expected before the end")),
                _ => {}
            }
        }
    }
}

/// Where a quoted text that starts at `from` ends (`closes` true there), past escapes.
fn quoted_end(text: &str, from: usize, closes: impl Fn(&str) -> bool) -> Option<usize> {
    let mut at = from;
    while at < text.len() {
        let rest = &text[at..];
        if closes(rest) {
            return Some(at);
        }
        let step = if rest.starts_with(ESCAPE) {
            ESCAPE.len_utf8() + rest[ESCAPE.len_utf8()..].chars().next()?.len_utf8()
        } else {
            rest.chars().next()?.len_utf8()
        };
        at += step;
    }
    None
}

/// A quoted text with its escapes made the characters they stand for (`\n`, `\t`, and any other
/// character as itself).
fn unescaped(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == ESCAPE {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A shader source as dEQP takes it (`parseShaderSource`): its first line break dropped, its
/// escapes made characters, and the first line's indentation taken off every line.
fn shader_source(text: &str) -> String {
    let text = text.trim_start_matches(' ');
    let text = text.strip_prefix('\r').unwrap_or(text);
    let text = text.strip_prefix('\n').unwrap_or(text);
    let text = unescaped(text).replace("\r\n", "\n");
    let width = |line: &str| -> usize {
        line.chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| if c == '\t' { TAB_WIDTH } else { 1 })
            .sum()
    };
    let indent = width(&text);
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let mut taken = 0;
        let mut rest = line;
        while taken < indent
            && let Some(c) = rest.chars().next().filter(|c| *c == ' ' || *c == '\t')
        {
            taken += if c == '\t' { TAB_WIDTH } else { 1 };
            rest = &rest[c.len_utf8()..];
        }
        out += rest;
    }
    out
}

/// Every case of a library file, each named by its groups and its own name.
fn cases(text: &str) -> Result<Vec<Case>, String> {
    let mut lexer = Lexer { text, at: 0 };
    let mut cases = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    loop {
        match lexer.next()? {
            Token::End if groups.is_empty() => return Ok(cases),
            Token::Word(GROUP) => {
                let name = match lexer.next()? {
                    Token::Word(w) | Token::Number(w) => w.to_owned(),
                    other => return Err(format!("a group name expected, {other:?} found")),
                };
                lexer.text()?;
                groups.push(name);
            }
            Token::Word(END) if !groups.is_empty() => {
                groups.pop();
            }
            Token::Word(IMPORT) => {
                // The imported file is a corpus item of its own.
                lexer.text()?;
            }
            Token::Word(CASE) => {
                let own = match lexer.next()? {
                    Token::Word(w) | Token::Number(w) => w,
                    other => return Err(format!("a case name expected, {other:?} found")),
                };
                let mut name = groups.join(&GROUP_SEPARATOR.to_string());
                if !name.is_empty() {
                    name.push(GROUP_SEPARATOR);
                }
                name += own;
                cases.push(case(&mut lexer, name)?);
            }
            other => return Err(format!("{other:?} where a group or case was expected")),
        }
    }
}

/// The rest of a case, after its name, up to its `end`.
fn case(lexer: &mut Lexer, name: String) -> Result<Case, String> {
    let mut case = Case {
        name,
        ..Case::default()
    };
    loop {
        match lexer.word()? {
            END => return Ok(case),
            DESC => {
                lexer.text()?;
            }
            EXPECT => case.expect = Some(lexer.word()?.to_owned()),
            OUTPUT_COLOR => {
                lexer.word()?;
            }
            VERSION => {
                let number = match lexer.next()? {
                    Token::Number(n) => n.parse().map_err(|_| format!("version {n}"))?,
                    other => return Err(format!("a version expected, {other:?} found")),
                };
                let save = lexer.at;
                let es = lexer.next()? == Token::Word(ES);
                if !es {
                    lexer.at = save;
                }
                case.version = Some(Version { number, es });
            }
            VALUES => values(lexer, &mut case.values)?,
            REQUIRE => {
                if let Some(r) = requirement(lexer)? {
                    case.requirements.push(r);
                }
            }
            PIPELINE_PROGRAM => {
                case.pipeline = true;
                while lexer.next()? != Token::Word(END) {}
            }
            BOTH => case.both = Some(lexer.source()?),
            VERTEX => case.vertex.push(lexer.source()?),
            FRAGMENT => case.fragment.push(lexer.source()?),
            other if OTHER_STAGES.contains(&other) => {
                lexer.source()?;
                case.others.push(other.to_owned());
            }
            other => return Err(format!("{other} in case {}", case.name)),
        }
    }
}

fn values(lexer: &mut Lexer, out: &mut Vec<Value>) -> Result<(), String> {
    lexer.symbol('{')?;
    loop {
        let storage = match lexer.next()? {
            Token::Symbol('}') => return Ok(()),
            Token::Word(INPUT) => Storage::Input,
            Token::Word(OUTPUT) => Storage::Output,
            Token::Word(UNIFORM) => Storage::Uniform,
            other => return Err(format!("{other:?} in a values block")),
        };
        let type_name = lexer.word()?;
        let ty = ValueType::parse(type_name).ok_or_else(|| format!("the type {type_name}"))?;
        let name = match lexer.next()? {
            Token::Word(w) => w.to_owned(),
            Token::Text(t) => unescaped(&t[1..t.len() - 1]),
            other => return Err(format!("a value name expected, {other:?} found")),
        };
        // The values themselves are dEQP's own inputs and expectations; the corpus seeds its own.
        lexer.skip_past(';')?;
        out.push(Value { storage, ty, name });
    }
}

/// A `require`: an extension's alternatives and stages, or None for a limit or capability (left
/// to the compiler to refuse).
fn requirement(lexer: &mut Lexer) -> Result<Option<Requirement>, String> {
    match lexer.word()? {
        EXTENSION_REQUIREMENT => {
            lexer.symbol('{')?;
            let mut alternatives = vec![lexer.text()?];
            loop {
                match lexer.next()? {
                    Token::Symbol('}') => break,
                    Token::Symbol('|') => alternatives.push(lexer.text()?),
                    other => return Err(format!("{other:?} in an extension list")),
                }
            }
            let save = lexer.at;
            // The grammar has one word here, before a stage list.
            let stages = if matches!(lexer.next()?, Token::Word(_)) {
                lexer.symbol('{')?;
                let mut stages = vec![lexer.word()?.to_owned()];
                loop {
                    match lexer.next()? {
                        Token::Symbol('}') => break,
                        Token::Symbol(',') => stages.push(lexer.word()?.to_owned()),
                        other => return Err(format!("{other:?} in a stage list")),
                    }
                }
                Some(stages)
            } else {
                lexer.at = save;
                None
            };
            Ok(Some(Requirement {
                alternatives,
                stages,
            }))
        }
        LIMIT_REQUIREMENT => {
            lexer.text()?;
            lexer.symbol('>')?;
            lexer.next()?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// A corpus item's library file and case name, when the item is a dEQP case.
pub fn split(item: &Path) -> Option<(PathBuf, String)> {
    let text = item.to_str()?;
    let (file, case) = text.rsplit_once(CASE_SEPARATOR)?;
    let file = PathBuf::from(file);
    file.extension()
        .is_some_and(|e| e == EXTENSION)
        .then(|| (file, case.to_owned()))
}

/// A corpus item's stages when it is dEQP's: a case's program, or why a library file has none.
pub fn sources(item: &Path) -> Option<Result<Vec<(Stage, String)>, String>> {
    if let Some((file, case)) = split(item) {
        return Some(program(&file, &case));
    }
    item.extension().is_some_and(|e| e == EXTENSION).then(|| {
        let text = std::fs::read_to_string(item).map_err(|e| format!("{}: {e}", item.display()))?;
        cases(&text)
            .map_err(|e| format!("dEQP's library file: {e}"))
            .and_then(|_| Err("a library file, whose cases are the corpus items".into()))
    })
}

/// The corpus items of a library file: one a program dEQP makes of a case, named as dEQP names
/// it; the file itself when it cannot be read, so that the run names why.
pub fn items(file: &Path) -> Vec<PathBuf> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let Ok(cases) = cases(&text) else {
        return vec![file.to_owned()];
    };
    let item = |name: &str| PathBuf::from(format!("{}{CASE_SEPARATOR}{name}", file.display()));
    cases
        .iter()
        .flat_map(|c| {
            if c.both.is_some() {
                vec![
                    item(&format!("{}{VERTEX_SUFFIX}", c.name)),
                    item(&format!("{}{FRAGMENT_SUFFIX}", c.name)),
                ]
            } else {
                vec![item(&c.name)]
            }
        })
        .collect()
}

/// The program of one case (`split`): its stages' sources, specialized as dEQP does; or why it is
/// not one the corpus draws.
pub fn program(file: &Path, name: &str) -> Result<Vec<(Stage, String)>, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    case_program(&text, name)
}

/// The program of the case `name` of a library file's text.
pub fn case_program(text: &str, name: &str) -> Result<Vec<(Stage, String)>, String> {
    let cases = cases(text).map_err(|e| format!("dEQP's library file: {e}"))?;
    let (case, only) = cases
        .iter()
        .find_map(|c| {
            if c.both.is_some() {
                let rest = name.strip_prefix(c.name.as_str())?;
                [
                    (VERTEX_SUFFIX, Stage::Vertex),
                    (FRAGMENT_SUFFIX, Stage::Fragment),
                ]
                .into_iter()
                .find(|(suffix, _)| rest == *suffix)
                .map(|(_, stage)| (c, Some(stage)))
            } else {
                (c.name == name).then_some((c, None))
            }
        })
        .ok_or_else(|| format!("no case {name}"))?;
    let expect = case.expect.as_deref().unwrap_or(BUILT[0]);
    if !BUILT.contains(&expect) {
        return Err(format!("dEQP expects this case to fail: {expect}"));
    }
    if case.pipeline {
        return Err("a pipeline of separable programs, which the corpus does not draw".into());
    }
    if let Some(stage) = case.others.first() {
        return Err(format!("stage {stage} is not in the common form"));
    }
    let version = case.version.unwrap_or(DEFAULT_VERSION);
    match (&case.both, only) {
        (Some(source), Some(Stage::Vertex)) => vertex_case(case, version, source),
        (Some(source), Some(_)) => fragment_case(case, version, source),
        _ => complete_case(case, version),
    }
}

fn of(case: &Case, storage: Storage) -> impl Iterator<Item = &Value> {
    case.values.iter().filter(move |v| v.storage == storage)
}

/// A `both` source as a vertex program (`specializeVertexShader`), and a fragment program that
/// writes what it passes on to the colour targets (where dEQP compares them).
fn vertex_case(
    case: &Case,
    version: Version,
    source: &str,
) -> Result<Vec<(Stage, String)>, String> {
    let stage_in = |rest: String| Qualifier::VertexInput.declared(version, &rest);
    let stage_out = |rest: String| Qualifier::VertexOutput.declared(version, &rest);
    let mut declarations = stage_in(format!("highp vec4 {POSITION_ATTRIBUTE};\n"));
    let mut setup = String::new();
    let mut output = format!("gl_Position = {POSITION_ATTRIBUTE};\n");
    for v in of(case, Storage::Input) {
        let (name, float) = (&v.name, v.ty.float_type().name());
        if v.ty.kind == Kind::Float {
            declarations += &stage_in(format!("{float} {name};\n"));
        } else {
            let ty = v.ty.name();
            declarations += &stage_in(format!("{float} {ATTRIBUTE_PREFIX}{name};\n"));
            setup += &format!("{ty} {name} = {ty}({ATTRIBUTE_PREFIX}{name});\n");
        }
    }
    let mut passed = Vec::new();
    for v in of(case, Storage::Output) {
        let (name, float) = (&v.name, v.ty.float_type().name());
        if v.ty.kind == Kind::Float {
            declarations += &stage_out(format!("{float} {name};\n"));
            passed.push((name.clone(), v.ty));
        } else {
            let ty = v.ty.name();
            declarations += &stage_out(format!("{float} {VARYING_PREFIX}{name};\n{ty} {name};\n"));
            output += &format!("{VARYING_PREFIX}{name} = {float}({name});\n");
            passed.push((format!("{VARYING_PREFIX}{name}"), v.ty.float_type()));
        }
    }
    let params = [
        ("DECLARATIONS", declarations),
        ("SETUP", setup),
        ("OUTPUT", output),
        ("POSITION_FRAG_COLOR", "gl_Position".to_owned()),
    ];
    let vertex = specialized(source, &params, case, VERTEX, version)?;
    let stage_in = |rest: String| Qualifier::FragmentInput.declared(version, &rest);
    let mut fragment = format!(
        "{}\nprecision highp float;\nprecision highp int;\n",
        version.directive()
    );
    for (name, ty) in &passed {
        fragment += &stage_in(format!("{} {name};\n", ty.name()));
    }
    let (outputs, writes) = written(version, &passed)?;
    fragment += &format!("{outputs}void main()\n{{\n{writes}}}\n");
    Ok(vec![(Stage::Vertex, vertex), (Stage::Fragment, fragment)])
}

/// A `both` source as a fragment program (`specializeFragmentShader`), its outputs written to the
/// colour targets; the corpus's feeder gives its inputs.
fn fragment_case(
    case: &Case,
    version: Version,
    source: &str,
) -> Result<Vec<(Stage, String)>, String> {
    let stage_in = |rest: String| Qualifier::FragmentInput.declared(version, &rest);
    let mut declarations = String::new();
    let mut setup = String::new();
    for v in of(case, Storage::Input) {
        let (name, float) = (&v.name, v.ty.float_type().name());
        if v.ty.kind == Kind::Float {
            declarations += &stage_in(format!("{float} {name};\n"));
        } else {
            let ty = v.ty.name();
            declarations += &stage_in(format!("{float} {VARYING_PREFIX}{name};\n"));
            setup += &format!("{ty} {name} = {ty}({VARYING_PREFIX}{name});\n");
        }
    }
    let (outputs, writes) = fragment_outputs(case, version)?;
    declarations += &outputs;
    let params = [
        ("DECLARATIONS", declarations),
        ("SETUP", setup),
        ("OUTPUT", writes),
        ("POSITION_FRAG_COLOR", colour(version).to_owned()),
    ];
    Ok(vec![(
        Stage::Fragment,
        specialized(source, &params, case, FRAGMENT, version)?,
    )])
}

/// A case of its own vertex and fragment sources (`specializeProgramSources`).
fn complete_case(case: &Case, version: Version) -> Result<Vec<(Stage, String)>, String> {
    let ([vertex], [fragment]) = (case.vertex.as_slice(), case.fragment.as_slice()) else {
        return Err("not one vertex and one fragment source".into());
    };
    let stage_in = |rest: String| Qualifier::VertexInput.declared(version, &rest);
    let mut declarations = stage_in(format!("highp vec4 {POSITION_ATTRIBUTE};\n"));
    let mut setup = String::new();
    for v in of(case, Storage::Input) {
        let (name, ty) = (&v.name, v.ty.name());
        if v.ty.kind == Kind::Float {
            declarations += &stage_in(format!("{ty} {name};\n"));
        } else {
            let float = v.ty.float_type().name();
            declarations += &stage_in(format!("{float} {ATTRIBUTE_PREFIX}{name};\n"));
            setup += &format!("{ty} {name} = {ty}({ATTRIBUTE_PREFIX}{name});\n");
        }
    }
    let uniforms = uniform_declarations(case);
    declarations += &uniforms;
    let vertex_params = [
        ("VERTEX_DECLARATIONS", declarations),
        ("VERTEX_SETUP", setup),
        (
            "VERTEX_OUTPUT",
            format!("gl_Position = {POSITION_ATTRIBUTE};\n"),
        ),
    ];
    let (mut outputs, writes) = fragment_outputs(case, version)?;
    outputs += &uniforms;
    let fragment_params = [
        ("FRAGMENT_DECLARATIONS", outputs),
        ("FRAGMENT_OUTPUT", writes),
        ("FRAG_COLOR", colour(version).to_owned()),
    ];
    Ok(vec![
        (
            Stage::Vertex,
            specialized(vertex, &vertex_params, case, VERTEX, version)?,
        ),
        (
            Stage::Fragment,
            specialized(fragment, &fragment_params, case, FRAGMENT, version)?,
        ),
    ])
}

/// The uniforms of the values block that are not structure members (`generateUniformDeclarations`).
fn uniform_declarations(case: &Case) -> String {
    of(case, Storage::Uniform)
        .filter(|v| !v.name.contains(GROUP_SEPARATOR))
        .map(|v| format!("uniform {} {};\n", v.ty.name(), v.name))
        .collect()
}

/// The colour a fragment program writes, by version.
fn colour(version: Version) -> &'static str {
    if version.in_out() {
        FRAG_COLOR
    } else {
        LEGACY_FRAG_COLOR
    }
}

/// A fragment program's outputs of the values block: each declared, as dEQP does, and written to
/// the colour targets.
fn fragment_outputs(case: &Case, version: Version) -> Result<(String, String), String> {
    let outputs: Vec<(String, ValueType)> = of(case, Storage::Output)
        .map(|v| (v.name.clone(), v.ty))
        .collect();
    let declared: String = outputs
        .iter()
        .map(|(name, ty)| format!("{} {name};\n", ty.name()))
        .collect();
    let (targets, writes) = written(version, &outputs)?;
    Ok((declared + &targets, writes))
}

/// The colour targets that hold `values`' components, and the code writing them: float
/// components four a target from the first (dEQP's colour, then `wc_output_<n>`), the others as
/// their bits in `uvec4` targets after them (as floats in GLSL ES 1.00, which has no integer
/// targets and one colour). With no float component the colour is white, as dEQP writes it.
fn written(version: Version, values: &[(String, ValueType)]) -> Result<(String, String), String> {
    let mut floats = Vec::new();
    let mut bits = Vec::new();
    for (name, ty) in values {
        for c in ty.components(name) {
            match ty.kind {
                Kind::Float => floats.push(c),
                _ if !version.in_out() => floats.push(format!("float({c})")),
                _ => bits.push(format!("uint({c})")),
            }
        }
    }
    let float_targets: Vec<&[String]> = floats.chunks(TARGET_COMPONENTS).collect();
    let bit_targets: Vec<&[String]> = bits.chunks(TARGET_COMPONENTS).collect();
    let limit = if version.in_out() { DRAW_BUFFERS } else { 1 };
    if float_targets.len().max(1) + bit_targets.len() > limit {
        return Err(format!(
            "outputs of {} components, more than the colour targets hold",
            floats.len() + bits.len()
        ));
    }
    let padded = |parts: &[String], zero: &str| {
        let mut parts = parts.to_vec();
        parts.resize(TARGET_COMPONENTS, zero.to_owned());
        parts.join(", ")
    };
    let mut declarations = String::new();
    if version.in_out() {
        declarations += &format!("layout(location = 0) out highp vec4 {FRAG_COLOR};\n");
    }
    let mut writes = match float_targets.first() {
        Some(first) => format!("{} = vec4({});\n", colour(version), padded(first, "0.0")),
        None => format!("{} = vec4(1.0);\n", colour(version)),
    };
    let rest = float_targets
        .iter()
        .skip(1)
        .map(|parts| ("vec4", parts, "0.0"))
        .chain(bit_targets.iter().map(|parts| ("uvec4", parts, "0u")));
    for (location, (kind, parts, zero)) in (1..).zip(rest) {
        declarations +=
            &format!("layout(location = {location}) out highp {kind} {OUTPUT_PREFIX}{location};\n");
        writes += &format!(
            "{OUTPUT_PREFIX}{location} = {kind}({});\n",
            padded(parts, zero)
        );
    }
    Ok((declarations, writes))
}

/// A source with dEQP's template values put in, the case's version where it names none, and the
/// extensions the case requires in its stage after its version line.
fn specialized(
    source: &str,
    params: &[(&str, String)],
    case: &Case,
    stage: &str,
    version: Version,
) -> Result<String, String> {
    const OPEN: &str = "${";
    const CLOSE: char = '}';
    // A value's modifier (`tcu::StringTemplate`): its lines joined into one.
    const MODIFIER: char = ':';
    const SINGLE_LINE: &str = "single-line";
    let mut out = String::new();
    let mut rest = source;
    while let Some(at) = rest.find(OPEN) {
        out += &rest[..at];
        let after = &rest[at + OPEN.len()..];
        let end = after
            .find(CLOSE)
            .ok_or("a template value without its end")?;
        let (key, single_line) = match after[..end].split_once(MODIFIER) {
            Some((key, SINGLE_LINE)) => (key, true),
            Some(_) => return Err(format!("the template modifier of {}", &after[..end])),
            None => (&after[..end], false),
        };
        let (_, value) = params
            .iter()
            .find(|(k, _)| *k == key)
            .ok_or_else(|| format!("the template value {key} is not made here"))?;
        if single_line {
            out += &value.replace('\n', " ");
        } else {
            out += value;
        }
        rest = &after[end + CLOSE.len_utf8()..];
    }
    out += rest;
    let extensions: String = case
        .requirements
        .iter()
        .filter(|r| {
            r.stages
                .as_ref()
                .is_none_or(|s| s.iter().any(|s| s == stage))
        })
        .map(|r| format!("{EXTENSION_DIRECTIVE} {} : require\n", r.alternatives[0]))
        .collect();
    // A source without a version is taken at the case's, as dEQP's context takes it.
    if !out.contains(VERSION_DIRECTIVE) {
        out = format!("{}\n{out}", version.directive());
    }
    if extensions.is_empty() {
        return Ok(out);
    }
    Ok(match out.find(VERSION_DIRECTIVE) {
        Some(at) => {
            let line_end = out[at..].find('\n').map_or(out.len(), |n| at + n + 1);
            format!("{}{extensions}{}", &out[..line_end], &out[line_end..])
        }
        None => extensions + &out,
    })
}
