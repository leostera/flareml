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
    KeyedActor(String, Box<Value>),
    Address(String, Box<Value>),
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
            Self::KeyedActor(name, key) => write!(f, "actor {name}.at({key})"),
            Self::Address(name, key) => write!(f, "{name}.at({key})"),
            Self::Rows(n) => write!(f, "{n}.rows"),
        }
    }
}
pub type Env = BTreeMap<String, Value>;
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Unaccepted,
    Ready(usize),
    Waiting {
        child: usize,
        bind: Option<String>,
        next: usize,
        pc: usize,
    },
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
    pub handler: String,
    pub key: Option<Value>,
    pub parent: Option<usize>,
    pub input: Value,
    pub env: Env,
    pub phase: Phase,
    pub response: Value,
    pub stack: Vec<Continuation>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct State {
    pub actors: BTreeMap<String, Value>,
    #[serde(with = "keyed_actor_serde")]
    pub keyed_actors: BTreeMap<String, BTreeMap<Value, Value>>,
    pub tables: BTreeMap<String, Vec<Value>>,
    pub frames: Vec<Frame>,
}
// JSON objects require string keys. Actor identities are typed Values, so encode
// per-actor state as ordered (identity, state) pairs instead of stringifying keys.
mod keyed_actor_serde {
    use super::Value;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(
        values: &BTreeMap<String, BTreeMap<Value, Value>>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        values
            .iter()
            .map(|(name, instances)| (name, instances.iter().collect::<Vec<_>>()))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<String, BTreeMap<Value, Value>>, D::Error> {
        let values = BTreeMap::<String, Vec<(Value, Value)>>::deserialize(deserializer)?;
        values
            .into_iter()
            .map(|(name, entries)| {
                let count = entries.len();
                let instances: BTreeMap<_, _> = entries.into_iter().collect();
                if instances.len() != count {
                    return Err(D::Error::custom("duplicate keyed actor identity"));
                }
                Ok((name, instances))
            })
            .collect()
    }
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
                if let Some((param, ty)) = &actor.key {
                    let mut keys = self.domain(&ty.name, 0)?;
                    keys.sort();
                    keys.dedup();
                    if keys.is_empty() {
                        return Err(Error::new(
                            actor.span,
                            "LIMIT: actor identity domain is empty or missing",
                        ));
                    }
                    if keys.len() > 4096 {
                        return Err(Error::new(
                            actor.span,
                            "LIMIT: keyed actor domain exceeds 4096 identities",
                        ));
                    }
                    let mut instances = BTreeMap::new();
                    for key in keys {
                        self.check_value(&key, actor.span)?;
                        let value =
                            self.eval(initial, &Env::from([(param.clone(), key.clone())]), &s)?;
                        self.check_value(&value, initial.span)?;
                        instances.insert(key, value);
                    }
                    s.keyed_actors.insert(actor.name.clone(), instances);
                } else {
                    let value = self.eval(initial, &Env::new(), &s)?;
                    self.check_value(&value, initial.span)?;
                    s.actors.insert(actor.name.clone(), value);
                }
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
            let key = i
                .key
                .as_ref()
                .map(|e| self.eval(e, &Env::new(), &s))
                .transpose()?;
            if let Some(key) = &key {
                self.check_value(key, i.span)?;
                if !s.keyed_actors[&self.handlers[&i.handler].actor].contains_key(key) {
                    return Err(Error::new(
                        i.span,
                        "LIMIT: actor key outside finite identity domain",
                    ));
                }
            }
            let input = self.eval(&i.value, &Env::new(), &s)?;
            self.check_value(&input, i.span)?;
            s.frames.push(Frame {
                handler: i.handler.clone(),
                key,
                parent: None,
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
            Value::Address(_, key) => self.check_value(key, span)?,
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
        if let ExprKind::Field(_, field) = &e.kind
            && field == "state"
            && let Some((path, Some(key))) = actor_target(e)
            && let Some(name) = path.strip_suffix(".state")
            && self.actors.contains_key(name)
        {
            let key = self.eval(&key, env, s)?;
            return s
                .keyed_actors
                .get(name)
                .and_then(|instances| instances.get(&key))
                .cloned()
                .ok_or_else(|| {
                    Error::new(e.span, "LIMIT: actor key outside finite identity domain")
                });
        }
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
                Value::KeyedActor(actor, key) if n == "state" => s
                    .keyed_actors
                    .get(&actor)
                    .and_then(|instances| instances.get(&key))
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
                if let ExprKind::Field(actor, method) = &f.kind
                    && method == "at"
                    && let ExprKind::Name(name) = &actor.kind
                    && self.actors.get(name).is_some_and(|a| a.key.is_some())
                {
                    let key = ev(&args[0])?;
                    if !s
                        .keyed_actors
                        .get(name)
                        .is_some_and(|values| values.contains_key(&key))
                    {
                        return Err(Error::new(
                            e.span,
                            "LIMIT: actor key outside finite identity domain",
                        ));
                    }
                    return Ok(Value::Address(name.clone(), Box::new(key)));
                }
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
            let h = &self.handlers[&frame.handler];
            let (description, span, id, fair) = match &frame.phase {
                Phase::Unaccepted => {
                    let function = &self.functions[&h.function];
                    let mut args = vec![];
                    if self.actors[&h.actor].state.is_some() {
                        args.push(match &frame.key {
                            Some(key) => Value::KeyedActor(h.actor.clone(), Box::new(key.clone())),
                            None => Value::Actor(h.actor.clone()),
                        });
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
                        format!("accept {}({}) as request #{i}", frame.handler, frame.input),
                        if frame.parent.is_some() {
                            h.span
                        } else {
                            self.check.inputs[i].span
                        },
                        format!("accept:{i}"),
                        frame.parent.is_some() && self.check.fair,
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
                Phase::Waiting {
                    child,
                    bind,
                    next: pc,
                    pc: origin,
                } => {
                    let callee = &s.frames[*child];
                    if !matches!(callee.phase, Phase::Done) {
                        continue;
                    }
                    let Value::Variant(tag, values) = &callee.response else {
                        return Err(Error::new(
                            self.code[*origin].span,
                            "internal: missing call response",
                        ));
                    };
                    if tag != "Some" || values.len() != 1 {
                        return Err(Error::new(
                            self.code[*origin].span,
                            "internal: invalid call response",
                        ));
                    }
                    let result = values[0].clone();
                    if let Some(n) = bind {
                        next.frames[i].env.insert(n.clone(), result.clone());
                    }
                    next.frames[i].phase = Phase::Ready(*pc);
                    (
                        format!("request #{i} receives reply {result} from request #{child}"),
                        self.code[*origin].span,
                        format!("reply:{i}:{child}:{origin}"),
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
                        if path == "call" {
                            let static_target = actor_target(&args[0])
                                .filter(|(target, _)| self.handlers.contains_key(target));
                            let (handler, key) = if let Some((handler, key_expr)) = static_target {
                                let key = key_expr
                                    .as_ref()
                                    .map(|e| self.eval(e, &s.frames[i].env, s))
                                    .transpose()?;
                                (handler, key)
                            } else if let ExprKind::Field(receiver, method) = &args[0].kind {
                                let Value::Address(actor, key) =
                                    self.eval(receiver, &s.frames[i].env, s)?
                                else {
                                    return Err(Error::new(
                                        value.span,
                                        "internal: call requires an address",
                                    ));
                                };
                                (format!("{actor}.{method}"), Some(*key))
                            } else {
                                return Err(Error::new(value.span, "internal: invalid actor call"));
                            };
                            if let Some(key) = &key {
                                self.check_value(key, value.span)?;
                                if !s.keyed_actors[&self.handlers[&handler].actor].contains_key(key)
                                {
                                    return Err(Error::new(
                                        value.span,
                                        "LIMIT: actor key outside finite identity domain",
                                    ));
                                }
                            }
                            let message = self.eval(&args[1], &s.frames[i].env, s)?;
                            self.check_value(&message, value.span)?;
                            if s.frames.len() >= 64 {
                                return Err(Error::new(
                                    value.span,
                                    "LIMIT: actor call frame capacity (64 total frames)",
                                ));
                            }
                            let child = s.frames.len();
                            s.frames.push(Frame {
                                handler: handler.clone(),
                                key: key.clone(),
                                parent: Some(i),
                                input: message.clone(),
                                env: Env::new(),
                                phase: Phase::Unaccepted,
                                response: Value::none(),
                                stack: vec![],
                            });
                            s.frames[i].phase = Phase::Waiting {
                                child,
                                bind,
                                next: *next,
                                pc,
                            };
                            return Ok((
                                format!(
                                    "request #{i} calls {}({message}) as request #{child}",
                                    key.map_or(handler.clone(), |key| format!(
                                        "{}.at({key}).{}",
                                        self.handlers[&handler].actor,
                                        handler.rsplit('.').next().unwrap_or_default()
                                    ))
                                ),
                                value.span,
                            ));
                        }
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
                            let capability = self.eval(receiver, &s.frames[i].env, s)?;
                            let updated = self.eval(&args[0], &s.frames[i].env, s)?;
                            self.check_value(&updated, value.span)?;
                            match capability {
                                Value::Actor(actor) => {
                                    s.actors.insert(actor, updated);
                                }
                                Value::KeyedActor(actor, key) => {
                                    s.keyed_actors
                                        .get_mut(&actor)
                                        .expect("validated actor")
                                        .insert(*key, updated);
                                }
                                _ => {
                                    return Err(Error::new(
                                        value.span,
                                        "internal: set without an actor capability",
                                    ));
                                }
                            }
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
