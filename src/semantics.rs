//! Values, finite domains and deterministic local evaluation. Scheduling is in messaging.
use crate::{model::Program, syntax::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    Int(i64),
    String(String),
    Unit,
    Variant(String, Vec<Value>),
    Record(String, BTreeMap<String, Value>),
    List(Vec<Value>),
    Input(usize),
    Message(String, usize),
    Instance(String, usize),
    /// Unforgeable source-level key for dynamically allocated addresses.
    Identity(usize),
    Address(String, Box<Value>),
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
                    .map(|(n, x)| format!("{n}: {x}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::List(xs) => write!(f, "[{}]", joined(xs)),
            Self::Input(i) => write!(f, "input #{i}"),
            Self::Message(actor, i) => write!(f, "{actor} message #{i}"),
            Self::Instance(actor, i) => write!(f, "{actor} instance #{i}"),
            Self::Identity(i) => write!(f, "instance #{i}"),
            Self::Address(name, key) => write!(f, "{name}[{key}]"),
        }
    }
}
pub type Env = BTreeMap<String, Value>;
pub(crate) type Outbox = Vec<(Value, Value, Span)>;
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub payload: Value,
    pub input: Option<usize>,
    pub observation: Option<usize>,
    pub source: Span,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageObservation {
    pub payload: Value,
    pub target: Value,
    pub external: bool,
    pub processed: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub inputs: Vec<ExternalInput>,
    #[serde(with = "mailbox_serde")]
    pub mailboxes: BTreeMap<Value, Vec<Envelope>>,
    pub input_submitted: Vec<bool>,
    pub input_processed: Vec<bool>,
    pub messages: BTreeMap<String, Vec<MessageObservation>>,
    /// Monotone allocation registry. Vector length is the next fresh identity;
    /// stateless instances store unit. Slots are never removed or reused.
    pub spawned: BTreeMap<String, Vec<Value>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalInput {
    pub target: Value,
    pub payload: Value,
    pub source: Span,
}
mod mailbox_serde {
    use super::{Envelope, Value};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
    use std::collections::BTreeMap;
    pub fn serialize<S: Serializer>(
        values: &BTreeMap<Value, Vec<Envelope>>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        values.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<BTreeMap<Value, Vec<Envelope>>, D::Error> {
        let entries = Vec::<(Value, Vec<Envelope>)>::deserialize(deserializer)?;
        let count = entries.len();
        let result: BTreeMap<_, _> = entries.into_iter().collect();
        if count != result.len() {
            return Err(D::Error::custom("duplicate mailbox address"));
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub description: String,
    pub span: Span,
    pub fair: bool,
    pub choices: Vec<crate::choices::Choice>,
    pub spawns: Vec<crate::spawning::Spawn>,
}
#[derive(Clone, Debug)]
pub struct Step {
    pub action: Action,
    pub state: State,
}

impl Program {
    pub fn initial(&self) -> Result<State> {
        self.initial_messages(None)
    }
    pub fn successors(&self, s: &State) -> Result<Vec<Step>> {
        self.message_successors(s, None, None)
    }
    pub fn check_value(&self, v: &Value, span: Span) -> Result<()> {
        let mut remaining = 4096usize;
        let mut pending = vec![(v, 0)];
        while let Some((value, depth)) = pending.pop() {
            if remaining == 0 || depth > 64 {
                return Err(Error::new(
                    span,
                    "LIMIT: value exceeds 4096 nodes or nesting depth 64",
                ));
            }
            remaining -= 1;
            match value {
                Value::Variant(_, xs) | Value::List(xs) => {
                    pending.extend(xs.iter().map(|v| (v, depth + 1)))
                }
                Value::Record(_, fs) => pending.extend(fs.values().map(|v| (v, depth + 1))),
                Value::Address(_, key) => pending.push((key, depth + 1)),
                Value::Int(_) | Value::String(_) => {
                    let name = if matches!(value, Value::Int(_)) {
                        "Int"
                    } else {
                        "String"
                    };
                    if !self.domain(name, 0)?.contains(value) {
                        return Err(Error::new(
                            span,
                            format!("LIMIT: value {value:?} escapes domain {name}"),
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn domain(&self, name: &str, depth: usize) -> Result<Vec<Value>> {
        if depth > 24 {
            return Err(Error::new(
                Span::default(),
                "LIMIT: value domain nesting exceeds 24",
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
            return xs
                .iter()
                .map(|x| self.eval(x, &Env::new(), &State::default()))
                .collect();
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
                                "LIMIT: finite domain product exceeds 4096",
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
                    "LIMIT: finite domain exceeds 4096",
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
                "LIMIT: value domain nesting exceeds 24",
            ));
        }
        if t.name == "Actor" {
            return Err(Error::new(
                Span::default(),
                "cannot enumerate actor references; use instances(Actor)",
            ));
        }
        if t.name == "Option" {
            let mut v = vec![Value::none()];
            v.extend(
                self.type_domain(&t.args[0], depth + 1)?
                    .into_iter()
                    .map(Value::some),
            );
            Ok(v)
        } else if t.name == "Result" {
            let mut v = vec![];
            for (name, ty) in ["Ok", "Err"].into_iter().zip(&t.args) {
                v.extend(
                    self.type_domain(ty, depth + 1)?
                        .into_iter()
                        .map(|x| Value::Variant(name.into(), vec![x])),
                );
            }
            Ok(v)
        } else {
            self.domain(&t.name, depth + 1)
        }
    }
    pub fn eval_domain(&self, expr: &Expr, env: &Env, s: &State) -> Result<Vec<Value>> {
        if let ExprKind::Name(n) = &expr.kind
            && !env.contains_key(n)
            && self.model.types.iter().any(|d| &d.name == n)
        {
            return self.domain(n, 0);
        }
        match self.eval(expr, env, s)? {
            Value::List(xs) => Ok(xs),
            _ => Err(Error::new(expr.span, "expected a finite collection")),
        }
    }
    pub fn eval(&self, e: &Expr, env: &Env, s: &State) -> Result<Value> {
        let _guard = crate::evaluation::EvaluationGuard::enter(e.span)?;
        use ExprKind::*;
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
                Value::Record(_, fs) => fs
                    .get(n)
                    .cloned()
                    .ok_or_else(|| Error::new(e.span, "missing record field")),
                Value::Input(i) => self.input_field(i, n, s, e.span),
                Value::Message(actor, i) => self.message_field(&actor, i, n, s, e.span),
                Value::Instance(actor, i) => self.instance_field(&actor, i, n, s, e.span),
                _ => Err(Error::new(e.span, "cannot inspect field")),
            },
            Call(f, args) => {
                let path = f.path().unwrap_or_default();
                if let Some(function) = self.functions.get(&path) {
                    if self.effects[&path].sends
                        || self.effects[&path].chooses
                        || self.effects[&path].spawns
                    {
                        return Err(Error::new(
                            e.span,
                            "internal: effectful helper reached pure evaluation",
                        ));
                    }
                    let values = args.iter().map(ev).collect::<Result<Vec<_>>>()?;
                    for value in &values {
                        self.check_value(value, e.span)?;
                    }
                    let mut locals = function
                        .params
                        .iter()
                        .zip(values)
                        .map(|((name, _), v)| (name.clone(), v))
                        .collect();
                    return self.eval_body(&function.body, &mut locals, s, None);
                }
                if path == "instances" {
                    let actor = args[0].path().expect("typed instance view");
                    return Ok(Value::List(
                        (0..self.check.spawn_bounds[&actor])
                            .map(|i| Value::Instance(actor.clone(), i))
                            .collect(),
                    ));
                }
                if path == "inputs" || path == "messages" {
                    let actor = args[0].path().unwrap_or_default();
                    return Ok(Value::List(if path == "inputs" {
                        s.inputs
                            .iter()
                            .enumerate()
                            .filter(|(_, input)| matches!(&input.target, Value::Address(name, _) if name == &actor))
                            .map(|(i, _)| Value::Input(i))
                            .collect()
                    } else {
                        (0..self.check.message_bound.expect("typed observation bound"))
                            .map(|i| Value::Message(actor.clone(), i))
                            .collect()
                    }));
                }
                if ["Some", "Ok", "Err"].contains(&path.as_str())
                    || self.constructors.contains_key(&path)
                {
                    return Ok(Value::Variant(
                        path,
                        args.iter().map(ev).collect::<Result<_>>()?,
                    ));
                }
                Err(Error::new(
                    e.span,
                    "effectful or unsupported operation in pure evaluator",
                ))
            }
            Unary(op, x) => match (op.as_str(), ev(x)?) {
                ("!" | "not", Value::Bool(b)) => Ok(Value::Bool(!b)),
                ("-", Value::Int(i)) => i
                    .checked_neg()
                    .map(Value::Int)
                    .ok_or_else(|| Error::new(e.span, "LIMIT: integer overflow")),
                _ => Err(Error::new(
                    e.span,
                    "cannot evaluate temporal or invalid unary expression as a state predicate",
                )),
            },
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
                for v in self.eval_domain(domain, env, s)? {
                    let mut env = env.clone();
                    env.insert(var.clone(), v);
                    if self.eval(body, &env, s)?.bool(body.span)? != *all {
                        return Ok(Value::Bool(!all));
                    }
                }
                Ok(Value::Bool(*all))
            }
        }
    }
    /// The same statement interpreter serves pure helpers, initializers and turns.
    /// Only a turn supplies an outbox; it is never published during evaluation.
    pub(crate) fn eval_body(
        &self,
        body: &[Stmt],
        env: &mut Env,
        s: &State,
        mut turn: Option<&mut crate::choices::Turn<'_>>,
    ) -> Result<Value> {
        let _guard = crate::evaluation::EvaluationGuard::enter(
            body.first().map_or(Span::default(), |s| s.span),
        )?;
        let mut result = Value::Unit;
        for statement in body {
            if let Some(turn) = &turn {
                turn.poll()?;
            }
            result = match &statement.kind {
                StmtKind::Inputs(inputs) => {
                    let turn = turn.as_deref_mut().ok_or_else(|| {
                        Error::new(statement.span, "internal: inputs outside setup")
                    })?;
                    for input in inputs {
                        turn.poll()?;
                        let target = self.eval(&input.target, env, s)?;
                        let payload = self.eval(&input.value, env, s)?;
                        self.check_value(&target, input.span)?;
                        self.check_value(&payload, input.span)?;
                        turn.inputs.push(ExternalInput {
                            target,
                            payload,
                            source: input.span,
                        });
                    }
                    Value::Unit
                }
                StmtKind::Let(name, expr) => {
                    let value = self.eval_statement(expr, env, s, turn.as_deref_mut())?;
                    self.check_value(&value, expr.span)?;
                    env.insert(name.clone(), value);
                    Value::Unit
                }
                StmtKind::Expr(expr) => self.eval_statement(expr, env, s, turn.as_deref_mut())?,
                StmtKind::Match(expr, arms) => {
                    let value = self.eval(expr, env, s)?;
                    self.check_value(&value, expr.span)?;
                    let mut chosen = None;
                    for (pattern, branch) in arms {
                        let mut locals = env.clone();
                        if bind_pattern(pattern, &value, &mut locals) {
                            chosen = Some(self.eval_body(
                                branch,
                                &mut locals,
                                s,
                                turn.as_deref_mut(),
                            )?);
                            break;
                        }
                    }
                    chosen.ok_or_else(|| {
                        Error::new(statement.span, "internal: non-exhaustive match")
                    })?
                }
            };
            self.check_value(&result, statement.span)?;
        }
        Ok(result)
    }
    fn eval_statement(
        &self,
        expr: &Expr,
        env: &Env,
        s: &State,
        turn: Option<&mut crate::choices::Turn<'_>>,
    ) -> Result<Value> {
        if let Some(turn) = turn
            && let ExprKind::Call(target, args) = &expr.kind
        {
            let path = target.path().unwrap_or_default();
            if path == "spawn" {
                let actor = args[0].path().expect("typed spawn definition");
                let values = args[1..]
                    .iter()
                    .map(|arg| self.eval(arg, env, s))
                    .collect::<Result<Vec<_>>>()?;
                return turn.spawn(self, &actor, values, expr.span, s);
            }
            if path == "choose" {
                let [
                    Expr {
                        kind: ExprKind::List(candidates),
                        ..
                    },
                ] = args.as_slice()
                else {
                    return Err(Error::new(expr.span, "internal: invalid choice operand"));
                };
                return turn.choose(self, candidates, expr.span, env, s);
            }
            if path == "send" {
                let address = self.eval(&args[0], env, s)?;
                let message = self.eval(&args[1], env, s)?;
                self.check_value(&message, expr.span)?;
                if !s.mailboxes.contains_key(&address)
                    && !turn.spawns.iter().any(|spawn| spawn.address == address)
                {
                    return Err(Error::new(
                        expr.span,
                        "LIMIT: send target outside finite identity domain",
                    ));
                }
                turn.outbox.push((address, message, expr.span));
                return Ok(Value::Unit);
            }
            if let Some(f) = self.functions.get(&path)
                && (self.effects[&path].sends
                    || self.effects[&path].chooses
                    || self.effects[&path].spawns)
            {
                let values = args
                    .iter()
                    .map(|e| self.eval(e, env, s))
                    .collect::<Result<Vec<_>>>()?;
                for v in &values {
                    self.check_value(v, expr.span)?;
                }
                let mut locals = f
                    .params
                    .iter()
                    .zip(values)
                    .map(|((name, _), value)| (name.clone(), value))
                    .collect();
                turn.calls.push(expr.span);
                let result = self.eval_body(&f.body, &mut locals, s, Some(turn));
                turn.calls.pop();
                return result;
            }
        }
        self.eval(expr, env, s)
    }
    pub fn predicate(&self, e: &Expr, env: &Env, s: &State) -> Result<bool> {
        self.eval(e, env, s)?.bool(e.span)
    }
}
pub(crate) fn bind_pattern(p: &Pattern, v: &Value, env: &mut Env) -> bool {
    match p {
        Pattern::Wild => true,
        Pattern::Bind(n) => {
            env.insert(n.clone(), v.clone());
            true
        }
        Pattern::Variant(n, ps) => {
            if let Value::Variant(m, vs) = v
                && n == m
                && ps.len() == vs.len()
            {
                ps.iter().zip(vs).all(|(p, v)| bind_pattern(p, v, env))
            } else {
                false
            }
        }
    }
}
