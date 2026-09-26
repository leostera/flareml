//! One execution contract: explicit deterministic setup, FIFO mailboxes, atomic turns.
use crate::{
    choices::Turn,
    model::Program,
    semantics::{Action, Env, State, Step, Value},
    syntax::{Error, Result, Span},
};
impl Program {
    pub(crate) fn initial_messages(&self, budget: Option<&crate::graph::Budget>) -> Result<State> {
        self.initial_recorded(budget).map(|(state, _, _)| state)
    }
    pub(crate) fn initial_recorded(
        &self,
        budget: Option<&crate::graph::Budget>,
    ) -> Result<(State, crate::semantics::Outbox, Vec<crate::spawning::Spawn>)> {
        let empty = State::default();
        let main = &self.functions[&self.check.main];
        let mut outcomes =
            self.turn_outcomes(&main.body, &Env::new(), &empty, budget, Some(&[]))?;
        let (_, turn) = outcomes.pop().expect("deterministic setup");
        let mut state = empty;
        self.publish(&mut state, &turn)?;
        for input in &turn.inputs {
            turn.poll()?;
            if !state.mailboxes.contains_key(&input.target) {
                return Err(Error::new(
                    input.source,
                    "input target must be created by main",
                ));
            }
        }
        state.input_submitted = vec![false; turn.inputs.len()];
        state.input_processed = vec![false; turn.inputs.len()];
        state.inputs = turn.inputs;
        Ok((state, turn.outbox, turn.spawns))
    }
    /// Called only on a private state clone; failure never publishes a partial turn.
    fn publish(&self, next: &mut State, turn: &Turn<'_>) -> Result<()> {
        for allocation in &turn.spawns {
            turn.poll()?;
            let Value::Address(name, key) = &allocation.address else {
                return Err(Error::new(
                    allocation.span,
                    "internal: invalid allocation address",
                ));
            };
            let slots = next.spawned.entry(name.clone()).or_default();
            if key.as_ref() != &Value::Identity(slots.len())
                || next.mailboxes.contains_key(&allocation.address)
            {
                return Err(Error::new(
                    allocation.span,
                    "internal: allocation is not fresh",
                ));
            }
            slots.push(allocation.initial.clone());
            next.mailboxes
                .insert(allocation.address.clone(), Vec::new());
        }
        for (target, payload, source) in &turn.outbox {
            turn.poll()?;
            self.enqueue_message(next, target.clone(), payload.clone(), None, *source)?;
        }
        Ok(())
    }
    pub(crate) fn fair_enabled(&self, s: &State) -> std::collections::BTreeSet<String> {
        s.mailboxes
            .iter()
            .filter(|(_, queue)| self.check.fair && !queue.is_empty())
            .map(|(address, _)| processing_id(address))
            .collect()
    }
    pub(crate) fn message_successors(
        &self,
        s: &State,
        budget: Option<&crate::graph::Budget>,
        replay: Option<&Action>,
    ) -> Result<Vec<Step>> {
        self.message_successors_recorded(s, budget, replay, false)
    }
    pub(crate) fn message_successors_recorded(
        &self,
        s: &State,
        budget: Option<&crate::graph::Budget>,
        replay: Option<&Action>,
        record: bool,
    ) -> Result<Vec<Step>> {
        let mut steps = vec![Step {
            action: Action {
                id: "stutter".into(),
                description: "stutter".into(),
                span: Span::default(),
                fair: false,
                choices: Vec::new(),
                spawns: Vec::new(),
            },
            state: s.clone(),
            outbox: record.then(Vec::new),
        }];
        for (i, input) in s.inputs.iter().enumerate() {
            if let Some(budget) = budget {
                budget.poll()?;
            }
            if s.input_submitted[i] || replay.is_some_and(|a| a.id != format!("submit:{i}")) {
                continue;
            }
            let mut next = s.clone();
            self.enqueue_message(
                &mut next,
                input.target.clone(),
                input.payload.clone(),
                Some(i),
                input.source,
            )?;
            next.input_submitted[i] = true;
            steps.push(Step {
                action: Action {
                    id: format!("submit:{i}"),
                    description: format!(
                        "submit {} to {} from input #{i}",
                        input.payload, input.target
                    ),
                    span: input.source,
                    fair: false,
                    choices: Vec::new(),
                    spawns: Vec::new(),
                },
                state: next,
                outbox: record
                    .then(|| vec![(input.target.clone(), input.payload.clone(), input.source)]),
            });
        }
        for (address, queue) in &s.mailboxes {
            if let Some(budget) = budget {
                budget.poll()?;
            }
            if replay.is_some_and(|a| a.id != processing_id(address)) {
                continue;
            }
            let Some(envelope) = queue.first() else {
                continue;
            };
            let Value::Address(actor_name, key) = address else {
                return Err(Error::new(
                    Span::default(),
                    "internal: invalid mailbox address",
                ));
            };
            let Value::Identity(index) = key.as_ref() else {
                return Err(Error::new(Span::default(), "internal: invalid identity"));
            };
            let actor = &self.actors[actor_name];
            let function = &self.functions[&actor.handler];
            let mut args = Vec::new();
            if actor.state.is_some() {
                args.push(s.spawned[actor_name][*index].clone());
            }
            args.push(envelope.payload.clone());
            let env = function
                .params
                .iter()
                .zip(args)
                .map(|((name, _), value)| (name.clone(), value))
                .collect();
            for (next_state, turn) in self.turn_outcomes(
                &function.body,
                &env,
                s,
                budget,
                replay.map(|a| a.choices.as_slice()),
            )? {
                turn.poll()?;
                if !turn.inputs.is_empty() {
                    return Err(Error::new(
                        function.span,
                        "internal: input registration outside main",
                    ));
                }
                self.check_value(&next_state, function.span)?;
                let mut next = s.clone();
                next.mailboxes
                    .get_mut(address)
                    .expect("known mailbox")
                    .remove(0);
                if actor.state.is_some() {
                    next.spawned.get_mut(actor_name).expect("allocated actor")[*index] =
                        next_state.clone();
                }
                if let Some(i) = envelope.input {
                    next.input_processed[i] = true;
                }
                if let Some(i) = envelope.observation {
                    next.messages.get_mut(actor_name).expect("observed actor")[i].processed = true;
                }
                self.publish(&mut next, &turn)?;
                let sends: Vec<_> = turn
                    .outbox
                    .iter()
                    .map(|(target, value, span)| {
                        format!("{value} -> {target} at byte {}", span.start)
                    })
                    .collect();
                steps.push(Step { action: Action {
                    id: processing_id(address),
                    description: format!("process {} at {address}; commit {next_state}; enqueue {} message(s) [{}]", envelope.payload, sends.len(), sends.join(", ")),
                    span: function.span, fair: self.check.fair, choices: turn.choices, spawns: turn.spawns,
                }, state: next, outbox: record.then_some(turn.outbox) });
            }
        }
        Ok(steps)
    }
}
fn processing_id(address: &Value) -> String {
    format!(
        "process:{}",
        serde_json::to_string(address).expect("serializable address")
    )
}
