//! Small-step, deterministic semantics. Nondeterminism comes from enabled scheduling choices.
use crate::{
    model::{Instruction, Program},
    syntax::*,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Int(i64),
    String(String),
    Unit,
    Variant(String, Vec<Value>),
    Record(String, BTreeMap<String, Value>),
    List(Vec<Value>),
    Request(usize),
    Actor(String),
    Rows(String),
}
impl Value {
    pub fn none() -> Self {
        Self::Variant("None".into(), vec![])
    }
    pub fn some(v: Self) -> Self {
        Self::Variant("Some".into(), vec![v])
    }
    pub fn bool(&self, span: Span) -> Result<bool> {
        if let Self::Bool(v) = self {
            Ok(*v)
        } else {
            Err(Error::new(span, "expected Boolean value"))
        }
    }
}
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let joined = |xs: &[Value]| {
            xs.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self {
            Self::Bool(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v:?}"),
            Self::Unit => write!(f, "()"),
            Self::Variant(n, xs) if xs.is_empty() => write!(f, "{n}"),
            Self::Variant(n, xs) => write!(f, "{n}({})", joined(xs)),
            Self::Record(n, fs) => write!(
                f,
                "{n} {{ {} }}",
                fs.iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::List(xs) => write!(f, "[{}]", joined(xs)),
            Self::Request(i) => write!(f, "request #{i}"),
            Self::Actor(name) => write!(f, "actor {name}"),
            Self::Rows(n) => write!(f, "{n}.rows"),
        }
    }
}
pub type Env = BTreeMap<String, Value>;
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Unaccepted,
    Ready(usize),
    Pending {
        table: String,
        method: String,
        args: Vec<Value>,
        bind: Option<String>,
        next: usize,
        pc: usize,
    },
    Done,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Continuation {
    pub env: Env,
    pub bind: Option<String>,
    pub next: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Frame {
    pub input: Value,
    pub env: Env,
    pub phase: Phase,
    pub response: Value,
    pub stack: Vec<Continuation>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct State {
    pub actors: BTreeMap<String, Value>,
    pub tables: BTreeMap<String, Vec<Value>>,
    pub frames: Vec<Frame>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Action {
    pub id: String,
    pub description: String,
    pub span: Span,
    pub fair: bool,
}
#[derive(Clone, Debug)]
pub struct Step {
    pub action: Action,
    pub state: State,
}

impl Program {
    pub fn initial(&self) -> Result<State> {
        let mut s = State {
            tables: self.tables.keys().map(|n| (n.clone(), vec![])).collect(),
            ..State::default()
        };
        for actor in self.actors.values() {
            if let Some((_, initial)) = &actor.state {
                let value = self.eval(initial, &Env::new(), &s)?;
                self.check_value(&value, initial.span)?;
                s.actors.insert(actor.name.clone(), value);
            }
        }
        for (n, xs) in &self.check.init {
            let mut rows = vec![];
            for x in xs {
                let v = self.eval(x, &Env::new(), &s)?;
                self.check_value(&v, x.span)?;
                rows.push(v);
            }
            rows.sort();
            if !self.valid_rows(n, &rows) {
                return Err(Error::new(
                    self.check.span,
                    format!("initial constraint violation in {n}"),
                ));
            }
            s.tables.insert(n.clone(), rows);
        }
        for i in &self.check.inputs {
            let input = self.eval(&i.value, &Env::new(), &s)?;
            self.check_value(&input, i.span)?;
            s.frames.push(Frame {
                input,
                env: Env::new(),
                phase: Phase::Unaccepted,
                response: Value::none(),
                stack: vec![],
            });
        }
        Ok(s)
    }
    pub fn check_value(&self, v: &Value, span: Span) -> Result<()> {
        match v {
            Value::Int(_) | Value::String(_) => {
                let name = if matches!(v, Value::Int(_)) {
                    "Int"
                } else {
                    "String"
                };
                let xs =
                    self.check.domains.get(name).ok_or_else(|| {
                        Error::new(span, format!("missing finite domain for {name}"))
                    })?;
                let empty = State::default();
                let values = xs
                    .iter()
                    .map(|x| self.eval(x, &Env::new(), &empty))
                    .collect::<Result<Vec<_>>>()?;
                if !values.contains(v) {
                    return Err(Error::new(
                        span,
                        format!("LIMIT: value {v:?} escapes domain {name}"),
                    ));
                }
            }
            Value::Variant(_, xs) | Value::List(xs) => {
                for x in xs {
                    self.check_value(x, span)?;
                }
            }
            Value::Record(_, fs) => {
                for x in fs.values() {
                    self.check_value(x, span)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn domain(&self, name: &str, depth: usize) -> Result<Vec<Value>> {
        if depth > 24 {
            return Err(Error::new(
                Span::default(),
                "recursive or excessively nested value domain is unsupported",
            ));
        }
        if let Some(a) = self.aliases.get(name) {
            return self.domain(a, depth + 1);
        }
        if name == "Bool" {
            return Ok(vec![Value::Bool(false), Value::Bool(true)]);
        }
        if name == "unit" {
            return Ok(vec![Value::Unit]);
        }
        if let Some(xs) = self.check.domains.get(name) {
            let s = State::default();
            return xs.iter().map(|x| self.eval(x, &Env::new(), &s)).collect();
        }
        let mut out = vec![];
        for (n, c) in self.constructors.iter().filter(|(_, c)| c.ty == name) {
            let fields: Vec<_> = c.fields.keys().cloned().collect();
            let ts: Vec<_> = if c.fields.is_empty() {
                c.payload.iter().collect()
            } else {
                c.fields.values().collect()
            };
            let mut product = vec![vec![]];
            for t in ts {
                let values = self.type_domain(t, depth + 1)?;
                let mut next = vec![];
                for prefix in &product {
                    for v in &values {
                        if next.len() >= 4096 {
                            return Err(Error::new(
                                Span::default(),
                                "finite domain product exceeds elaboration limit (4096)",
                            ));
                        }
                        let mut xs = prefix.clone();
                        xs.push(v.clone());
                        next.push(xs);
                    }
                }
                product = next;
            }
            for xs in product {
                out.push(if fields.is_empty() {
                    Value::Variant(n.clone(), xs)
                } else {
                    Value::Record(n.clone(), fields.iter().cloned().zip(xs).collect())
                });
            }
            if out.len() > 4096 {
                return Err(Error::new(
                    Span::default(),
                    "finite domain exceeds elaboration limit (4096)",
                ));
            }
        }
        if out.is_empty() && !self.model.types.iter().any(|d| d.name == name) {
            return Err(Error::new(
                Span::default(),
                format!("missing finite domain for {name}"),
            ));
        }
        Ok(out)
    }
    fn type_domain(&self, t: &Type, depth: usize) -> Result<Vec<Value>> {
        if depth > 24 {
            return Err(Error::new(
                Span::default(),
                "recursive domain is unsupported",
            ));
        }
        if t.name == "Option" && t.args.len() == 1 {
            let mut v = vec![Value::none()];
            v.extend(
                self.type_domain(&t.args[0], depth + 1)?
                    .into_iter()
                    .map(Value::some),
            );
            Ok(v)
        } else if !t.args.is_empty() {
            Err(Error::new(
                Span::default(),
                "enumeration of this generic domain is not supported",
            ))
        } else {
            self.domain(&t.name, depth + 1)
        }
    }
    pub fn collection(&self, v: Value, s: &State, span: Span) -> Result<Vec<Value>> {
        match v {
            Value::List(xs) => Ok(xs),
            Value::Rows(n) => Ok(s.tables[&n].clone()),
            _ => Err(Error::new(span, "expected finite collection")),
        }
    }
    pub fn eval(&self, e: &Expr, env: &Env, s: &State) -> Result<Value> {
        use ExprKind::*;
        if let Some(actor) = e
            .path()
            .and_then(|path| path.strip_suffix(".state").map(str::to_owned))
            && self.actors.contains_key(&actor)
        {
            return s
                .actors
                .get(&actor)
                .cloned()
                .ok_or_else(|| Error::new(e.span, "actor has no initialized state"));
        }
        if let Some(path) = e.path()
            && path.ends_with(".rows")
        {
            let table = path.trim_end_matches(".rows");
            if self.tables.contains_key(table) {
                return Ok(Value::Rows(table.into()));
            }
        }
        let ev = |x: &Expr| self.eval(x, env, s);
        match &e.kind {
            Bool(b) => Ok(Value::Bool(*b)),
            Int(i) => Ok(Value::Int(*i)),
            String(x) => Ok(Value::String(x.clone())),
            Unit => Ok(Value::Unit),
            Name(n) => {
                if let Some(v) = env.get(n) {
                    return Ok(v.clone());
                }
                if n == "None" {
                    return Ok(Value::none());
                }
                if self.constructors.contains_key(n) {
                    return Ok(Value::Variant(n.clone(), vec![]));
                }
                Ok(Value::List(self.domain(n, 0)?))
            }
            Record(n, fs) => Ok(Value::Record(
                n.clone(),
                fs.iter()
                    .map(|(n, x)| Ok((n.clone(), ev(x)?)))
                    .collect::<Result<_>>()?,
            )),
            List(xs) => Ok(Value::List(xs.iter().map(ev).collect::<Result<_>>()?)),
            Field(x, n) => match ev(x)? {
                Value::Actor(actor) if n == "state" => s
                    .actors
                    .get(&actor)
                    .cloned()
                    .ok_or_else(|| Error::new(e.span, "actor capability has no state")),
                Value::Record(_, fs) => fs
                    .get(n)
                    .cloned()
                    .ok_or_else(|| Error::new(e.span, "missing record field")),
                Value::Request(i) => {
                    let f = s
                        .frames
                        .get(i)
                        .ok_or_else(|| Error::new(e.span, "invalid request slot"))?;
                    match n.as_str() {
                        "input" => Ok(f.input.clone()),
                        "response" => Ok(f.response.clone()),
                        "accepted" => Ok(Value::Bool(!matches!(f.phase, Phase::Unaccepted))),
                        "completed" => Ok(Value::Bool(matches!(f.phase, Phase::Done))),
                        _ => Err(Error::new(e.span, "unknown request field")),
                    }
                }
                _ => Err(Error::new(e.span, "cannot inspect field")),
            },
            Call(f, args) => {
                let path = f.path().unwrap_or_default();
                if let Some(function) = self.functions.get(&path) {
                    if self.effects[&path].suspends_or_writes() {
                        return Err(Error::new(
                            e.span,
                            "internal: effectful function reached pure evaluation",
                        ));
                    }
                    let values = args.iter().map(ev).collect::<Result<Vec<_>>>()?;
                    let mut locals = function
                        .params
                        .iter()
                        .zip(values)
                        .map(|((name, _), v)| (name.clone(), v))
                        .collect();
                    return self.eval_function_body(&function.body, &mut locals, s);
                }
                if path == "requests" {
                    let h = args[0].path().unwrap_or_default();
                    return Ok(Value::List(
                        self.check
                            .inputs
                            .iter()
                            .enumerate()
                            .filter(|(_, x)| x.handler == h)
                            .map(|(i, _)| Value::Request(i))
                            .collect(),
                    ));
                }
                if ["Some", "Ok", "Err"].contains(&path.as_str())
                    || self.constructors.contains_key(&path)
                {
                    return Ok(Value::Variant(
                        path,
                        args.iter().map(ev).collect::<Result<_>>()?,
                    ));
                }
                if let ExprKind::Field(receiver, method) = &f.kind
                    && method == "contains_key"
                {
                    let Value::Rows(n) = ev(receiver)? else {
                        return Err(Error::new(e.span, "expected rows view"));
                    };
                    let key = ev(&args[0])?;
                    return Ok(Value::Bool(
                        s.tables[&n]
                            .iter()
                            .any(|row| self.key(&n, row) == Some(&key)),
                    ));
                }
                Err(Error::new(
                    e.span,
                    "effectful or unsupported operation in pure evaluator",
                ))
            }
            Unary(op, x) => {
                let v = ev(x)?;
                match (op.as_str(), v) {
                    ("!" | "not", Value::Bool(b)) => Ok(Value::Bool(!b)),
                    ("-", Value::Int(i)) => i
                        .checked_neg()
                        .map(Value::Int)
                        .ok_or_else(|| Error::new(e.span, "LIMIT: integer overflow")),
                    _ => Err(Error::new(
                        e.span,
                        "cannot evaluate temporal or invalid unary expression as a state predicate",
                    )),
                }
            }
            Binary(op, a, b) => {
                let av = ev(a)?;
                if op == "&&" && !av.bool(a.span)? {
                    return Ok(Value::Bool(false));
                }
                if op == "||" && av.bool(a.span)? {
                    return Ok(Value::Bool(true));
                }
                if op == "implies" && !av.bool(a.span)? {
                    return Ok(Value::Bool(true));
                }
                let bv = ev(b)?;
                match op.as_str() {
                    "==" => Ok(Value::Bool(av == bv)),
                    "!=" => Ok(Value::Bool(av != bv)),
                    "&&" | "||" | "implies" => Ok(Value::Bool(bv.bool(b.span)?)),
                    "+" | "-" | "<" | ">" | "<=" | ">=" => {
                        let (Value::Int(a), Value::Int(b)) = (av, bv) else {
                            return Err(Error::new(e.span, "integer operands required"));
                        };
                        match op.as_str() {
                            "+" => a
                                .checked_add(b)
                                .map(Value::Int)
                                .ok_or_else(|| Error::new(e.span, "LIMIT: integer overflow")),
                            "-" => a
                                .checked_sub(b)
                                .map(Value::Int)
                                .ok_or_else(|| Error::new(e.span, "LIMIT: integer overflow")),
                            "<" => Ok(Value::Bool(a < b)),
                            ">" => Ok(Value::Bool(a > b)),
                            "<=" => Ok(Value::Bool(a <= b)),
                            _ => Ok(Value::Bool(a >= b)),
                        }
                    }
                    _ => Err(Error::new(
                        e.span,
                        "temporal expression is not a state predicate",
                    )),
                }
            }
            Quant {
                all,
                var,
                domain,
                body,
            } => {
                let xs = self.collection(ev(domain)?, s, domain.span)?;
                for v in xs {
                    let mut env = env.clone();
                    env.insert(var.clone(), v);
                    let b = self.eval(body, &env, s)?.bool(body.span)?;
                    if b != *all {
                        return Ok(Value::Bool(!all));
                    }
                }
                Ok(Value::Bool(*all))
            }
        }
    }
    fn eval_function_body(&self, body: &[Stmt], env: &mut Env, s: &State) -> Result<Value> {
        let mut result = Value::Unit;
        for statement in body {
            result = match &statement.kind {
                StmtKind::Let(name, expr) => {
                    let value = self.eval(expr, env, s)?;
                    env.insert(name.clone(), value);
                    Value::Unit
                }
                StmtKind::Expr(expr) => self.eval(expr, env, s)?,
                StmtKind::Match(expr, arms) => {
                    let value = self.eval(expr, env, s)?;
                    let mut result = None;
                    for (pattern, branch) in arms {
                        let mut locals = env.clone();
                        if bind_pattern(pattern, &value, &mut locals) {
                            result = Some(self.eval_function_body(branch, &mut locals, s)?);
                            break;
                        }
                    }
                    result.ok_or_else(|| {
                        Error::new(
                            statement.span,
                            "internal: non-exhaustive pure function match",
                        )
                    })?
                }
            };
        }
        Ok(result)
    }
    pub fn predicate(&self, e: &Expr, env: &Env, s: &State) -> Result<bool> {
        self.eval(e, env, s)?.bool(e.span)
    }
    fn key<'a>(&self, table: &str, row: &'a Value) -> Option<&'a Value> {
        let key = self.tables[table].fields.iter().find(|(_, f)| f.primary)?.0;
        if let Value::Record(_, fs) = row {
            fs.get(key)
        } else {
            None
        }
    }
    fn valid_rows(&self, table: &str, rows: &[Value]) -> bool {
        for (n, f) in &self.tables[table].fields {
            if f.primary || f.unique {
                let mut seen = BTreeSet::new();
                for row in rows {
                    let Value::Record(_, fs) = row else {
                        return false;
                    };
                    let Some(v) = fs.get(n) else {
                        return false;
                    };
                    if !f.primary && *v == Value::none() {
                        continue;
                    }
                    if !seen.insert(v) {
                        return false;
                    }
                }
            }
        }
        true
    }
    fn operation(&self, table: &str, method: &str, args: &[Value], s: &mut State) -> Value {
        let old = s.tables[table].clone();
        if method == "get" {
            return old
                .iter()
                .find(|r| self.key(table, r) == Some(&args[0]))
                .cloned()
                .map(Value::some)
                .unwrap_or_else(Value::none);
        }
        let mut rows = old.clone();
        let mut error = None;
        match method {
            "insert" => rows.push(args[0].clone()),
            "update" | "delete" => {
                if let Some(i) = rows
                    .iter()
                    .position(|r| self.key(table, r) == Some(&args[0]))
                {
                    if method == "update" {
                        rows[i] = args[1].clone();
                    } else {
                        rows.remove(i);
                    }
                } else {
                    error = Some("MissingRow");
                }
            }
            _ => unreachable!("type checked operation"),
        }
        if !self.valid_rows(table, &rows) {
            error = Some("ConstraintViolation");
        }
        if let Some(e) = error {
            Value::Variant("Err".into(), vec![Value::Variant(e.into(), vec![])])
        } else {
            rows.sort();
            s.tables.insert(table.into(), rows);
            Value::Variant("Ok".into(), vec![Value::Unit])
        }
    }
    pub fn successors(&self, s: &State) -> Result<Vec<Step>> {
        let mut steps = vec![Step {
            action: Action {
                id: "stutter".into(),
                description: "stutter".into(),
                span: Span::default(),
                fair: false,
            },
            state: s.clone(),
        }];
        for (i, frame) in s.frames.iter().enumerate() {
            let mut next = s.clone();
            let input = &self.check.inputs[i];
            let h = &self.handlers[&input.handler];
            let (description, span, id, fair) = match &frame.phase {
                Phase::Unaccepted => {
                    let function = &self.functions[&h.function];
                    let mut args = vec![];
                    if self.actors[&h.actor].state.is_some() {
                        args.push(Value::Actor(h.actor.clone()));
                    }
                    args.push(frame.input.clone());
                    next.frames[i].env = function
                        .params
                        .iter()
                        .zip(args)
                        .map(|((name, _), v)| (name.clone(), v))
                        .collect();
                    next.frames[i].phase = Phase::Ready(self.entries[&h.function]);
                    (
                        format!("accept {}({}) as request #{i}", input.handler, frame.input),
                        input.span,
                        format!("accept:{i}"),
                        false,
                    )
                }
                Phase::Ready(pc) => {
                    let (description, span) = self.resume(&mut next, i, *pc)?;
                    (
                        description,
                        span,
                        format!("resume:{i}:{pc}"),
                        self.check.fair,
                    )
                }
                Phase::Pending {
                    table,
                    method,
                    args,
                    bind,
                    next: pc,
                    pc: origin,
                } => {
                    let result = self.operation(table, method, args, &mut next);
                    if let Some(n) = bind {
                        next.frames[i].env.insert(n.clone(), result.clone());
                    }
                    next.frames[i].phase = Phase::Ready(*pc);
                    (
                        format!("request #{i}: {table}.{method} completes with {result}"),
                        self.code[*origin].span,
                        format!("complete:{i}:{origin}"),
                        self.check.fair,
                    )
                }
                Phase::Done => continue,
            };
            steps.push(Step {
                action: Action {
                    id,
                    description,
                    span,
                    fair,
                },
                state: next,
            });
        }
        Ok(steps)
    }
    fn resume(&self, s: &mut State, i: usize, mut pc: usize) -> Result<(String, Span)> {
        // Function call graphs are acyclic and have a checked expansion budget.
        for _ in 0..=20_000 {
            let code = &self.code[pc];
            match &code.instruction {
                Instruction::End | Instruction::Return(_) => {
                    let value = if let Instruction::Return(expr) = &code.instruction {
                        self.eval(expr, &s.frames[i].env, s)?
                    } else {
                        Value::Unit
                    };
                    self.check_value(&value, code.span)?;
                    if let Some(continuation) = s.frames[i].stack.pop() {
                        s.frames[i].env = continuation.env;
                        if let Some(name) = continuation.bind {
                            s.frames[i].env.insert(name, value);
                        }
                        pc = continuation.next;
                    } else {
                        s.frames[i].phase = Phase::Done;
                        s.frames[i].response = Value::some(value.clone());
                        let verb = if self.check.semantics == "cf-core-v0" {
                            "responds"
                        } else {
                            "returns"
                        };
                        return Ok((format!("request #{i} {verb} {value}"), code.span));
                    }
                }
                Instruction::Match { value, arms } => {
                    let v = self.eval(value, &s.frames[i].env, s)?;
                    let mut matched = None;
                    for (pat, target) in arms {
                        let mut env = s.frames[i].env.clone();
                        if bind_pattern(pat, &v, &mut env) {
                            matched = Some((*target, env));
                            break;
                        }
                    }
                    let Some((target, env)) = matched else {
                        return Err(Error::new(
                            code.span,
                            "internal: non-exhaustive lowered match",
                        ));
                    };
                    s.frames[i].env = env;
                    pc = target;
                }
                Instruction::Let { value, next, .. } | Instruction::Eval { value, next } => {
                    let bind = if let Instruction::Let { name, .. } = &code.instruction {
                        Some(name.clone())
                    } else {
                        None
                    };
                    if let ExprKind::Call(f, args) = &value.kind {
                        let path = f.path().unwrap_or_default();
                        if let Some(function) = self.functions.get(&path)
                            && self.effects[&path].suspends_or_writes()
                        {
                            let values = args
                                .iter()
                                .map(|e| self.eval(e, &s.frames[i].env, s))
                                .collect::<Result<Vec<_>>>()?;
                            for v in &values {
                                self.check_value(v, value.span)?;
                            }
                            let locals = function
                                .params
                                .iter()
                                .zip(values)
                                .map(|((name, _), v)| (name.clone(), v))
                                .collect();
                            let saved = std::mem::replace(&mut s.frames[i].env, locals);
                            s.frames[i].stack.push(Continuation {
                                env: saved,
                                bind,
                                next: *next,
                            });
                            pc = self.entries[&path];
                            continue;
                        }
                        if let ExprKind::Field(receiver, method) = &f.kind
                            && method == "set"
                        {
                            let Value::Actor(actor) = self.eval(receiver, &s.frames[i].env, s)?
                            else {
                                return Err(Error::new(
                                    value.span,
                                    "internal: set without an actor capability",
                                ));
                            };
                            let updated = self.eval(&args[0], &s.frames[i].env, s)?;
                            self.check_value(&updated, value.span)?;
                            s.actors.insert(actor, updated);
                            if let Some(name) = bind {
                                s.frames[i].env.insert(name, Value::Unit);
                            }
                            pc = *next;
                            continue;
                        }
                        if let Some((table, method)) = path.rsplit_once('.')
                            && self.tables.contains_key(table)
                        {
                            let args = args
                                .iter()
                                .map(|x| self.eval(x, &s.frames[i].env, s))
                                .collect::<Result<Vec<_>>>()?;
                            for v in &args {
                                self.check_value(v, value.span)?;
                            }
                            s.frames[i].phase = Phase::Pending {
                                table: table.into(),
                                method: method.into(),
                                args: args.clone(),
                                bind,
                                next: *next,
                                pc,
                            };
                            return Ok((
                                format!(
                                    "request #{i} issues {path}({})",
                                    args.iter()
                                        .map(ToString::to_string)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                                value.span,
                            ));
                        }
                    }
                    let v = self.eval(value, &s.frames[i].env, s)?;
                    self.check_value(&v, value.span)?;
                    if let Some(n) = bind {
                        s.frames[i].env.insert(n, v);
                    }
                    pc = *next;
                }
            }
        }
        Err(Error::new(
            Span::default(),
            "LIMIT: local computation exceeded the actor spike's step budget",
        ))
    }
}
fn bind_pattern(p: &Pattern, v: &Value, env: &mut Env) -> bool {
    match p {
        Pattern::Wild => true,
        Pattern::Bind(n) => {
            env.insert(n.clone(), v.clone());
            true
        }
        Pattern::Variant(n, ps) => {
            if let Value::Variant(tag, xs) = v {
                tag == n
                    && ps.len() == xs.len()
                    && ps.iter().zip(xs).all(|(p, v)| bind_pattern(p, v, env))
            } else {
                false
            }
        }
    }
}
