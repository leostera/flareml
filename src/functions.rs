//! Actor binding and conservative, transitive send/choice/inspection effects.
use crate::{
    model::{Effects, Program, Ty},
    syntax::*,
};
use std::collections::{BTreeMap, BTreeSet};

impl Program {
    pub(crate) fn is_global(&self, name: &str) -> bool {
        self.functions.contains_key(name)
            || self.actors.contains_key(name)
            || self.constructors.contains_key(name)
            || [
                "send", "choose", "inputs", "messages", "Some", "None", "Ok", "Err",
            ]
            .contains(&name)
    }
    pub(crate) fn bind_actors_and_functions(&mut self) -> Result<()> {
        let mut names: BTreeSet<String> = self.constructors.keys().cloned().collect();
        names.extend(self.model.types.iter().map(|t| t.name.clone()));
        names.extend(
            [
                "send", "choose", "inputs", "messages", "Some", "None", "Ok", "Err", "Actor",
                "Option", "Result",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        for f in &self.model.functions {
            if !names.insert(f.name.clone()) {
                return Err(Error::new(
                    f.span,
                    "duplicate or reserved function/global name",
                ));
            }
            self.functions.insert(f.name.clone(), f.clone());
        }
        for actor in &self.model.actors {
            if !names.insert(actor.name.clone()) {
                return Err(Error::new(
                    actor.span,
                    "duplicate or reserved actor/global name",
                ));
            }
            self.actors.insert(actor.name.clone(), actor.clone());
        }
        for c in self.constructors.values() {
            for ty in c.payload.iter().chain(c.fields.values()) {
                self.require_data(ty, Span::default())?;
            }
        }
        for f in self.functions.values() {
            self.require_data(&f.output, f.span)?;
            for (_, ty) in &f.params {
                self.require_data(ty, f.span)?;
            }
        }
        for actor in self.actors.values() {
            if let Some((name, ty)) = &actor.key {
                self.require_data(ty, actor.span)?;
                if !matches!(self.resolve(ty, actor.span)?, Ty::Named(_)) {
                    return Err(Error::new(
                        actor.span,
                        "actor identity requires a finite named data type",
                    ));
                }
                if self.is_global(name) {
                    return Err(Error::new(
                        actor.span,
                        "actor identity parameter shadows a global name",
                    ));
                }
            }
            let state = actor
                .state
                .as_ref()
                .map(|ty| self.resolve(ty, actor.span))
                .transpose()?;
            if let Some(ty) = &actor.state {
                self.require_data(ty, actor.span)?;
            }
            let f = &self.functions[&actor.handler];
            if self.resolve(&f.output, f.span)? != state.clone().unwrap_or(Ty::Named("unit".into()))
            {
                return Err(Error::new(
                    f.span,
                    "handle_message must return the actor's state type (or unit for a stateless actor)",
                ));
            }
            if f.params.len() != if state.is_some() { 2 } else { 1 } {
                return Err(Error::new(
                    f.span,
                    if state.is_some() {
                        "stateful handle_message takes (state: State, message: Message)"
                    } else {
                        "stateless handle_message takes exactly one message parameter"
                    },
                ));
            }
            if let Some(state) = &state
                && self.resolve(&f.params[0].1, f.span)? != *state
            {
                return Err(Error::new(
                    f.span,
                    "handler state parameter does not match the actor's state type",
                ));
            }
        }
        self.infer_effects()?;
        for actor in self.actors.values() {
            if let Some(init) = &actor.initializer {
                let f = &self.functions[init];
                let key_matches = match (&actor.key, f.params.as_slice()) {
                    (None, []) => true,
                    (Some((_, key)), [(_, param)]) => {
                        self.resolve(key, actor.span)? == self.resolve(param, f.span)?
                    }
                    _ => false,
                };
                if !key_matches
                    || self.effects[init].sends
                    || self.effects[init].chooses
                    || self.effects[init].inspects
                {
                    return Err(Error::new(
                        f.span,
                        "init must be pure and take only the actor identity (or no arguments for a singleton)",
                    ));
                }
            }
            if self.effects[&actor.handler].inspects {
                return Err(Error::new(
                    actor.span,
                    "specification inspector functions cannot be actor handlers",
                ));
            }
        }
        Ok(())
    }
    fn require_data(&self, ty: &Type, span: Span) -> Result<()> {
        self.require_data_inner(ty, span, &mut BTreeSet::new(), &mut BTreeSet::new(), 0)
    }
    fn require_data_inner(
        &self,
        ty: &Type,
        span: Span,
        seen: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<()> {
        if depth > 64 {
            return Err(Error::new(span, "data type nesting exceeds 64"));
        }
        self.resolve(ty, span)?;
        if ty.name == "Actor" {
            return Ok(());
        }
        for arg in &ty.args {
            self.require_data_inner(arg, span, seen, done, depth + 1)?;
        }
        let Ty::Named(name) = self.resolve(ty, span)? else {
            return Ok(());
        };
        if done.contains(&name) {
            return Ok(());
        }
        if !seen.insert(name.clone()) {
            return Err(Error::new(
                span,
                "recursive data is unsupported; use a finite non-recursive representation",
            ));
        }
        for c in self.constructors.values().filter(|c| c.ty == name) {
            for field in c.payload.iter().chain(c.fields.values()) {
                self.require_data_inner(field, span, seen, done, depth + 1)?;
            }
        }
        seen.remove(&name);
        done.insert(name);
        Ok(())
    }
    fn infer_effects(&mut self) -> Result<()> {
        let mut graph = petgraph::graph::DiGraph::<String, ()>::new();
        let nodes: BTreeMap<_, _> = self
            .functions
            .keys()
            .map(|name| (name.clone(), graph.add_node(name.clone())))
            .collect();
        let mut direct = BTreeMap::new();
        for (name, f) in &self.functions {
            let mut fx = Effects::default();
            let mut dependencies = vec![];
            let mut cost = 0usize;
            visit_body(&f.body, &mut |e| {
                cost += 1;
                if let ExprKind::Field(_, field) = &e.kind
                    && field == "state"
                    && actor_target(e).is_some_and(|(path, _)| {
                        path.strip_suffix(".state")
                            .is_some_and(|n| self.actors.contains_key(n))
                    })
                {
                    fx.inspects = true;
                }
                if matches!(e.kind, ExprKind::Quant { .. }) {
                    fx.inspects = true;
                }
                if let ExprKind::Call(target, _) = &e.kind {
                    let path = target.path().unwrap_or_default();
                    if self.functions.contains_key(&path) {
                        dependencies.push(path.clone());
                    }
                    if path == "inputs" || path == "messages" {
                        fx.inspects = true;
                    }
                    if path == "send" {
                        fx.sends = true;
                    }
                    if path == "choose" {
                        fx.chooses = true;
                    }
                }
            });
            for dep in &dependencies {
                graph.add_edge(nodes[name], nodes[dep], ());
            }
            direct.insert(name.clone(), (fx, dependencies, cost));
        }
        let order = petgraph::algo::toposort(&graph, None).map_err(|cycle| {
            Error::new(
                self.functions[&graph[cycle.node_id()]].span,
                "recursive local function call cycle is unsupported",
            )
        })?;
        let mut costs = BTreeMap::<String, usize>::new();
        let mut depths = BTreeMap::<String, usize>::new();
        for index in order.into_iter().rev() {
            let name = &graph[index];
            let (mut fx, dependencies, mut cost) = direct[name].clone();
            let mut depth = 1;
            for dep in dependencies {
                fx.include(self.effects[&dep]);
                cost = cost.saturating_add(costs[&dep]);
                depth = depth.max(1 + depths[&dep]);
            }
            if depth > 64 {
                return Err(Error::new(
                    self.functions[name].span,
                    "local function call depth exceeds 64",
                ));
            }
            if cost > 10_000 {
                return Err(Error::new(
                    self.functions[name].span,
                    "expanded function cost exceeds 10,000-node elaboration limit",
                ));
            }
            if fx.inspects && (fx.sends || fx.chooses) {
                return Err(Error::new(
                    self.functions[name].span,
                    "a function cannot mix specification inspection with send/choice effects",
                ));
            }
            depths.insert(name.clone(), depth);
            costs.insert(name.clone(), cost);
            self.effects.insert(name.clone(), fx);
        }
        Ok(())
    }
}
fn visit_body(body: &[Stmt], visit: &mut impl FnMut(&Expr)) {
    for s in body {
        match &s.kind {
            StmtKind::Let(_, e) | StmtKind::Expr(e) => visit_expr(e, visit),
            StmtKind::Match(e, arms) => {
                visit_expr(e, visit);
                for (_, body) in arms {
                    visit_body(body, visit);
                }
            }
        }
    }
}
fn visit_expr(e: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(e);
    match &e.kind {
        ExprKind::Field(e, _) | ExprKind::Unary(_, e) => visit_expr(e, visit),
        ExprKind::Call(e, args) => {
            visit_expr(e, visit);
            for e in args {
                visit_expr(e, visit);
            }
        }
        ExprKind::Binary(_, a, b) => {
            visit_expr(a, visit);
            visit_expr(b, visit);
        }
        ExprKind::Quant { domain, body, .. } => {
            visit_expr(domain, visit);
            visit_expr(body, visit);
        }
        ExprKind::Record(_, fields) => {
            for e in fields.values() {
                visit_expr(e, visit);
            }
        }
        ExprKind::List(xs) => {
            for e in xs {
                visit_expr(e, visit);
            }
        }
        _ => {}
    }
}
