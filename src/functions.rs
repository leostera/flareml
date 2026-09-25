//! Actor binding and conservative, transitive function effect inference.
//! Local function dependencies must be acyclic; actor calls cross scheduler boundaries.
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
            || self
                .tables
                .keys()
                .any(|p| p.split('.').next() == Some(name))
            || ["call", "requests", "respond", "Some", "None", "Ok", "Err"].contains(&name)
    }

    pub(crate) fn bind_actors_and_functions(&mut self) -> Result<()> {
        let mut names: BTreeSet<String> = self.constructors.keys().cloned().collect();
        names.extend(self.model.types.iter().map(|t| t.name.clone()));
        names.extend(
            self.tables
                .keys()
                .filter_map(|p| p.split('.').next().map(str::to_owned)),
        );
        names.extend(
            [
                "call", "requests", "respond", "Some", "None", "Ok", "Err", "Actor", "Address",
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
                self.require_data(ty, Span::default(), &mut BTreeSet::new())?;
            }
        }
        for f in self.functions.values() {
            self.require_data(&f.output, f.span, &mut BTreeSet::new())?;
            for (_, ty) in &f.params {
                if ty.name == "Actor" && ty.args.len() == 1 {
                    self.require_data(&ty.args[0], f.span, &mut BTreeSet::new())?;
                } else {
                    self.require_data(ty, f.span, &mut BTreeSet::new())?;
                }
            }
        }
        for actor in self.actors.values() {
            if let Some((name, ty)) = &actor.key {
                self.require_data(ty, actor.span, &mut BTreeSet::new())?;
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
                .map(|(ty, _)| self.resolve(ty, actor.span))
                .transpose()?;
            if let Some((ty, _)) = &actor.state {
                self.require_data(ty, actor.span, &mut BTreeSet::new())?;
            }
            for (method, function) in &actor.handlers {
                let f = self.functions.get(function).ok_or_else(|| {
                    Error::new(actor.span, format!("unknown handler function `{function}`"))
                })?;
                let count = if state.is_some() { 2 } else { 1 };
                if f.params.len() != count {
                    return Err(Error::new(
                        actor.span,
                        if state.is_some() {
                            "stateful handler functions take (owner: Actor<State>, message: Input)"
                        } else {
                            "stateless handler functions take exactly one message parameter"
                        },
                    ));
                }
                if let Some(state) = &state {
                    let expected = Ty::Actor(Box::new(state.clone()));
                    if self.resolve(&f.params[0].1, f.span)? != expected {
                        return Err(Error::new(
                            f.span,
                            "handler Actor<State> capability does not match the actor's state type",
                        ));
                    }
                }
                let input = f.params.last().expect("validated arity").1.clone();
                self.require_data(&input, f.span, &mut BTreeSet::new())?;
                let path = format!("{}.{}", actor.name, method);
                self.handlers.insert(
                    path.clone(),
                    Handler {
                        path,
                        actor: actor.name.clone(),
                        function: function.clone(),
                        input,
                        output: f.output.clone(),
                        span: actor.span,
                    },
                );
            }
        }
        self.infer_effects()?;
        for h in self.handlers.values() {
            if self.effects[&h.function].inspects {
                return Err(Error::new(
                    h.span,
                    "specification inspector functions cannot be actor handlers",
                ));
            }
        }
        Ok(())
    }

    fn require_data(&self, ty: &Type, span: Span, seen: &mut BTreeSet<String>) -> Result<()> {
        if ty.name == "Actor" {
            return Err(Error::new(
                span,
                "Actor<State> is an owned capability, not storable/returnable/message data",
            ));
        }
        if ty.name == "Address" {
            self.resolve(ty, span)?;
            return Ok(());
        }
        self.resolve(ty, span)?;
        for arg in &ty.args {
            self.require_data(arg, span, seen)?;
        }
        let Ty::Named(name) = self.resolve(ty, span)? else {
            return Ok(());
        };
        if !seen.insert(name.clone()) {
            return Ok(());
        }
        for c in self.constructors.values().filter(|c| c.ty == name) {
            for field in c.payload.iter().chain(c.fields.values()) {
                self.require_data(field, span, seen)?;
            }
        }
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
                    && actor_target(e).is_some_and(|(_, key)| key.is_some())
                {
                    fx.inspects = true;
                }
                if let Some(path) = e.path()
                    && (path
                        .strip_suffix(".rows")
                        .is_some_and(|p| self.tables.contains_key(p))
                        || path
                            .strip_suffix(".state")
                            .is_some_and(|p| self.actors.contains_key(p)))
                {
                    fx.inspects = true;
                }
                if matches!(e.kind, ExprKind::Quant { .. }) {
                    fx.inspects = true;
                }
                if let ExprKind::Call(target, _args) = &e.kind {
                    let path = target.path().unwrap_or_default();
                    if self.functions.contains_key(&path) {
                        dependencies.push(path.clone());
                    }
                    if path == "requests" {
                        fx.inspects = true;
                    }
                    if path == "call" {
                        fx.io = true;
                    }
                    if let ExprKind::Field(receiver, method) = &target.kind {
                        if method == "set" {
                            fx.writes_state = true;
                        }
                        if receiver
                            .path()
                            .is_some_and(|p| self.tables.contains_key(&p))
                        {
                            fx.io = true;
                        }
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
        for index in order.into_iter().rev() {
            let name = &graph[index];
            let (mut fx, dependencies, mut cost) = direct[name].clone();
            for dep in dependencies {
                fx.include(self.effects[&dep]);
                cost = cost.saturating_add(costs[&dep]);
            }
            if cost > 10_000 {
                return Err(Error::new(
                    self.functions[name].span,
                    "expanded function cost exceeds the spike's 10,000-node elaboration limit",
                ));
            }
            if fx.inspects && fx.suspends_or_writes() {
                return Err(Error::new(
                    self.functions[name].span,
                    "a function cannot mix specification inspection with state or I/O effects",
                ));
            }
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
