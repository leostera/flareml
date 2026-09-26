//! Finite, branch-local allocation. The registry is installed only at turn commit.
use crate::{
    choices::Turn,
    model::Program,
    semantics::{Env, State, Value},
    syntax::{Error, Result, Span},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    pub address: Value,
    pub span: Span,
    pub calls: Vec<Span>,
    pub initial: Value,
    pub arguments: Vec<Value>,
}

impl Turn<'_> {
    pub(crate) fn spawn(
        &mut self,
        p: &Program,
        name: &str,
        arguments: Vec<Value>,
        span: Span,
        state: &State,
    ) -> Result<Value> {
        self.poll()?;
        let actor = &p.actors[name];
        let prior = state.spawned.get(name).map_or(0, Vec::len);
        let reserved = self
            .spawns
            .iter()
            .filter(|s| matches!(&s.address, Value::Address(a, _) if a == name))
            .count();
        let index = prior + reserved;
        if index >= p.check.spawn_bounds[name] {
            return Err(Error::new(
                span,
                format!(
                    "LIMIT: spawn capacity ({} lifetime instances of {name})",
                    p.check.spawn_bounds[name]
                ),
            ));
        }
        if state.mailboxes.len() + self.spawns.len() >= 4096 {
            return Err(Error::new(
                span,
                "LIMIT: total actor address capacity (4096)",
            ));
        }
        for value in &arguments {
            p.check_value(value, span)?;
        }
        let initial = if let Some(init) = &actor.initializer {
            let f = &p.functions[init];
            let mut env: Env = f
                .params
                .iter()
                .zip(&arguments)
                .map(|((name, _), value)| (name.clone(), value.clone()))
                .collect();
            p.eval_body(&f.body, &mut env, state, None)?
        } else {
            Value::Unit
        };
        p.check_value(&initial, span)?;
        let address = Value::Address(name.into(), Box::new(Value::Identity(index)));
        self.spawns.push(Spawn {
            address: address.clone(),
            span,
            calls: self.calls.clone(),
            initial,
            arguments,
        });
        Ok(address)
    }
}

impl Program {
    pub(crate) fn instance_field(
        &self,
        actor: &str,
        index: usize,
        field: &str,
        state: &State,
        span: Span,
    ) -> Result<Value> {
        if self
            .check
            .spawn_bounds
            .get(actor)
            .is_none_or(|bound| index >= *bound)
        {
            return Err(Error::new(span, "internal: invalid instance observation"));
        }
        let value = state.spawned.get(actor).and_then(|slots| slots.get(index));
        match field {
            "created" => Ok(Value::Bool(value.is_some())),
            "reference" => Ok(value.map_or_else(Value::none, |_| {
                Value::some(Value::Address(
                    actor.into(),
                    Box::new(Value::Identity(index)),
                ))
            })),
            "state" if self.actors[actor].state.is_some() => {
                Ok(value.map_or_else(Value::none, |v| Value::some(v.clone())))
            }
            _ => Err(Error::new(
                span,
                "unknown or unavailable instance observation field",
            )),
        }
    }
}
