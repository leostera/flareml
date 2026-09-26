//! Name resolution, a deliberately closed type/effect system, and handler lowering.
use crate::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Named(String),
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    List(Box<Ty>),
    Rows(String),
    Request(String),
    Input(String),
    Message(String),
    Actor(Box<Ty>),
    Address(String),
    Temporal,
    Never,
}
impl Ty {
    fn named(n: &str) -> Self {
        Self::Named(n.into())
    }
    fn merge(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Never, t) | (t, Self::Never) => Some(t.clone()),
            (Self::Option(a), Self::Option(b)) => Some(Self::Option(Box::new(a.merge(b)?))),
            (Self::List(a), Self::List(b)) => Some(Self::List(Box::new(a.merge(b)?))),
            (Self::Result(a, b), Self::Result(c, d)) => {
                Some(Self::Result(Box::new(a.merge(c)?), Box::new(b.merge(d)?)))
            }
            _ if self == other => Some(self.clone()),
            _ => None,
        }
    }
    fn compatible(&self, other: &Self) -> bool {
        self.merge(other).is_some()
    }
}
#[derive(Clone, Debug)]
pub struct Constructor {
    pub ty: String,
    pub payload: Vec<Type>,
    pub fields: BTreeMap<String, Type>,
}
#[derive(Clone, Debug)]
pub enum Instruction {
    End,
    Return(Expr),
    Let {
        name: String,
        value: Expr,
        next: usize,
    },
    Eval {
        value: Expr,
        next: usize,
    },
    Match {
        value: Expr,
        arms: Vec<(Pattern, usize)>,
    },
}
#[derive(Clone, Debug)]
pub struct Code {
    pub instruction: Instruction,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Effects {
    pub io: bool,
    pub writes_state: bool,
    pub inspects: bool,
}
impl Effects {
    pub fn suspends_or_writes(self) -> bool {
        self.io || self.writes_state
    }
    pub fn include(&mut self, other: Self) {
        self.io |= other.io;
        self.writes_state |= other.writes_state;
        self.inspects |= other.inspects;
    }
}
#[derive(Clone, Debug)]
pub struct Program {
    pub model: Model,
    pub check: Check,
    pub constructors: BTreeMap<String, Constructor>,
    pub aliases: BTreeMap<String, String>,
    pub tables: BTreeMap<String, Table>,
    pub handlers: BTreeMap<String, Handler>,
    pub functions: BTreeMap<String, Function>,
    pub actors: BTreeMap<String, Actor>,
    pub effects: BTreeMap<String, Effects>,
    pub code: Vec<Code>,
    pub entries: BTreeMap<String, usize>,
}
impl Program {
    pub fn build(model: Model, selected: Option<&str>) -> Result<Self> {
        if model.claims.is_empty() {
            return Err(Error::new(
                Span::default(),
                "model needs at least one invariant, property, or cover",
            ));
        }
        let check = if let Some(n) = selected {
            model
                .checks
                .iter()
                .find(|c| c.name == n)
                .ok_or_else(|| Error::new(Span::default(), format!("unknown check `{n}`")))?
        } else if model.checks.len() == 1 {
            &model.checks[0]
        } else {
            return Err(Error::new(
                Span::default(),
                "select one check with --check (exactly one required without selection)",
            ));
        }
        .clone();
        if !["actors-v0", "actors-v1", "actors-v2", "cf-core-v0"]
            .contains(&check.semantics.as_str())
        {
            return Err(Error::new(
                check.span,
                "unsupported semantics; actor models use actors-v0, actors-v1 or actors-v2",
            ));
        }
        if check.semantics == "cf-core-v0"
            && model
                .functions
                .iter()
                .any(|f| !f.name.starts_with("$legacy."))
        {
            return Err(Error::new(
                check.span,
                "general functions/actors require the \"actors-v0\" profile",
            ));
        }
        if model
            .actors
            .iter()
            .any(|a| a.v2 != (check.semantics == "actors-v2"))
        {
            return Err(Error::new(
                check.span,
                "actors-v2 requires unified actor declarations; older profiles require legacy actor declarations",
            ));
        }
        if (check.semantics == "actors-v2") != check.mailbox_bound.is_some() {
            return Err(Error::new(
                check.span,
                "actors-v2 requires mailbox_bound; older profiles cannot declare one",
            ));
        }
        if check.message_bound.is_some() && check.semantics != "actors-v2" {
            return Err(Error::new(check.span, "message_bound requires actors-v2"));
        }
        if check.semantics == "actors-v2" && !model.tables.is_empty() {
            return Err(Error::new(
                check.span,
                "actors-v2 does not support D1 tables",
            ));
        }
        let mut p = Self {
            model,
            check,
            constructors: BTreeMap::new(),
            aliases: BTreeMap::new(),
            tables: BTreeMap::new(),
            handlers: BTreeMap::new(),
            functions: BTreeMap::new(),
            actors: BTreeMap::new(),
            effects: BTreeMap::new(),
            code: vec![],
            entries: BTreeMap::new(),
        };
        let mut type_names = BTreeSet::from([
            "Bool".to_owned(),
            "Int".to_owned(),
            "String".to_owned(),
            "unit".to_owned(),
            "DataError".to_owned(),
            "Actor".to_owned(),
            "Address".to_owned(),
        ]);
        for d in &p.model.types {
            if !type_names.insert(d.name.clone()) {
                return Err(Error::new(d.span, "duplicate or reserved type name"));
            }
        }
        for t in &p.model.tables {
            if !type_names.insert(t.name.clone())
                || p.tables.insert(t.path.clone(), t.clone()).is_some()
            {
                return Err(Error::new(t.span, "duplicate table/type name"));
            }
        }
        for d in &p.model.types {
            if d.variants.len() == 1
                && d.variants[0].payload.is_empty()
                && d.variants[0].fields.is_empty()
                && d.variants[0].name != d.name
                && type_names.contains(&d.variants[0].name)
            {
                p.aliases.insert(d.name.clone(), d.variants[0].name.clone());
                continue;
            }
            for v in &d.variants {
                if [
                    "None",
                    "Some",
                    "Ok",
                    "Err",
                    "ConstraintViolation",
                    "MissingRow",
                ]
                .contains(&v.name.as_str())
                    || p.constructors
                        .insert(
                            v.name.clone(),
                            Constructor {
                                ty: d.name.clone(),
                                payload: v.payload.clone(),
                                fields: v.fields.clone(),
                            },
                        )
                        .is_some()
                {
                    return Err(Error::new(d.span, "duplicate or reserved constructor"));
                }
            }
        }
        for t in p.tables.values() {
            if t.fields.values().filter(|f| f.primary).count() != 1 {
                return Err(Error::new(t.span, "table requires exactly one primary_key"));
            }
            if p.constructors
                .insert(
                    t.name.clone(),
                    Constructor {
                        ty: t.name.clone(),
                        payload: vec![],
                        fields: t
                            .fields
                            .iter()
                            .map(|(n, f)| (n.clone(), f.ty.clone()))
                            .collect(),
                    },
                )
                .is_some()
            {
                return Err(Error::new(t.span, "duplicate table constructor"));
            }
        }
        for n in ["ConstraintViolation", "MissingRow"] {
            p.constructors.insert(
                n.into(),
                Constructor {
                    ty: "DataError".into(),
                    payload: vec![],
                    fields: BTreeMap::new(),
                },
            );
        }
        for d in &p.model.types {
            p.resolve(&Type::named(&d.name), d.span)?;
        }
        for c in p.constructors.values() {
            for t in c.payload.iter().chain(c.fields.values()) {
                p.resolve(t, Span::default())?;
            }
        }
        for t in p.tables.values() {
            for f in t.fields.values() {
                let ty = p.resolve(&f.ty, t.span)?;
                if f.primary && matches!(ty, Ty::Option(_)) {
                    return Err(Error::new(t.span, "primary key cannot be optional"));
                }
            }
        }
        p.bind_actors_and_functions()?;
        let mut names = BTreeSet::new();
        for c in &p.model.claims {
            if !names.insert(c.name.clone()) {
                return Err(Error::new(c.span, "claim names must be unique"));
            }
        }
        names.clear();
        for c in &p.model.checks {
            if !names.insert(c.name.clone()) {
                return Err(Error::new(c.span, "check names must be unique"));
            }
        }
        let env = BTreeMap::new();
        for (domain, xs) in &p.check.domains {
            if !["Int", "String"].contains(&domain.as_str()) {
                return Err(Error::new(
                    p.check.span,
                    "only Int and String need explicit domains; variants are closed",
                ));
            }
            if xs.len() > 1025 {
                return Err(Error::new(p.check.span, "domain exceeds 1025 values"));
            }
            for x in xs {
                let actual = p.type_expr(x, &env, None, false, false)?;
                p.require(&Ty::named(domain), &actual, x.span)?;
            }
        }
        for (path, xs) in &p.check.init {
            let t = p
                .tables
                .get(path)
                .ok_or_else(|| Error::new(p.check.span, format!("unknown table `{path}`")))?;
            for x in xs {
                let actual = p.type_expr(x, &env, None, false, false)?;
                p.require(&Ty::named(&t.name), &actual, x.span)?;
            }
        }
        for i in &p.check.inputs {
            if i.via_send != (p.check.semantics == "actors-v2") {
                return Err(Error::new(
                    i.span,
                    "actors-v2 inputs require once send(address, message); older profiles require handler inputs",
                ));
            }
            let actor = i.handler.split('.').next().and_then(|n| p.actors.get(n));
            match (actor.and_then(|a| a.key.as_ref()), i.key.as_ref()) {
                (Some((_, ty)), Some(key)) => {
                    if p.check.semantics != "actors-v1" && p.check.semantics != "actors-v2" {
                        return Err(Error::new(
                            i.span,
                            "keyed actors require actors-v1 or actors-v2",
                        ));
                    }
                    let actual = p.type_expr(key, &env, None, false, false)?;
                    p.require(&p.resolve(ty, key.span)?, &actual, key.span)?;
                }
                (Some(_), None) => {
                    return Err(Error::new(
                        i.span,
                        "keyed actor input requires Actor.at(key).method",
                    ));
                }
                (None, Some(_)) => {
                    return Err(Error::new(
                        i.span,
                        "only keyed actors accept Actor.at(key).method",
                    ));
                }
                _ => {}
            }
            let h = p
                .handlers
                .get(&i.handler)
                .ok_or_else(|| Error::new(i.span, "unknown input handler"))?;
            let actual = p.type_expr(&i.value, &env, None, false, false)?;
            p.require(&p.resolve(&h.input, h.span)?, &actual, i.span)?;
        }
        for actor in p.actors.values() {
            if actor.key.is_some()
                && p.check.semantics != "actors-v1"
                && p.check.semantics != "actors-v2"
            {
                return Err(Error::new(
                    actor.span,
                    "keyed actors require actors-v1 or actors-v2",
                ));
            }
            if let Some((ty, initial)) = &actor.state {
                if actor.v2 {
                    continue;
                }
                let mut init_env = env.clone();
                if let Some((name, key_ty)) = &actor.key {
                    init_env.insert(name.clone(), p.resolve(key_ty, actor.span)?);
                }
                let actual = p.type_expr(initial, &init_env, None, false, false)?;
                p.require(&p.resolve(ty, initial.span)?, &actual, initial.span)?;
            }
        }
        for f in p.functions.values() {
            if p.check.semantics == "actors-v2" && f.params.iter().any(|(_, t)| t.name == "Actor") {
                return Err(Error::new(
                    f.span,
                    "actors-v2 has no Actor<State> owner capability",
                ));
            }
            let output = p.resolve(&f.output, f.span)?;
            let mut env = BTreeMap::new();
            for (name, ty) in &f.params {
                if p.is_global(name) || env.insert(name.clone(), p.resolve(ty, f.span)?).is_some() {
                    return Err(Error::new(
                        f.span,
                        "duplicate parameter or parameter shadows a global name",
                    ));
                }
            }
            let terminal = p.type_block(
                &f.body,
                &mut env,
                &output,
                p.effects[&f.name].inspects,
                true,
            )?;
            if !terminal && output != Ty::named("unit") {
                return Err(Error::new(
                    f.span,
                    "non-unit function needs a result on every branch",
                ));
            }
        }
        for c in &p.model.claims {
            let actual = p.type_expr(&c.body, &env, None, true, false)?;
            if c.kind == ClaimKind::Property {
                if actual != Ty::Temporal {
                    return Err(Error::new(
                        c.span,
                        "property requires a temporal operator; use invariant for state predicates",
                    ));
                }
                validate_temporal(&c.body)?;
            } else {
                p.require(&Ty::named("Bool"), &actual, c.span)?;
            }
        }
        let functions: Vec<_> = p.functions.values().cloned().collect();
        for f in functions {
            let end = p.push(Instruction::End, f.span);
            let entry = p.lower(&f.body, end, true);
            p.entries.insert(f.name, entry);
        }
        Ok(p)
    }
    pub fn resolve(&self, t: &Type, span: Span) -> Result<Ty> {
        if t.name == "Address" && t.args.len() == 1 {
            let actor = &t.args[0];
            if !actor.args.is_empty()
                || !self
                    .model
                    .actors
                    .iter()
                    .any(|a| a.name == actor.name && (a.key.is_some() || a.v2))
            {
                return Err(Error::new(
                    span,
                    "Address<ActorName> requires a keyed actor (or an actors-v2 singleton)",
                ));
            }
            return Ok(Ty::Address(actor.name.clone()));
        }
        if t.name == "Actor" && t.args.len() == 1 {
            return Ok(Ty::Actor(Box::new(self.resolve(&t.args[0], span)?)));
        }
        if t.name == "Option" && t.args.len() == 1 {
            return Ok(Ty::Option(Box::new(self.resolve(&t.args[0], span)?)));
        }
        if t.name == "Result" && t.args.len() == 2 {
            return Ok(Ty::Result(
                Box::new(self.resolve(&t.args[0], span)?),
                Box::new(self.resolve(&t.args[1], span)?),
            ));
        }
        if !t.args.is_empty() {
            return Err(Error::new(span, "unsupported generic type"));
        }
        let mut n = t.name.clone();
        let mut seen = BTreeSet::new();
        while let Some(a) = self.aliases.get(&n) {
            if !seen.insert(n.clone()) {
                return Err(Error::new(span, "cyclic type alias"));
            }
            n = a.clone();
        }
        if ["Bool", "Int", "String", "unit", "DataError"].contains(&n.as_str())
            || self.model.types.iter().any(|d| d.name == n)
            || self.tables.values().any(|t| t.name == n)
        {
            Ok(Ty::Named(n))
        } else {
            Err(Error::new(span, format!("unknown type `{n}`")))
        }
    }
    fn require(&self, expected: &Ty, actual: &Ty, span: Span) -> Result<()> {
        if expected.compatible(actual) {
            Ok(())
        } else {
            Err(Error::new(
                span,
                format!("type mismatch: expected {expected:?}, found {actual:?}"),
            ))
        }
    }
    fn type_block(
        &self,
        body: &[Stmt],
        env: &mut BTreeMap<String, Ty>,
        output: &Ty,
        property: bool,
        tail: bool,
    ) -> Result<bool> {
        let mut terminal = false;
        for (index, s) in body.iter().enumerate() {
            let returns = tail && index + 1 == body.len();
            match &s.kind {
                StmtKind::Let(n, e) => {
                    let t = self.type_expr(e, env, Some(output), property, true)?;
                    if matches!(t, Ty::Result(..)) {
                        let handled = body.get(index + 1).is_some_and(|next| matches!(
                            &next.kind,
                            StmtKind::Match(Expr { kind: ExprKind::Name(bound), .. }, _) if bound == n
                        ));
                        if !handled {
                            return Err(Error::new(
                                s.span,
                                "a Result binding must be matched immediately in this language slice",
                            ));
                        }
                    }
                    if self.is_global(n) || env.insert(n.clone(), t).is_some() {
                        return Err(Error::new(s.span, "local shadowing is not supported"));
                    }
                }
                StmtKind::Expr(e) => {
                    let t = self.type_expr(e, env, Some(output), property, true)?;
                    if returns {
                        self.require(output, &t, e.span)?;
                        terminal = true;
                    } else if matches!(t, Ty::Result(..)) {
                        return Err(Error::new(
                            e.span,
                            "handle the Result of this operation with match",
                        ));
                    }
                }
                StmtKind::Match(e, arms) => {
                    let ty = self.type_expr(e, env, Some(output), property, false)?;
                    if matches!(ty, Ty::Result(..))
                        && arms
                            .iter()
                            .any(|(p, _)| !matches!(p, Pattern::Variant(_, _)))
                    {
                        return Err(Error::new(
                            e.span,
                            "match Result explicitly with Ok and Err arms",
                        ));
                    }
                    let mut covered = BTreeSet::new();
                    let mut wildcard = false;
                    terminal = !arms.is_empty();
                    for (pat, statements) in arms {
                        if wildcard {
                            return Err(Error::new(s.span, "unreachable match arm"));
                        }
                        let mut inner = env.clone();
                        self.pattern_types(pat, &ty, &mut inner, s.span)?;
                        match pat {
                            Pattern::Wild | Pattern::Bind(_) => wildcard = true,
                            Pattern::Variant(n, _) => {
                                if !covered.insert(n.clone()) {
                                    return Err(Error::new(s.span, "duplicate match arm"));
                                }
                            }
                        }
                        terminal &=
                            self.type_block(statements, &mut inner, output, property, returns)?;
                    }
                    let cases: BTreeSet<_> = match &ty {
                        Ty::Option(_) => ["None".into(), "Some".into()].into(),
                        Ty::Result(..) => ["Ok".into(), "Err".into()].into(),
                        Ty::Named(n) => self
                            .constructors
                            .iter()
                            .filter(|(_, c)| &c.ty == n)
                            .map(|(k, _)| k.clone())
                            .collect(),
                        _ => BTreeSet::new(),
                    };
                    if !wildcard && (cases.is_empty() || cases != covered) {
                        return Err(Error::new(s.span, "non-exhaustive match"));
                    }
                }
            }
        }
        Ok(terminal)
    }
    fn pattern_types(
        &self,
        p: &Pattern,
        ty: &Ty,
        env: &mut BTreeMap<String, Ty>,
        span: Span,
    ) -> Result<()> {
        match p {
            Pattern::Wild => Ok(()),
            Pattern::Bind(n) => {
                if self.is_global(n) || env.insert(n.clone(), ty.clone()).is_some() {
                    Err(Error::new(span, "pattern binding shadows another variable"))
                } else {
                    Ok(())
                }
            }
            Pattern::Variant(n, ps) => {
                let args = match (n.as_str(), ty) {
                    ("None", Ty::Option(_)) => vec![],
                    ("Some", Ty::Option(t)) => vec![*t.clone()],
                    ("Ok", Ty::Result(t, _)) => vec![*t.clone()],
                    ("Err", Ty::Result(_, t)) => vec![*t.clone()],
                    _ => {
                        let c = self
                            .constructors
                            .get(n)
                            .ok_or_else(|| Error::new(span, "unknown pattern constructor"))?;
                        self.require(&Ty::named(&c.ty), ty, span)?;
                        if !c.fields.is_empty() {
                            return Err(Error::new(
                                span,
                                "record constructor patterns are not supported yet; bind the record instead",
                            ));
                        }
                        c.payload
                            .iter()
                            .map(|t| self.resolve(t, span))
                            .collect::<Result<Vec<_>>>()?
                    }
                };
                if args.len() != ps.len() {
                    return Err(Error::new(span, "pattern arity mismatch"));
                }
                for (p, t) in ps.iter().zip(args.iter()) {
                    if matches!(p, Pattern::Variant(..)) {
                        return Err(Error::new(
                            span,
                            "nested constructor patterns are not supported yet; match the bound payload separately",
                        ));
                    }
                    self.pattern_types(p, t, env, span)?;
                }
                Ok(())
            }
        }
    }
    // Retain the signature during the spike to keep the type/effect call sites aligned;
    // remove the now-redundant output context when the function IR is consolidated.
    #[allow(clippy::only_used_in_recursion)]
    pub fn type_expr(
        &self,
        e: &Expr,
        env: &BTreeMap<String, Ty>,
        output: Option<&Ty>,
        property: bool,
        effect: bool,
    ) -> Result<Ty> {
        let pure = |e: &Expr| self.type_expr(e, env, output, property, false);
        let boolty = Ty::named("Bool");
        if let ExprKind::Field(_, field) = &e.kind
            && field == "state"
            && let Some((path, Some(key))) = actor_target(e)
            && let Some(name) = path.strip_suffix(".state")
            && let Some(actor) = self.actors.get(name)
        {
            if !property {
                return Err(Error::new(
                    e.span,
                    "actor state inspection is property-only",
                ));
            }
            let (_, key_ty) = actor
                .key
                .as_ref()
                .ok_or_else(|| Error::new(e.span, "actor is not keyed"))?;
            self.require(&self.resolve(key_ty, key.span)?, &pure(&key)?, key.span)?;
            let (ty, _) = actor
                .state
                .as_ref()
                .ok_or_else(|| Error::new(e.span, "stateless actors have no state"))?;
            return self.resolve(ty, e.span);
        }
        if let Some(actor) = e
            .path()
            .and_then(|path| path.strip_suffix(".state").and_then(|n| self.actors.get(n)))
        {
            if actor.key.is_some() {
                return Err(Error::new(
                    e.span,
                    "keyed state inspection requires Actor.at(key).state",
                ));
            }
            if !property {
                return Err(Error::new(
                    e.span,
                    if self.check.semantics == "actors-v2" {
                        "actor state inspection is property-only; handlers use their state parameter"
                    } else {
                        "actor state inspection is property-only; handlers use their own Actor<State> capability"
                    },
                ));
            }
            let (ty, _) = actor
                .state
                .as_ref()
                .ok_or_else(|| Error::new(e.span, "stateless actors have no state"))?;
            return self.resolve(ty, e.span);
        }
        if let ExprKind::Call(target, args) = &e.kind
            && let ExprKind::Field(actor, method) = &target.kind
            && method == "at"
            && let ExprKind::Name(name) = &actor.kind
            && let Some(decl) = self.actors.get(name)
        {
            let (_, key_ty) = decl
                .key
                .as_ref()
                .ok_or_else(|| Error::new(e.span, "only keyed actors have .at(key) addresses"))?;
            if args.len() != 1 {
                return Err(Error::new(e.span, "Actor.at requires exactly one key"));
            }
            self.require(
                &self.resolve(key_ty, args[0].span)?,
                &pure(&args[0])?,
                args[0].span,
            )?;
            return Ok(Ty::Address(name.clone()));
        }
        if let Some(path) = e.path()
            && property
            && path.ends_with(".rows")
        {
            let t = path.trim_end_matches(".rows");
            if self.tables.contains_key(t) {
                return Ok(Ty::Rows(t.into()));
            }
        }
        match &e.kind {
            ExprKind::Bool(_) => Ok(boolty),
            ExprKind::Int(_) => Ok(Ty::named("Int")),
            ExprKind::String(_) => Ok(Ty::named("String")),
            ExprKind::Unit => Ok(Ty::named("unit")),
            ExprKind::Name(n) => {
                if self.check.semantics == "actors-v2"
                    && self.actors.get(n).is_some_and(|a| a.key.is_none())
                {
                    return Ok(Ty::Address(n.clone()));
                }
                if let Some(t) = env.get(n) {
                    return Ok(t.clone());
                }
                if n == "None" {
                    return Ok(Ty::Option(Box::new(Ty::Never)));
                }
                if let Some(c) = self.constructors.get(n) {
                    if !c.payload.is_empty() || !c.fields.is_empty() {
                        return Err(Error::new(e.span, "constructor requires payload/fields"));
                    }
                    return Ok(Ty::named(&c.ty));
                }
                if property && let Ok(t) = self.resolve(&Type::named(n), e.span) {
                    return Ok(Ty::List(Box::new(t)));
                }
                Err(Error::new(e.span, format!("unknown name `{n}`")))
            }
            ExprKind::Record(n, fs) => {
                let c = self
                    .constructors
                    .get(n)
                    .ok_or_else(|| Error::new(e.span, "unknown record constructor"))?;
                if !c.payload.is_empty() || c.fields.is_empty() || !c.fields.keys().eq(fs.keys()) {
                    return Err(Error::new(
                        e.span,
                        "record fields do not match its declaration",
                    ));
                }
                for (n, x) in fs {
                    self.require(&self.resolve(&c.fields[n], x.span)?, &pure(x)?, x.span)?;
                }
                Ok(Ty::named(&c.ty))
            }
            ExprKind::List(xs) => {
                let mut ty = Ty::Never;
                for x in xs {
                    let t = pure(x)?;
                    ty = ty.merge(&t).ok_or_else(|| {
                        Error::new(x.span, "list elements must have a single compatible type")
                    })?;
                }
                Ok(Ty::List(Box::new(ty)))
            }
            ExprKind::Field(x, n) => match pure(x)? {
                Ty::Actor(state) if n == "state" => Ok(*state),
                Ty::Input(actor) => match n.as_str() {
                    "submitted" | "processed" => Ok(boolty),
                    "payload" => self.resolve(
                        &self.handlers[&format!("{actor}.handle_message")].input,
                        e.span,
                    ),
                    "target" => Ok(Ty::Address(actor)),
                    _ => Err(Error::new(e.span, "unknown input observation field")),
                },
                Ty::Message(actor) => match n.as_str() {
                    "sent" | "processed" | "external" => Ok(boolty),
                    "payload" => Ok(Ty::Option(Box::new(self.resolve(
                        &self.handlers[&format!("{actor}.handle_message")].input,
                        e.span,
                    )?))),
                    "target" => Ok(Ty::Option(Box::new(Ty::Address(actor)))),
                    _ => Err(Error::new(e.span, "unknown message observation field")),
                },
                Ty::Request(h) => {
                    let handler = &self.handlers[&h];
                    match n.as_str() {
                        "input" => self.resolve(&handler.input, e.span),
                        "response" => {
                            Ok(Ty::Option(Box::new(self.resolve(&handler.output, e.span)?)))
                        }
                        "accepted" | "completed" => Ok(boolty),
                        _ => Err(Error::new(e.span, "unknown request field")),
                    }
                }
                Ty::Named(t) => {
                    let cs: Vec<_> = self.constructors.values().filter(|c| c.ty == t).collect();
                    if cs.len() != 1 {
                        return Err(Error::new(e.span, "field access requires a record type"));
                    }
                    let ty = cs[0]
                        .fields
                        .get(n)
                        .ok_or_else(|| Error::new(e.span, format!("unknown field `{n}`")))?;
                    self.resolve(ty, e.span)
                }
                _ => Err(Error::new(e.span, "field access requires a record")),
            },
            ExprKind::Call(f, args) => {
                let path = f.path().unwrap_or_default();
                if path == "respond" {
                    return Err(Error::new(
                        e.span,
                        "functions return their last expression; replace respond(value) with value",
                    ));
                }
                if let Some(function) = self.functions.get(&path) {
                    let fx = self.effects[&path];
                    if (fx.suspends_or_writes() && (property || !effect))
                        || (fx.inspects && !property)
                    {
                        return Err(Error::new(
                            e.span,
                            "function effects are not allowed here (effectful calls must be direct statements/bindings; inspectors are specification-only)",
                        ));
                    }
                    if function.params.len() != args.len() {
                        return Err(Error::new(e.span, "function argument count mismatch"));
                    }
                    for ((_, ty), arg) in function.params.iter().zip(args) {
                        self.require(&self.resolve(ty, arg.span)?, &pure(arg)?, arg.span)?;
                    }
                    return self.resolve(&function.output, e.span);
                }
                if path == "send" {
                    if self.check.semantics != "actors-v2" || property || !effect || args.len() != 2
                    {
                        return Err(Error::new(
                            e.span,
                            "send(address, message) is a direct actors-v2 handler effect",
                        ));
                    }
                    let Ty::Address(actor) = pure(&args[0])? else {
                        return Err(Error::new(
                            args[0].span,
                            "send requires a typed actor address",
                        ));
                    };
                    let h = &self.handlers[&format!("{actor}.handle_message")];
                    self.require(
                        &self.resolve(&h.input, e.span)?,
                        &pure(&args[1])?,
                        args[1].span,
                    )?;
                    return Ok(Ty::named("unit"));
                }
                if path == "call" {
                    if self.check.semantics != "actors-v1" {
                        return Err(Error::new(
                            e.span,
                            "actor calls require the actors-v1 profile",
                        ));
                    }
                    if property || !effect || args.len() != 2 {
                        return Err(Error::new(
                            e.span,
                            "call(handler, message) must be a direct handler statement or let binding",
                        ));
                    }
                    let static_target = actor_target(&args[0])
                        .filter(|(target, _)| self.handlers.contains_key(target));
                    let dynamic = static_target.is_none();
                    let (target, key) = if let Some(target) = static_target {
                        target
                    } else if let ExprKind::Field(receiver, method) = &args[0].kind {
                        let Ty::Address(name) = pure(receiver)? else {
                            return Err(Error::new(
                                args[0].span,
                                "call requires a known actor handler or typed address",
                            ));
                        };
                        (format!("{name}.{method}"), None)
                    } else {
                        return Err(Error::new(
                            args[0].span,
                            "call requires a known actor handler or typed address",
                        ));
                    };
                    let handler = self.handlers.get(&target).ok_or_else(|| {
                        Error::new(args[0].span, "call requires a known actor handler")
                    })?;
                    match (
                        self.actors[&handler.actor].key.as_ref(),
                        key.as_ref(),
                        dynamic,
                    ) {
                        (Some(_), None, true) => {}
                        (Some((_, ty)), Some(k), _) => {
                            self.require(&self.resolve(ty, k.span)?, &pure(k)?, k.span)?;
                        }
                        (Some(_), None, _) => {
                            return Err(Error::new(
                                args[0].span,
                                "keyed call requires Actor.at(key).method",
                            ));
                        }
                        (None, Some(_), _) => {
                            return Err(Error::new(
                                args[0].span,
                                "only keyed actors accept Actor.at(key).method",
                            ));
                        }
                        _ => {}
                    }
                    self.require(
                        &self.resolve(&handler.input, e.span)?,
                        &pure(&args[1])?,
                        args[1].span,
                    )?;
                    return self.resolve(&handler.output, e.span);
                }
                if self.check.semantics == "actors-v2" && (path == "inputs" || path == "messages") {
                    if !property || args.len() != 1 {
                        return Err(Error::new(
                            e.span,
                            "inputs/messages is a specification-only view of one actor declaration",
                        ));
                    }
                    let actor = args[0].path().filter(|n| self.actors.contains_key(n)).ok_or_else(|| Error::new(e.span, "inputs/messages requires an actor declaration, not a keyed address"))?;
                    if path == "messages" && self.check.message_bound.is_none() {
                        return Err(Error::new(
                            e.span,
                            "messages(Actor) requires an explicit message_bound",
                        ));
                    }
                    return Ok(Ty::List(Box::new(if path == "inputs" {
                        Ty::Input(actor)
                    } else {
                        Ty::Message(actor)
                    })));
                }
                if path == "requests" {
                    if self.check.semantics == "actors-v2" {
                        return Err(Error::new(
                            e.span,
                            "requests(Actor.method) is not an actors-v2 message inspector",
                        ));
                    }
                    if !property || args.len() != 1 {
                        return Err(Error::new(
                            e.span,
                            "requests is a property-only view of one handler",
                        ));
                    }
                    let h = args[0]
                        .path()
                        .filter(|h| self.handlers.contains_key(h))
                        .ok_or_else(|| Error::new(e.span, "unknown handler in requests view"))?;
                    return Ok(Ty::List(Box::new(Ty::Request(h))));
                }
                if ["Some", "Ok", "Err"].contains(&path.as_str()) {
                    if args.len() != 1 {
                        return Err(Error::new(e.span, "constructor requires one argument"));
                    }
                    let t = Box::new(pure(&args[0])?);
                    return Ok(match path.as_str() {
                        "Some" => Ty::Option(t),
                        "Ok" => Ty::Result(t, Box::new(Ty::Never)),
                        _ => Ty::Result(Box::new(Ty::Never), t),
                    });
                }
                if let Some(c) = self.constructors.get(&path) {
                    if !c.fields.is_empty() || c.payload.len() != args.len() {
                        return Err(Error::new(e.span, "constructor arity mismatch"));
                    }
                    for (t, x) in c.payload.iter().zip(args) {
                        self.require(&self.resolve(t, e.span)?, &pure(x)?, x.span)?;
                    }
                    return Ok(Ty::named(&c.ty));
                }
                if let ExprKind::Field(receiver, method) = &f.kind {
                    if method == "set" {
                        if property || !effect || args.len() != 1 {
                            return Err(Error::new(
                                e.span,
                                "Actor.set(value) is a direct state effect",
                            ));
                        }
                        let Ty::Actor(state) = pure(receiver)? else {
                            return Err(Error::new(
                                e.span,
                                "set requires an owned Actor<State> capability",
                            ));
                        };
                        self.require(&state, &pure(&args[0])?, e.span)?;
                        return Ok(Ty::named("unit"));
                    }
                    if let Some(t) = receiver.path().and_then(|p| self.tables.get(&p)) {
                        if property || !effect {
                            return Err(Error::new(
                                e.span,
                                "resource operations must be direct handler statements or let bindings; properties inspect .rows",
                            ));
                        }
                        let key = &t
                            .fields
                            .values()
                            .find(|f| f.primary)
                            .expect("validated key")
                            .ty;
                        let expected = match method.as_str() {
                            "get" | "delete" => vec![self.resolve(key, e.span)?],
                            "insert" => vec![Ty::named(&t.name)],
                            "update" => vec![self.resolve(key, e.span)?, Ty::named(&t.name)],
                            _ => return Err(Error::new(e.span, "unsupported D1 operation")),
                        };
                        if expected.len() != args.len() {
                            return Err(Error::new(e.span, "D1 operation arity mismatch"));
                        }
                        for (t, x) in expected.iter().zip(args) {
                            self.require(t, &pure(x)?, x.span)?;
                        }
                        return Ok(if method == "get" {
                            Ty::Option(Box::new(Ty::named(&t.name)))
                        } else {
                            Ty::Result(
                                Box::new(Ty::named("unit")),
                                Box::new(Ty::named("DataError")),
                            )
                        });
                    }
                    if method == "contains_key" {
                        let Ty::Rows(path) = pure(receiver)? else {
                            return Err(Error::new(
                                e.span,
                                "contains_key requires a table rows view",
                            ));
                        };
                        if args.len() != 1 {
                            return Err(Error::new(e.span, "contains_key requires one key"));
                        }
                        let key = &self.tables[&path]
                            .fields
                            .values()
                            .find(|f| f.primary)
                            .expect("validated key")
                            .ty;
                        self.require(&self.resolve(key, e.span)?, &pure(&args[0])?, e.span)?;
                        return Ok(boolty);
                    }
                }
                Err(Error::new(
                    e.span,
                    format!("unknown or unsupported operation `{path}`"),
                ))
            }
            ExprKind::Unary(op, x) => {
                let t = pure(x)?;
                match op.as_str() {
                    "always" | "eventually" => {
                        if !property || (t != boolty && t != Ty::Temporal) {
                            return Err(Error::new(
                                e.span,
                                "temporal operator requires a predicate/property",
                            ));
                        }
                        Ok(Ty::Temporal)
                    }
                    "!" | "not" => {
                        self.require(&boolty, &t, e.span)?;
                        Ok(boolty)
                    }
                    "-" => {
                        self.require(&Ty::named("Int"), &t, e.span)?;
                        Ok(t)
                    }
                    _ => Err(Error::new(e.span, "unsupported unary operator")),
                }
            }
            ExprKind::Binary(op, a, b) => {
                let at = pure(a)?;
                let bt = pure(b)?;
                match op.as_str() {
                    "leads_to" | "until" => {
                        if !property {
                            return Err(Error::new(e.span, "temporal operator outside property"));
                        }
                        self.require(&boolty, &at, a.span)?;
                        self.require(&boolty, &bt, b.span)?;
                        Ok(Ty::Temporal)
                    }
                    "implies" | "&&" | "||" => {
                        if property && (at == Ty::Temporal || bt == Ty::Temporal) {
                            if ![boolty.clone(), Ty::Temporal].contains(&at)
                                || ![boolty.clone(), Ty::Temporal].contains(&bt)
                            {
                                return Err(Error::new(e.span, "invalid temporal operand"));
                            }
                            Ok(Ty::Temporal)
                        } else {
                            self.require(&boolty, &at, a.span)?;
                            self.require(&boolty, &bt, b.span)?;
                            Ok(boolty)
                        }
                    }
                    "==" | "!=" => {
                        if at == Ty::Temporal || bt == Ty::Temporal {
                            return Err(Error::new(e.span, "cannot compare temporal formulas"));
                        }
                        self.require(&at, &bt, e.span)?;
                        Ok(boolty)
                    }
                    "+" | "-" | "<" | ">" | "<=" | ">=" => {
                        self.require(&Ty::named("Int"), &at, a.span)?;
                        self.require(&Ty::named("Int"), &bt, b.span)?;
                        Ok(if op == "+" || op == "-" { at } else { boolty })
                    }
                    _ => Err(Error::new(e.span, "unsupported binary operator")),
                }
            }
            ExprKind::Quant {
                all: _,
                var,
                domain,
                body,
            } => {
                if !property {
                    return Err(Error::new(e.span, "quantifiers are property-only"));
                }
                let t = match pure(domain)? {
                    Ty::List(t) => *t,
                    Ty::Rows(p) => Ty::named(&self.tables[&p].name),
                    _ => {
                        return Err(Error::new(
                            domain.span,
                            "quantifier requires a finite collection/domain",
                        ));
                    }
                };
                let mut env = env.clone();
                if env.insert(var.clone(), t).is_some() {
                    return Err(Error::new(e.span, "quantifier shadows variable"));
                }
                let t = self.type_expr(body, &env, output, property, false)?;
                if t != boolty && t != Ty::Temporal {
                    return Err(Error::new(
                        body.span,
                        "quantifier body must be Boolean or temporal",
                    ));
                }
                Ok(t)
            }
        }
    }
    fn push(&mut self, instruction: Instruction, span: Span) -> usize {
        let i = self.code.len();
        self.code.push(Code { instruction, span });
        i
    }
    fn lower(&mut self, body: &[Stmt], mut next: usize, tail: bool) -> usize {
        for (index, s) in body.iter().enumerate().rev() {
            let returns = tail && index + 1 == body.len();
            let instruction = match &s.kind {
                StmtKind::Let(n, e) => Instruction::Let {
                    name: n.clone(),
                    value: e.clone(),
                    next,
                },
                StmtKind::Expr(e) => {
                    if returns {
                        let result = Expr {
                            kind: ExprKind::Name("$result".into()),
                            span: e.span,
                        };
                        let ret = self.push(Instruction::Return(result), e.span);
                        Instruction::Let {
                            name: "$result".into(),
                            value: e.clone(),
                            next: ret,
                        }
                    } else {
                        Instruction::Eval {
                            value: e.clone(),
                            next,
                        }
                    }
                }
                StmtKind::Match(e, arms) => Instruction::Match {
                    value: e.clone(),
                    arms: arms
                        .iter()
                        .map(|(p, b)| (p.clone(), self.lower(b, next, returns)))
                        .collect(),
                },
            };
            next = self.push(instruction, s.span);
        }
        next
    }
}

/// This explicit whitelist is part of the checking contract, not a parser convenience.
pub fn validate_temporal(e: &Expr) -> Result<()> {
    use ExprKind::*;
    let state = |e: &Expr| !e.temporal();
    let valid = match &e.kind {
        Quant {
            all: true,
            domain,
            body,
            ..
        } => {
            let stable = matches!(domain.kind, Name(_))
                || matches!(&domain.kind,Call(f,_) if matches!(f.path().as_deref(), Some("requests" | "inputs" | "messages")));
            if !stable {
                return Err(Error::new(
                    domain.span,
                    "temporal quantification requires a stable type domain or requests/inputs/messages view",
                ));
            }
            validate_temporal(body)?;
            true
        }
        Binary(op, a, b) if op == "&&" => {
            validate_temporal(a)?;
            validate_temporal(b)?;
            true
        }
        Binary(op, a, b) if op == "leads_to" || op == "until" => state(a) && state(b),
        Unary(op, x) if op == "always" || op == "eventually" => {
            state(x)
                || match &x.kind {
                    Unary(inner, p) => {
                        ((op == "always" && inner == "eventually")
                            || (op == "eventually" && inner == "always"))
                            && state(p)
                    }
                    Binary(imp, p, q) if op == "always" && imp == "implies" && state(p) => {
                        matches!(&q.kind,Unary(g,r) if g=="always"&&state(r))
                    }
                    _ => false,
                }
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::new(
            e.span,
            "unsupported temporal fragment; use always, eventually, leads_to, until, GF, FG, or persistence with state predicates",
        ))
    }
}
