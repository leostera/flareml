//! One execution contract: finite FIFO mailboxes and atomic state/outbox turns.
use crate::{
    model::Program,
    semantics::{Action, Env, State, Step, Value},
    syntax::{Error, Result, Span},
};

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
                if s.mailboxes.len() > 4096 {
                    return Err(Error::new(
                        actor.span,
                        "LIMIT: total actor address capacity (4096)",
                    ));
                }
            }
        }
        // All addresses exist before any pure initializer, independent of source order.
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
                    let state = self.eval_body(&f.body, &mut env, &s, None)?;
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
            let actor = input.actor.clone();
            if !s
                .mailboxes
                .contains_key(&Value::Address(actor, Box::new(key)))
            {
                return Err(Error::new(
                    input.span,
                    "LIMIT: input target outside finite identity domain",
                ));
            }
            self.check_value(&self.eval(&input.value, &Env::new(), &s)?, input.span)?;
            s.input_submitted.push(false);
            s.input_processed.push(false);
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
            let address = Value::Address(input.actor.clone(), Box::new(key));
            let message = self.eval(&input.value, &Env::new(), s)?;
            let mut next = s.clone();
            self.enqueue_message(
                &mut next,
                address.clone(),
                message.clone(),
                Some(i),
                input.span,
            )?;
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
            let Some(envelope) = queue.first() else {
                continue;
            };
            let message = &envelope.payload;
            let Value::Address(actor_name, key) = address else {
                return Err(Error::new(Span::default(), "internal: invalid mailbox key"));
            };
            let actor = &self.actors[actor_name];
            let function = &self.functions[&actor.handler];
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
            let next_state = self.eval_body(&function.body, &mut env, s, Some(&mut outbox))?;
            self.check_value(&next_state, function.span)?;
            // Work on a clone: a cutoff never partly mutates the source state.
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
            if let Some(i) = envelope.input {
                next.input_processed[i] = true;
            }
            if let Some(i) = envelope.observation {
                next.messages.get_mut(actor_name).expect("observed actor")[i].processed = true;
            }
            let count = outbox.len();
            let mut sends = vec![];
            for (target, value, source) in outbox {
                sends.push(format!("{value} -> {target} at byte {}", source.start));
                self.enqueue_message(&mut next, target, value, None, source)?;
            }
            steps.push(Step {
                action: Action {
                    id: format!("process:{}", serde_json::to_string(address).expect("serializable address")),
                    description: format!("process {message} at {address}; commit {next_state}; enqueue {count} message(s) [{}]", sends.join(", ")),
                    span: function.span, fair: self.check.fair,
                },
                state: next,
            });
        }
        Ok(steps)
    }
}
