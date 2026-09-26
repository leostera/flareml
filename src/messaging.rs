//! Finite asynchronous actor turns for the explicitly versioned actors-v2 profile.
use crate::{
    model::Program,
    semantics::{Action, Env, State, Step, Value, bind_pattern},
    syntax::{Error, Expr, ExprKind, Result, Span, Stmt, StmtKind},
};
use std::collections::BTreeMap;

impl Program {
    pub(crate) fn initial_messages(&self) -> Result<State> {
        let mut s = State::default();
        for actor in self.actors.values() {
            let keys = if let Some((_, ty)) = &actor.key {
                let mut keys = self.domain(&ty.name, 0)?;
                keys.sort();
                keys.dedup();
                if keys.is_empty() || keys.len() > 4096 {
                    return Err(Error::new(
                        actor.span,
                        "LIMIT: actor identity domain is empty or exceeds 4096",
                    ));
                }
                keys
            } else {
                vec![Value::Unit]
            };
            for key in keys {
                self.check_value(&key, actor.span)?;
                s.mailboxes
                    .insert(Value::Address(actor.name.clone(), Box::new(key)), vec![]);
            }
        }
        // All typed addresses exist before any initializer runs. Initialization
        // cannot depend on declaration order or on an uninitialized actor state.
        for actor in self.actors.values() {
            if let Some(init) = &actor.initializer {
                let f = &self.functions[init];
                let keys: Vec<_> = s
                    .mailboxes
                    .keys()
                    .filter_map(|address| match address {
                        Value::Address(name, key) if name == &actor.name => {
                            Some(key.as_ref().clone())
                        }
                        _ => None,
                    })
                    .collect();
                for key in keys {
                    let mut env = Env::new();
                    if actor.key.is_some() {
                        env.insert(f.params[0].0.clone(), key.clone());
                    }
                    let state = self.eval_function_body(&f.body, &mut env, &s)?;
                    self.check_value(&state, f.span)?;
                    if actor.key.is_some() {
                        s.keyed_actors
                            .entry(actor.name.clone())
                            .or_default()
                            .insert(key, state);
                    } else {
                        s.actors.insert(actor.name.clone(), state);
                    }
                }
            }
        }
        for input in &self.check.inputs {
            let key = input
                .key
                .as_ref()
                .map(|k| self.eval(k, &Env::new(), &s))
                .transpose()?
                .unwrap_or(Value::Unit);
            let actor = self.handlers[&input.handler].actor.clone();
            if !s
                .mailboxes
                .contains_key(&Value::Address(actor, Box::new(key)))
            {
                return Err(Error::new(
                    input.span,
                    "LIMIT: input target outside finite identity domain",
                ));
            }
            let message = self.eval(&input.value, &Env::new(), &s)?;
            self.check_value(&message, input.span)?;
            s.input_submitted.push(false);
        }
        Ok(s)
    }

    pub(crate) fn message_successors(&self, s: &State) -> Result<Vec<Step>> {
        let mut steps = vec![Step {
            action: Action {
                id: "stutter".into(),
                description: "stutter".into(),
                span: Span::default(),
                fair: false,
            },
            state: s.clone(),
        }];
        let bound = self
            .check
            .mailbox_bound
            .expect("validated actors-v2 mailbox bound");
        for (i, input) in self.check.inputs.iter().enumerate() {
            if s.input_submitted[i] {
                continue;
            }
            let key = input
                .key
                .as_ref()
                .map(|k| self.eval(k, &Env::new(), s))
                .transpose()?
                .unwrap_or(Value::Unit);
            let address =
                Value::Address(self.handlers[&input.handler].actor.clone(), Box::new(key));
            let message = self.eval(&input.value, &Env::new(), s)?;
            let mut next = s.clone();
            let queue = next.mailboxes.get_mut(&address).ok_or_else(|| {
                Error::new(
                    input.span,
                    "LIMIT: input target outside finite identity domain",
                )
            })?;
            if queue.len() >= bound {
                return Err(Error::new(
                    input.span,
                    format!("LIMIT: mailbox capacity ({bound}) at {address}"),
                ));
            }
            queue.push(message.clone());
            next.input_submitted[i] = true;
            steps.push(Step {
                action: Action {
                    id: format!("submit:{i}"),
                    description: format!("submit {message} to {address} from input #{i}"),
                    span: input.span,
                    fair: false,
                },
                state: next,
            });
        }
        for (address, queue) in &s.mailboxes {
            let Some(message) = queue.first() else {
                continue;
            };
            let Value::Address(actor_name, key) = address else {
                return Err(Error::new(Span::default(), "internal: invalid mailbox key"));
            };
            let actor = &self.actors[actor_name];
            let h = &self.handlers[&format!("{actor_name}.handle_message")];
            let function = &self.functions[&h.function];
            let mut args = vec![];
            if actor.state.is_some() {
                args.push(if actor.key.is_some() {
                    s.keyed_actors[actor_name][key.as_ref()].clone()
                } else {
                    s.actors[actor_name].clone()
                });
            }
            args.push(message.clone());
            let mut env: Env = function
                .params
                .iter()
                .zip(args)
                .map(|((name, _), value)| (name.clone(), value))
                .collect();
            let mut outbox = vec![];
            let next_state = self.eval_message_body(&function.body, &mut env, s, &mut outbox)?;
            self.check_value(&next_state, function.span)?;
            let mut next = s.clone();
            next.mailboxes
                .get_mut(address)
                .expect("known mailbox")
                .remove(0);
            if actor.state.is_some() {
                if actor.key.is_some() {
                    next.keyed_actors
                        .get_mut(actor_name)
                        .expect("initialized actor")
                        .insert(*key.clone(), next_state.clone());
                } else {
                    next.actors.insert(actor_name.clone(), next_state.clone());
                }
            }
            let mut counts = BTreeMap::<Value, usize>::new();
            for (target, value) in outbox {
                self.check_value(&value, function.span)?;
                let queue = next.mailboxes.get_mut(&target).ok_or_else(|| {
                    Error::new(
                        function.span,
                        "LIMIT: send target outside finite identity domain",
                    )
                })?;
                if queue.len() >= bound {
                    return Err(Error::new(
                        function.span,
                        format!("LIMIT: mailbox capacity ({bound}) at {target}"),
                    ));
                }
                queue.push(value);
                *counts.entry(target).or_default() += 1;
            }
            steps.push(Step {
                action: Action {
                    id: format!("process:{}", serde_json::to_string(address).expect("serializable address")),
                    description: format!("process {message} at {address}; commit {next_state}; enqueue {} message(s)", counts.values().sum::<usize>()),
                    span: function.span,
                    fair: self.check.fair,
                },
                state: next,
            });
        }
        Ok(steps)
    }

    fn eval_message_body(
        &self,
        body: &[Stmt],
        env: &mut Env,
        s: &State,
        outbox: &mut Vec<(Value, Value)>,
    ) -> Result<Value> {
        let mut result = Value::Unit;
        for statement in body {
            result = match &statement.kind {
                StmtKind::Let(name, expr) => {
                    let value = self.eval_message_expr(expr, env, s, outbox)?;
                    env.insert(name.clone(), value);
                    Value::Unit
                }
                StmtKind::Expr(expr) => self.eval_message_expr(expr, env, s, outbox)?,
                StmtKind::Match(expr, arms) => {
                    let value = self.eval(expr, env, s)?;
                    let mut chosen = None;
                    for (pattern, branch) in arms {
                        let mut locals = env.clone();
                        if bind_pattern(pattern, &value, &mut locals) {
                            chosen =
                                Some(self.eval_message_body(branch, &mut locals, s, outbox)?);
                            break;
                        }
                    }
                    chosen.ok_or_else(|| {
                        Error::new(statement.span, "internal: non-exhaustive message match")
                    })?
                }
            };
        }
        Ok(result)
    }

    fn eval_message_expr(
        &self,
        expr: &Expr,
        env: &Env,
        s: &State,
        outbox: &mut Vec<(Value, Value)>,
    ) -> Result<Value> {
        if let ExprKind::Call(target, args) = &expr.kind {
            let path = target.path().unwrap_or_default();
            if path == "send" {
                let address = self.eval(&args[0], env, s)?;
                let message = self.eval(&args[1], env, s)?;
                self.check_value(&message, expr.span)?;
                if !s.mailboxes.contains_key(&address) {
                    return Err(Error::new(
                        expr.span,
                        "LIMIT: send target outside finite identity domain",
                    ));
                }
                outbox.push((address, message));
                return Ok(Value::Unit);
            }
            if let Some(f) = self.functions.get(&path)
                && self.effects[&path].suspends_or_writes()
            {
                let values = args
                    .iter()
                    .map(|e| self.eval(e, env, s))
                    .collect::<Result<Vec<_>>>()?;
                let mut locals: Env = f
                    .params
                    .iter()
                    .zip(values)
                    .map(|((name, _), value)| (name.clone(), value))
                    .collect();
                return self.eval_message_body(&f.body, &mut locals, s, outbox);
            }
        }
        self.eval(expr, env, s)
    }
}
