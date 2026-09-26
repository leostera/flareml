//! Standalone FML syntax. Unknown constructs fail closed rather than being skipped.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Error {
    pub message: String,
    pub span: Span,
}
impl Error {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExprKind {
    Name(String),
    String(String),
    Int(i64),
    Bool(bool),
    Unit,
    Record(String, BTreeMap<String, Expr>),
    List(Vec<Expr>),
    Field(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Quant {
        all: bool,
        var: String,
        domain: Box<Expr>,
        body: Box<Expr>,
    },
}
impl Expr {
    pub fn path(&self) -> Option<String> {
        match &self.kind {
            ExprKind::Name(n) => Some(n.clone()),
            ExprKind::Field(x, n) => Some(format!("{}.{}", x.path()?, n)),
            _ => None,
        }
    }
    pub fn temporal(&self) -> bool {
        match &self.kind {
            ExprKind::Unary(op, x) => {
                ["always", "eventually", "reachable"].contains(&op.as_str()) || x.temporal()
            }
            ExprKind::Binary(op, a, b) => {
                ["leads_to", "until"].contains(&op.as_str()) || a.temporal() || b.temporal()
            }
            ExprKind::Quant { domain, body, .. } => domain.temporal() || body.temporal(),
            ExprKind::Field(x, _) => x.temporal(),
            ExprKind::Call(f, xs) => f.temporal() || xs.iter().any(Self::temporal),
            ExprKind::Record(_, fs) => fs.values().any(Self::temporal),
            ExprKind::List(xs) => xs.iter().any(Self::temporal),
            _ => false,
        }
    }
}
/// Recognize a static actor path, optionally through a keyed address.
pub fn actor_target(e: &Expr) -> Option<(String, Option<Expr>)> {
    if let Some(path) = e.path() {
        return Some((path, None));
    }
    let ExprKind::Field(address, method) = &e.kind else {
        return None;
    };
    let ExprKind::Call(at, keys) = &address.kind else {
        return None;
    };
    if keys.len() != 1 {
        return None;
    }
    let ExprKind::Field(actor, name) = &at.kind else {
        return None;
    };
    if name != "at" {
        return None;
    }
    let ExprKind::Name(actor) = &actor.kind else {
        return None;
    };
    Some((format!("{actor}.{method}"), Some(keys[0].clone())))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub name: String,
    pub args: Vec<Type>,
}
impl Type {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            args: vec![],
        }
    }
}
#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub payload: Vec<Type>,
    pub fields: BTreeMap<String, Type>,
}
#[derive(Clone, Debug)]
pub struct TypeDecl {
    pub name: String,
    pub variants: Vec<Variant>,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub enum Pattern {
    Wild,
    Bind(String),
    Variant(String, Vec<Pattern>),
}
#[derive(Clone, Debug)]
pub enum StmtKind {
    Let(String, Expr),
    Expr(Expr),
    Match(Expr, Vec<(Pattern, Vec<Stmt>)>),
}
#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub output: Type,
    pub body: Vec<Stmt>,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Actor {
    pub name: String,
    /// A finite, typed identity domain; absent for singleton actors.
    pub key: Option<(String, Type)>,
    /// Owned state is retained between invocations; this does not imply durability.
    pub state: Option<Type>,
    pub handler: String,
    pub initializer: Option<String>,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimKind {
    Invariant,
    Property,
    Cover,
}
#[derive(Clone, Debug)]
pub struct Claim {
    pub kind: ClaimKind,
    pub name: String,
    pub body: Expr,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Input {
    pub actor: String,
    pub key: Option<Expr>,
    pub value: Expr,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Check {
    pub name: String,
    pub inputs: Vec<Input>,
    pub domains: BTreeMap<String, Vec<Expr>>,
    pub fair: bool,
    pub mailbox_bound: Option<usize>,
    /// Optional lifetime observation slots per actor declaration, never reused.
    pub message_bound: Option<usize>,
    pub span: Span,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub types: Vec<TypeDecl>,
    pub functions: Vec<Function>,
    pub actors: Vec<Actor>,
    pub claims: Vec<Claim>,
    pub checks: Vec<Check>,
}

#[derive(Clone, Debug)]
struct Token {
    text: String,
    span: Span,
    string: bool,
}
#[derive(logos::Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
enum Lexeme {
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Name,
    #[regex(r"[0-9]+")]
    Integer,
    #[regex(r#"\"([^\"\\\x00-\x1F]|\\[^\r\n])*\""#)]
    String,
    #[regex(r"->|==|!=|&&|\|\||<=|>=|\.\.|[{}()\[\]:;,|.=<>+!\-]")]
    Symbol,
}
fn lex(source: &str) -> Result<Vec<Token>> {
    use logos::Logos;
    if source.len() > 1_000_000 {
        return Err(Error::new(Span::default(), "source exceeds 1 MB limit"));
    }
    let mut lexer = Lexeme::lexer(source);
    let mut out = vec![];
    while let Some(token) = lexer.next() {
        let range = lexer.span();
        let span = Span {
            start: range.start,
            end: range.end,
        };
        let kind = token.map_err(|_| Error::new(span, "invalid token or unterminated string"))?;
        let string = kind == Lexeme::String;
        let text = if string {
            serde_json::from_str::<String>(lexer.slice())
                .map_err(|e| Error::new(span, format!("invalid string: {e}")))?
        } else {
            lexer.slice().to_owned()
        };
        out.push(Token { text, span, string });
        if out.len() > 100_000 {
            return Err(Error::new(span, "token limit exceeded"));
        }
    }
    out.push(Token {
        text: "<eof>".into(),
        span: Span {
            start: source.len(),
            end: source.len(),
        },
        string: false,
    });
    Ok(out)
}
struct Parser {
    tokens: Vec<Token>,
    i: usize,
    depth: usize,
}
impl Parser {
    fn token(&self) -> &Token {
        &self.tokens[self.i]
    }
    fn at(&self, s: &str) -> bool {
        !self.token().string && self.token().text == s
    }
    fn take(&mut self) -> Token {
        let t = self.token().clone();
        if self.i + 1 < self.tokens.len() {
            self.i += 1;
        }
        t
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.take();
            true
        } else {
            false
        }
    }
    fn err<T>(&self, message: impl Into<String>) -> Result<T> {
        Err(Error::new(self.token().span, message))
    }
    fn expect(&mut self, s: &str) -> Result<()> {
        if self.eat(s) {
            Ok(())
        } else {
            self.err(format!("expected `{s}`, found `{}`", self.token().text))
        }
    }
    fn name(&mut self) -> Result<String> {
        let t = self.token();
        if [
            "type",
            "actor",
            "let",
            "match",
            "property",
            "check",
            "forall",
            "exists",
            "true",
            "false",
            "always",
            "eventually",
            "reachable",
            "leads_to",
            "until",
            "implies",
            "not",
        ]
        .contains(&t.text.as_str())
        {
            return self.err("reserved keyword cannot be used as an identifier");
        }
        if !t.string
            && t.text
                .as_bytes()
                .first()
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        {
            Ok(self.take().text)
        } else {
            self.err("expected identifier")
        }
    }
    fn string(&mut self) -> Result<String> {
        if self.token().string {
            Ok(self.take().text)
        } else {
            self.err("expected quoted string")
        }
    }
    fn path(&mut self) -> Result<String> {
        let mut p = self.name()?;
        while self.eat(".") {
            p.push('.');
            p.push_str(&self.name()?);
        }
        Ok(p)
    }
    fn enter(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > 128 {
            self.err("syntax nesting limit exceeded")
        } else {
            Ok(())
        }
    }
    fn ty(&mut self) -> Result<Type> {
        self.enter()?;
        let name = self.name()?;
        let mut args = vec![];
        if self.eat("<") {
            loop {
                args.push(self.ty()?);
                if !self.eat(",") {
                    break;
                }
            }
            self.expect(">")?;
        }
        self.depth -= 1;
        Ok(Type { name, args })
    }
    fn expr(&mut self, min: u8) -> Result<Expr> {
        self.enter()?;
        let t = self.take();
        let span = t.span;
        let kind = if t.string {
            ExprKind::String(t.text)
        } else if t.text.as_bytes()[0].is_ascii_digit() {
            ExprKind::Int(
                t.text
                    .parse()
                    .map_err(|_| Error::new(span, "integer literal out of range"))?,
            )
        } else {
            match t.text.as_str() {
                "true" => ExprKind::Bool(true),
                "false" => ExprKind::Bool(false),
                "(" => {
                    if self.eat(")") {
                        ExprKind::Unit
                    } else {
                        let e = self.expr(0)?;
                        self.expect(")")?;
                        e.kind
                    }
                }
                "[" => {
                    let mut xs = vec![];
                    while !self.eat("]") {
                        xs.push(self.expr(0)?);
                        if !self.eat(",") {
                            self.expect("]")?;
                            break;
                        }
                    }
                    ExprKind::List(xs)
                }
                "!" | "not" | "-" | "always" | "eventually" | "reachable" => {
                    ExprKind::Unary(t.text, Box::new(self.expr(8)?))
                }
                "forall" | "exists" => {
                    self.expect("(")?;
                    let var = self.name()?;
                    self.expect("in")?;
                    let domain = self.expr(0)?;
                    self.expect(")")?;
                    self.expect("{")?;
                    let body = self.expr(0)?;
                    self.expect("}")?;
                    ExprKind::Quant {
                        all: t.text == "forall",
                        var,
                        domain: Box::new(domain),
                        body: Box::new(body),
                    }
                }
                s if s
                    .as_bytes()
                    .first()
                    .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_') =>
                {
                    // Constructor record braces are distinguished from a following match block.
                    let record = self.at("{")
                        && self.tokens.get(self.i + 2).is_some_and(|t| t.text == ":")
                        && s.as_bytes()[0].is_ascii_uppercase();
                    if record {
                        self.expect("{")?;
                        let mut fields = BTreeMap::new();
                        while !self.eat("}") {
                            let n = self.name()?;
                            self.expect(":")?;
                            let x = self.expr(0)?;
                            if fields.insert(n, x).is_some() {
                                return self.err("duplicate record field");
                            }
                            if !self.eat(",") {
                                self.expect("}")?;
                                break;
                            }
                        }
                        ExprKind::Record(s.into(), fields)
                    } else {
                        ExprKind::Name(s.into())
                    }
                }
                _ => {
                    return Err(Error::new(
                        span,
                        format!("expected expression, found `{}`", t.text),
                    ));
                }
            }
        };
        let mut lhs = Expr { kind, span };
        let mut chain = 0;
        loop {
            chain += 1;
            if chain > 128 {
                return self.err("expression chain nesting limit exceeded");
            }
            if self.at(".") && 10 >= min {
                self.take();
                let n = self.name()?;
                lhs = Expr {
                    kind: ExprKind::Field(Box::new(lhs), n),
                    span,
                };
                continue;
            }
            if self.at("(") && 10 >= min {
                self.take();
                let mut xs = vec![];
                while !self.eat(")") {
                    xs.push(self.expr(0)?);
                    if !self.eat(",") {
                        self.expect(")")?;
                        break;
                    }
                }
                lhs = Expr {
                    kind: ExprKind::Call(Box::new(lhs), xs),
                    span,
                };
                continue;
            }
            let op = self.token().text.clone();
            let bp = match op.as_str() {
                "leads_to" | "until" => 1,
                "implies" => 2,
                "||" => 3,
                "&&" => 4,
                "==" | "!=" => 5,
                "<" | ">" | "<=" | ">=" => 6,
                "+" | "-" => 7,
                _ => break,
            };
            if bp < min {
                break;
            }
            self.take();
            let rhs = self.expr(bp + 1)?;
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                span,
            };
        }
        // Pratt parsing's loop can build a deep left spine without recursive
        // parser calls. Bound the resulting tree too, before recursive passes.
        let mut pending = vec![(&lhs, 0)];
        while let Some((expr, depth)) = pending.pop() {
            if depth > 128 {
                return self.err("expression tree nesting limit exceeded");
            }
            match &expr.kind {
                ExprKind::Field(x, _) | ExprKind::Unary(_, x) => pending.push((x, depth + 1)),
                ExprKind::Binary(_, a, b) => {
                    pending.push((a, depth + 1));
                    pending.push((b, depth + 1));
                }
                ExprKind::Call(f, args) => {
                    pending.push((f, depth + 1));
                    pending.extend(args.iter().map(|x| (x, depth + 1)));
                }
                ExprKind::Record(_, fields) => {
                    pending.extend(fields.values().map(|x| (x, depth + 1)))
                }
                ExprKind::List(xs) => pending.extend(xs.iter().map(|x| (x, depth + 1))),
                ExprKind::Quant { domain, body, .. } => {
                    pending.push((domain, depth + 1));
                    pending.push((body, depth + 1));
                }
                _ => {}
            }
        }
        lhs.span.end = self.tokens[self.i.saturating_sub(1)].span.end;
        self.depth -= 1;
        Ok(lhs)
    }
    fn pattern(&mut self) -> Result<Pattern> {
        self.enter()?;
        let n = self.name()?;
        let out = if n == "_" {
            Pattern::Wild
        } else if n.as_bytes()[0].is_ascii_lowercase() {
            Pattern::Bind(n)
        } else {
            let mut ps = vec![];
            if self.eat("(") {
                loop {
                    ps.push(self.pattern()?);
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(")")?;
            }
            Pattern::Variant(n, ps)
        };
        self.depth -= 1;
        Ok(out)
    }
    fn block(&mut self) -> Result<Vec<Stmt>> {
        self.enter()?;
        self.expect("{")?;
        let mut stmts = vec![];
        while !self.eat("}") {
            let statement = self.stmt()?;
            let binding = matches!(statement.kind, StmtKind::Let(..));
            stmts.push(statement);
            let separator = self.token().span;
            if self.eat(";") {
                if self.at("}") {
                    // A terminated tail discards its value. Lower it to an
                    // explicit unit tail shared by typing and evaluation.
                    stmts.push(Stmt {
                        kind: StmtKind::Expr(Expr {
                            kind: ExprKind::Unit,
                            span: separator,
                        }),
                        span: separator,
                    });
                }
            } else if binding || !self.at("}") {
                let end = self.tokens[self.i - 1].span.end;
                return Err(Error::new(
                    Span { start: end, end },
                    "expected `;` after statement (only a final value may omit it)",
                ));
            }
        }
        self.depth -= 1;
        Ok(stmts)
    }
    fn stmt(&mut self) -> Result<Stmt> {
        self.enter()?;
        let span = self.token().span;
        let kind = if self.eat("let") {
            let n = self.name()?;
            self.expect("=")?;
            StmtKind::Let(n, self.expr(0)?)
        } else if self.eat("match") {
            let e = self.expr(0)?;
            self.expect("{")?;
            let mut arms = vec![];
            while !self.eat("}") {
                if arms.is_empty() {
                    self.eat("|");
                } else {
                    self.expect("|")?;
                }
                let p = self.pattern()?;
                self.expect("->")?;
                let body = if self.at("{") {
                    self.block()?
                } else if self.at("let") {
                    return self.err("a match arm with a binding requires a braced block");
                } else {
                    vec![self.stmt()?]
                };
                arms.push((p, body));
            }
            StmtKind::Match(e, arms)
        } else {
            StmtKind::Expr(self.expr(0)?)
        };
        self.depth -= 1;
        Ok(Stmt { kind, span })
    }
    fn model(&mut self) -> Result<Model> {
        let mut m = Model::default();
        while !self.at("<eof>") {
            let span = self.token().span;
            if self.eat("type") {
                let name = self.name()?;
                self.expect("=")?;
                self.eat("|");
                let mut variants = vec![];
                loop {
                    let n = self.name()?;
                    let mut payload = vec![];
                    let mut fields = BTreeMap::new();
                    if self.eat("(") {
                        loop {
                            payload.push(self.ty()?);
                            if !self.eat(",") {
                                break;
                            }
                        }
                        self.expect(")")?;
                    }
                    if self.eat("{") {
                        while !self.eat("}") {
                            let f = self.name()?;
                            self.expect(":")?;
                            let ty = self.ty()?;
                            if fields.insert(f, ty).is_some() {
                                return self.err("duplicate type field");
                            }
                            if !self.eat(",") {
                                self.expect("}")?;
                                break;
                            }
                        }
                    }
                    variants.push(Variant {
                        name: n,
                        payload,
                        fields,
                    });
                    if !self.eat("|") {
                        break;
                    }
                }
                m.types.push(TypeDecl {
                    name,
                    variants,
                    span,
                });
            } else if self.eat("let") {
                let name = self.name()?;
                self.expect("=")?;
                self.expect("(")?;
                let mut params = vec![];
                while !self.eat(")") {
                    let param = self.name()?;
                    self.expect(":")?;
                    params.push((param, self.ty()?));
                    if !self.eat(",") {
                        self.expect(")")?;
                        break;
                    }
                }
                let output = if self.eat(":") {
                    self.ty()?
                } else {
                    Type::named("unit")
                };
                let body = self.block()?;
                m.functions.push(Function {
                    name,
                    params,
                    output,
                    body,
                    span,
                });
            } else if self.eat("actor") {
                let name = self.name()?;
                let key = if self.eat("(") {
                    let param = self.name()?;
                    self.expect(":")?;
                    let ty = self.ty()?;
                    self.expect(")")?;
                    Some((param, ty))
                } else {
                    None
                };
                self.expect("{")?;
                let mut initializer = None;
                let mut handler = None;
                while !self.eat("}") {
                    let method_span = self.token().span;
                    let method = self.name()?;
                    if method != "init" && method != "handle_message" {
                        return self.err("actor supports only init and handle_message");
                    }
                    self.expect("(")?;
                    let mut params = vec![];
                    while !self.eat(")") {
                        let param = self.name()?;
                        self.expect(":")?;
                        params.push((param, self.ty()?));
                        if !self.eat(",") {
                            self.expect(")")?;
                            break;
                        }
                    }
                    self.expect(":")?;
                    let output = self.ty()?;
                    let body = self.block()?;
                    let function = format!("$actor.{name}.{method}");
                    if method == "init" {
                        if initializer.replace(function.clone()).is_some() {
                            return self.err("duplicate actor initializer");
                        }
                    } else if handler.replace(function.clone()).is_some() {
                        return self.err("duplicate actor message handler");
                    }
                    m.functions.push(Function {
                        name: function,
                        params,
                        output,
                        body,
                        span: method_span,
                    });
                    self.eat(";");
                }
                let handler =
                    handler.ok_or_else(|| Error::new(span, "actor requires handle_message"))?;
                let state = initializer.as_ref().map(|f| {
                    m.functions
                        .iter()
                        .find(|x| &x.name == f)
                        .expect("parsed init")
                        .output
                        .clone()
                });
                m.actors.push(Actor {
                    name,
                    key,
                    state,
                    handler,
                    initializer,
                    span,
                });
            } else if self.eat("property") {
                let kind = ClaimKind::Property;
                let name = self.string()?;
                self.expect("{")?;
                let body = self.expr(0)?;
                self.expect("}")?;
                m.claims.push(Claim {
                    kind,
                    name,
                    body,
                    span,
                });
            } else if self.eat("check") {
                let name = self.name()?;
                self.expect("{")?;
                let mut c = Check {
                    name,
                    inputs: vec![],
                    domains: BTreeMap::new(),
                    fair: false,
                    mailbox_bound: None,
                    message_bound: None,
                    span,
                };
                let mut sections = std::collections::BTreeSet::new();
                while !self.eat("}") {
                    if self.eat("domain") {
                        let n = self.name()?;
                        self.expect("=")?;
                        let first = self.expr(0)?;
                        let xs = if self.eat("..") {
                            let end = self.expr(0)?;
                            match (first.kind, end.kind) {
                                (ExprKind::Int(a), ExprKind::Int(b)) if b >= a && b - a <= 1024 => {
                                    (a..=b)
                                        .map(|i| Expr {
                                            kind: ExprKind::Int(i),
                                            span,
                                        })
                                        .collect()
                                }
                                _ => {
                                    return self
                                        .err("domain range must contain at most 1025 integers");
                                }
                            }
                        } else if let ExprKind::List(xs) = first.kind {
                            xs
                        } else {
                            return self.err("domain requires a literal list or integer range");
                        };
                        if c.domains.insert(n, xs).is_some() {
                            return self.err("duplicate domain");
                        }
                        continue;
                    }
                    let section = self.name()?;
                    if !sections.insert(section.clone()) {
                        return self.err("duplicate check section");
                    }
                    match section.as_str() {
                        "inputs" => {
                            self.expect("{")?;
                            while !self.eat("}") {
                                self.expect("once")?;
                                let span = self.token().span;
                                let request = self.expr(0)?;
                                let ExprKind::Call(target, mut args) = request.kind else {
                                    return self.err("input requires once send(address, message)");
                                };
                                if target.path().as_deref() != Some("send") {
                                    return self.err("input requires once send(address, message)");
                                }
                                let (actor, key, value) = {
                                    if args.len() != 2 {
                                        return self
                                            .err("send input requires an address and one message");
                                    }
                                    let address = args.remove(0);
                                    let (name, key) = match address.kind {
                                        ExprKind::Name(name) => (name, None),
                                        ExprKind::Call(at, mut keys) if keys.len() == 1 => {
                                            let ExprKind::Field(actor, method) = at.kind else {
                                                return self
                                                    .err("send input requires Actor.at(key)");
                                            };
                                            let ExprKind::Name(name) = actor.kind else {
                                                return self
                                                    .err("send input requires Actor.at(key)");
                                            };
                                            if method != "at" {
                                                return self
                                                    .err("send input requires Actor.at(key)");
                                            }
                                            (name, Some(keys.remove(0)))
                                        }
                                        _ => {
                                            return self
                                                .err("send input requires a static actor address");
                                        }
                                    };
                                    (name, key, args.remove(0))
                                };
                                c.inputs.push(Input {
                                    actor,
                                    key,
                                    value,
                                    span,
                                });
                            }
                        }
                        "mailbox_bound" | "message_bound" => {
                            self.expect("=")?;
                            let token = self.take();
                            let n = token.text.parse::<usize>().map_err(|_| {
                                Error::new(
                                    token.span,
                                    format!("{section} requires a positive integer"),
                                )
                            })?;
                            if n == 0 || n > 4096 {
                                return Err(Error::new(
                                    token.span,
                                    format!("{section} must be 1..4096"),
                                ));
                            }
                            if section == "mailbox_bound" {
                                c.mailbox_bound = Some(n);
                            } else {
                                c.message_bound = Some(n);
                            }
                        }
                        "fairness" => {
                            self.expect("{")?;
                            self.expect("weak")?;
                            let p = self.path()?;
                            if p != "runtime.progress" {
                                return self
                                    .err("unsupported fairness family; expected runtime.progress");
                            }
                            self.expect("}")?;
                            c.fair = true;
                        }
                        _ => return self.err(format!("unsupported check section `{section}`")),
                    }
                }
                m.checks.push(c);
            } else {
                return self.err(format!("unsupported declaration `{}`", self.token().text));
            }
            self.eat(";");
        }
        Ok(m)
    }
}
pub fn parse(source: &str) -> Result<Model> {
    Parser {
        tokens: lex(source)?,
        i: 0,
        depth: 0,
    }
    .model()
}
