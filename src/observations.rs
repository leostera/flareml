//! Finite, stable observation handles. Message slots are never recycled: a
//! temporal binding cannot accidentally refer to a later send with the same data.
use crate::{
    model::Program,
    semantics::{Envelope, MessageObservation, State, Value},
    syntax::{Error, Result, Span},
};

impl Program {
    pub(crate) fn enqueue_message(
        &self,
        s: &mut State,
        address: Value,
        payload: Value,
        input: Option<usize>,
        source: Span,
    ) -> Result<()> {
        let queue = s.mailboxes.get(&address).ok_or_else(|| {
            Error::new(source, "LIMIT: send target outside finite identity domain")
        })?;
        let capacity = self.check.mailbox_bound.expect("typed mailbox bound");
        if queue.len() >= capacity {
            return Err(Error::new(
                source,
                format!("LIMIT: mailbox capacity ({capacity}) at {address}"),
            ));
        }
        let Value::Address(actor, _) = &address else {
            return Err(Error::new(source, "internal: invalid send address"));
        };
        let observation = if let Some(bound) = self.check.message_bound {
            let records = s.messages.entry(actor.clone()).or_default();
            if records.len() >= bound {
                return Err(Error::new(
                    source,
                    format!(
                        "LIMIT: message observation capacity ({bound} lifetime sends to actor {actor})"
                    ),
                ));
            }
            let index = records.len();
            records.push(MessageObservation {
                payload: payload.clone(),
                target: address.clone(),
                external: input.is_some(),
                processed: false,
            });
            Some(index)
        } else {
            None
        };
        s.mailboxes
            .get_mut(&address)
            .expect("validated mailbox")
            .push(Envelope {
                payload,
                input,
                observation,
                source,
            });
        Ok(())
    }

    pub(crate) fn input_field(
        &self,
        i: usize,
        field: &str,
        s: &State,
        span: Span,
    ) -> Result<Value> {
        let input = s
            .inputs
            .get(i)
            .ok_or_else(|| Error::new(span, "internal: invalid input observation"))?;
        match field {
            "submitted" => Ok(Value::Bool(s.input_submitted[i])),
            "processed" => Ok(Value::Bool(s.input_processed[i])),
            "payload" => Ok(input.payload.clone()),
            "target" => Ok(input.target.clone()),
            _ => Err(Error::new(span, "unknown input observation field")),
        }
    }

    pub(crate) fn message_field(
        &self,
        actor: &str,
        index: usize,
        field: &str,
        s: &State,
        span: Span,
    ) -> Result<Value> {
        if self.check.message_bound.is_none_or(|n| index >= n) || !self.actors.contains_key(actor) {
            return Err(Error::new(span, "internal: invalid message observation"));
        }
        let record = s.messages.get(actor).and_then(|records| records.get(index));
        match field {
            "sent" => Ok(Value::Bool(record.is_some())),
            "processed" => Ok(Value::Bool(record.is_some_and(|r| r.processed))),
            "external" => Ok(Value::Bool(record.is_some_and(|r| r.external))),
            "payload" => Ok(record.map_or_else(Value::none, |r| Value::some(r.payload.clone()))),
            "target" => Ok(record.map_or_else(Value::none, |r| Value::some(r.target.clone()))),
            _ => Err(Error::new(span, "unknown message observation field")),
        }
    }
}
