//! Disposable, lossless presentation of authoritative replay. No viewer semantics.
use crate::{
    model::Program,
    semantics::{Outbox, State, Value},
    syntax::{Error, Result, Span},
    trace::Trace,
};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, VecDeque};

type Queues = BTreeMap<Value, VecDeque<String>>;
const MAX_QUEUE_REFERENCES: usize = 1_000_000;

pub struct Session {
    source: String,
    trace: Trace,
    siblings: Vec<Session>,
    property: Json,
    stateful: BTreeMap<String, bool>,
    queues: Vec<Queues>,
    events: Vec<Json>,
}
fn error(message: &str) -> Error {
    Error::new(Span::default(), message)
}
pub fn actor_id(address: &Value) -> String {
    match address {
        Value::Address(name, key) => match key.as_ref() {
            Value::Identity(index) => format!("actor:{name}:{index}"),
            _ => unreachable!("validated identity"),
        },
        _ => unreachable!("validated address"),
    }
}
/// Preserve all i64 values across JSON/JavaScript, including inside choices/records.
pub fn lossless(mut value: Json) -> Json {
    match &mut value {
        Json::Object(fields) => {
            if fields.len() == 1
                && let Some(Json::Number(n)) = fields.get("Int")
            {
                return json!({"Int": n.to_string()});
            }
            for field in fields.values_mut() {
                *field = lossless(field.take());
            }
        }
        Json::Array(values) => {
            for item in values {
                *item = lossless(item.take());
            }
        }
        _ => {}
    }
    value
}
fn location(source: &str, span: Span) -> Json {
    let prefix = source.get(..span.start).unwrap_or_default();
    let text = source.get(span.start..span.end).unwrap_or_default();
    json!({"start":span.start,"end":span.end,"line":prefix.bytes().filter(|b| *b == b'\n').count()+1,
        "column":prefix.rsplit('\n').next().unwrap_or_default().chars().count()+1,"text":text})
}
fn enqueues(source: &str, snapshot: usize, outbox: Outbox, queues: &mut Queues) -> Vec<Json> {
    outbox.into_iter().enumerate().map(|(i,(target,payload,span))| {
        let id = format!("envelope:{snapshot}:{i}");
        queues.entry(target.clone()).or_default().push_back(id.clone());
        json!({"id":id,"target":actor_id(&target),"payload":payload,"source":location(source,span)})
    }).collect()
}
fn verify_queues(state: &State, queues: &Queues) -> Result<()> {
    if state
        .mailboxes
        .iter()
        .any(|(address, queue)| queues.get(address).map_or(0, VecDeque::len) != queue.len())
        || queues
            .keys()
            .any(|address| !state.mailboxes.contains_key(address))
    {
        return Err(error("internal: explorer queue projection mismatch"));
    }
    Ok(())
}
impl Session {
    /// Nothing from this session is exposed until the entire evidence is validated.
    pub fn validated(source: String, trace: Trace, program: &Program) -> Result<Self> {
        trace.validate(&source, program)?;
        // A constrained second replay captures presentation-only outboxes. Both
        // executions use the same interpreter and compare exact committed states.
        let (initial, outbox, allocations) = program.initial_recorded(None)?;
        if initial != trace.states[0] {
            return Err(error("internal: explorer setup mismatch"));
        }
        let mut current = Queues::new();
        let sends = enqueues(&source, 0, outbox, &mut current);
        verify_queues(&initial, &current)?;
        let spawns: Vec<_> = allocations.iter().map(|s| json!({"actor":actor_id(&s.address),"initial":s.initial,"arguments":s.arguments,"source":location(&source,s.span),"calls":s.calls.iter().map(|s|location(&source,*s)).collect::<Vec<_>>()})).collect();
        let mut events = vec![
            json!({"kind":"setup","actor":null,"consumed":null,"sends":sends,"choices":[],"spawns":spawns,"source":null}),
        ];
        let mut references = current.values().map(VecDeque::len).sum::<usize>();
        let mut queues = vec![current.clone()];
        for (i, action) in trace.actions.iter().enumerate() {
            let step = program
                .message_successors_recorded(&trace.states[i], None, Some(action), true)?
                .into_iter()
                .find(|s| s.action == *action)
                .ok_or_else(|| error("internal: explorer action mismatch"))?;
            if step.state != trace.states[i + 1] {
                return Err(error("internal: explorer successor mismatch"));
            }
            let mut actor = None;
            let mut consumed = None;
            let kind = if let Some(encoded) = action.id.strip_prefix("process:") {
                let address: Value = serde_json::from_str(encoded)
                    .map_err(|_| error("internal: explorer identity"))?;
                actor = Some(actor_id(&address));
                let id = current
                    .get_mut(&address)
                    .and_then(VecDeque::pop_front)
                    .ok_or_else(|| error("internal: explorer dequeue"))?;
                let envelope = &trace.states[i].mailboxes[&address][0];
                consumed = Some(
                    json!({"id":id,"payload":envelope.payload,"input":envelope.input,"observation":envelope.observation,"source":location(&source,envelope.source)}),
                );
                "process"
            } else if action.id.starts_with("submit:") {
                "submit"
            } else {
                "stutter"
            };
            let sends = enqueues(
                &source,
                i + 1,
                step.outbox.unwrap_or_default(),
                &mut current,
            );
            verify_queues(&step.state, &current)?;
            references += current.values().map(VecDeque::len).sum::<usize>();
            if references > MAX_QUEUE_REFERENCES {
                return Err(error(
                    "explorer limit: more than 1000000 queued envelope references; use text replay",
                ));
            }
            let choices: Vec<_> = action.choices.iter().map(|c| json!({"encounter":c.encounter,"candidate":c.candidate,"value":c.value,"source":location(&source,c.span),"calls":c.calls.iter().map(|s|location(&source,*s)).collect::<Vec<_>>()})).collect();
            let spawns: Vec<_> = action.spawns.iter().map(|s| json!({"actor":actor_id(&s.address),"initial":s.initial,"arguments":s.arguments,"source":location(&source,s.span),"calls":s.calls.iter().map(|s|location(&source,*s)).collect::<Vec<_>>()})).collect();
            events.push(lossless(json!({"kind":kind,"actor":actor,"consumed":consumed,"sends":sends,"choices":choices,"spawns":spawns,"source":if kind == "stutter" {Json::Null} else {location(&source,action.span)}})));
            queues.push(current.clone());
        }
        events[0] = lossless(events[0].take());
        let claim = program
            .model
            .claims
            .iter()
            .find(|c| c.name == trace.claim)
            .ok_or_else(|| error("internal: missing validated claim"))?;
        let property = location(&source, claim.body.span);
        Ok(Self {
            source,
            trace,
            siblings: Vec::new(),
            property,
            queues,
            events,
            stateful: program
                .actors
                .iter()
                .map(|(name, a)| (name.clone(), a.state.is_some()))
                .collect(),
        })
    }
    pub fn add_witness(&mut self, session: Session) {
        self.siblings.push(session);
    }
    pub fn execution(&self, index: usize) -> Option<&Session> {
        if index == 0 {
            Some(self)
        } else {
            self.siblings.get(index - 1)
        }
    }
    /// Merge exact committed prefixes, not states reached by different histories.
    pub fn tree(&self) -> Json {
        use sha2::{Digest, Sha256};
        let mut keys = BTreeMap::new();
        let mut nodes: Vec<Json> = Vec::new();
        let mut traces = Vec::new();
        for (id, session) in std::iter::once(self)
            .chain(self.siblings.iter())
            .enumerate()
        {
            let mut parent: Option<usize> = None;
            let mut path = Vec::new();
            for (step, state) in session.trace.states.iter().enumerate() {
                let evidence = if step == 0 {
                    json!([
                        session.trace.source_hash,
                        session.trace.check,
                        session.trace.spawn_bounds,
                        session.trace.mailbox_bound,
                        session.trace.message_bound,
                        state
                    ])
                } else {
                    json!([session.trace.actions[step - 1], state])
                };
                let hash = Sha256::digest(serde_json::to_vec(&evidence).unwrap()).to_vec();
                let key = (parent, hash);
                let node = *keys.entry(key).or_insert_with(|| {
                    let index = nodes.len();
                    let event = &session.events[step];
                    nodes.push(json!({"id":index,"parent":parent,"step":step,"trace":id,"kind":event["kind"],"actor":event["actor"],"choices":event["choices"],"ends":[]}));
                    index
                });
                path.push(node);
                parent = Some(node);
            }
            let end = *path.last().unwrap();
            nodes[end]["ends"].as_array_mut().unwrap().push(json!(id));
            traces.push(json!({"id":id,"claim":session.trace.claim,"violation":session.metadata()["kind"] != "Cover","loop_start":session.trace.loop_start,"path":path}));
        }
        lossless(json!({"nodes":nodes,"traces":traces}))
    }
    pub fn metadata(&self) -> Json {
        json!({"schema_version":1,"check":self.trace.check,"claim":self.trace.claim,"kind":self.trace.kind,
            "source_hash":self.trace.source_hash,"source":self.source,"tool_version":self.trace.tool_version,
            "property":self.property,"clause":self.trace.clause,"trace_version":self.trace.format_version,"snapshots":self.trace.states.len(),"loop_start":self.trace.loop_start,
            "fair":self.trace.weak_progress,"spawn_bounds":self.trace.spawn_bounds,"mailbox_bound":self.trace.mailbox_bound,"message_bound":self.trace.message_bound})
    }
    pub fn summaries(&self, offset: usize, filter: &str) -> Json {
        let mut total = 0;
        let mut items = Vec::new();
        let filter = filter.to_lowercase();
        for (i, e) in self.events.iter().enumerate() {
            let text = format!(
                "{i} {} {}",
                e["kind"].as_str().unwrap_or_default(),
                e["actor"].as_str().unwrap_or_default()
            )
            .to_lowercase();
            if !text.contains(&filter) {
                continue;
            }
            if total >= offset && items.len() < 100 {
                items.push(json!({"index":i,"kind":e["kind"],"actor":e["actor"],"choices":e["choices"].as_array().unwrap().len(),"spawns":e["spawns"].as_array().unwrap().len()}));
            }
            total += 1;
        }
        json!({"total":total,"items":items})
    }
    pub fn snapshot(&self, index: usize) -> Option<Json> {
        let state = self.trace.states.get(index)?;
        let actors: Vec<_> = state.spawned.iter().flat_map(|(name,values)| values.iter().enumerate().map(move |(slot,value)| {
            let address = Value::Address(name.clone(),Box::new(Value::Identity(slot)));
            let mailbox: Vec<_> = state.mailboxes[&address].iter().enumerate().map(|(position,e)| json!({
                "id":self.queues[index][&address][position],"payload":e.payload,"input":e.input,"observation":e.observation,"source":location(&self.source,e.source)
            })).collect();
            json!({"id":actor_id(&address),"name":name,"slot":slot,"label":address.to_string(),"stateful":self.stateful[name],"state":if self.stateful[name] {serde_json::to_value(value).unwrap()} else {Json::Null},"mailbox":mailbox})
        })).collect();
        Some(lossless(
            json!({"index":index,"actors":actors,"inputs":state.inputs,"submitted":state.input_submitted,"processed":state.input_processed,"messages":state.messages,"event":self.events[index]}),
        ))
    }
}
