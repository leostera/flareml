//! Closed name, type and effect checking for finite message-driven models.
use crate::syntax::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Named(String),
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    List(Box<Ty>),
    Input(String),
    Message(String),
    Instance(String),
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
}
#[derive(Clone, Debug)]
pub struct Constructor {
    pub ty: String,
    pub payload: Vec<Type>,
    pub fields: BTreeMap<String, Type>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Effects {
    pub sends: bool,
    pub chooses: bool,
    pub spawns: bool,
    pub inspects: bool,
}
impl Effects {
    pub fn include(&mut self, other: Self) {
        self.sends |= other.sends;
        self.chooses |= other.chooses;
        self.spawns |= other.spawns;
        self.inspects |= other.inspects;
    }
}
#[derive(Clone, Debug)]
pub struct Program {
    pub model: Model,
    pub check: Check,
    pub constructors: BTreeMap<String, Constructor>,
    pub aliases: BTreeMap<String, String>,
    pub functions: BTreeMap<String, Function>,
    pub actors: BTreeMap<String, Actor>,
    pub effects: BTreeMap<String, Effects>,
}
impl Program {
    pub fn build(mut model: Model, selected: Option<&str>) -> Result<Self> {
        if model.claims.is_empty() {
            return Err(Error::new(
                Span::default(),
                "model needs at least one property",
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
        if check.mailbox_bound.is_none() {
            return Err(Error::new(check.span, "check requires mailbox_bound"));
        }
        crate::claims::classify(&mut model);
        let mut p = Self {
            model,
            check,
            constructors: BTreeMap::new(),
            aliases: BTreeMap::new(),
            functions: BTreeMap::new(),
            actors: BTreeMap::new(),
            effects: BTreeMap::new(),
        };
        let mut type_names: BTreeSet<String> = [
            "Bool",
            "Int",
            "String",
            "unit",
            "Option",
            "Result",
            "Actor",
            "choose",
            "spawn",
            "instances",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        for d in &p.model.types {
            if !type_names.insert(d.name.clone()) {
                return Err(Error::new(d.span, "duplicate or reserved type name"));
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
                    "send",
                    "choose",
                    "spawn",
                    "instances",
                    "inputs",
                    "messages",
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
        for d in &p.model.types {
            p.resolve(&Type::named(&d.name), d.span)?;
        }
        for c in p.constructors.values() {
            for t in c.payload.iter().chain(c.fields.values()) {
                p.resolve(t, Span::default())?;
            }
        }
        p.bind_actors_and_functions()?;
        let mut names = BTreeSet::new();
        for c in &p.model.claims {
            if !names.insert(&c.name) {
                return Err(Error::new(c.span, "claim names must be unique"));
            }
        }
        names.clear();
        for c in &p.model.checks {
            if !names.insert(&c.name) {
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
            if xs.is_empty() || xs.len() > 1025 {
                return Err(Error::new(
                    p.check.span,
                    "domain must contain 1..1025 values",
                ));
            }
            for x in xs {
                // Domains describe literal pools, not computations that can in turn
                // require domain checking or depend on initialized actor addresses.
                let literal = matches!(&x.kind, ExprKind::Int(_) | ExprKind::String(_))
                    || matches!(&x.kind, ExprKind::Unary(op, n) if op == "-" && matches!(n.kind, ExprKind::Int(_)));
                if !literal {
                    return Err(Error::new(x.span, "domain entries must be literals"));
                }
                p.require(
                    &Ty::named(domain),
                    &p.type_expr(x, &env, false, false)?,
                    x.span,
                )?;
            }
        }
        for f in p.functions.values() {
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
            p.type_block(&f.body, &mut env, &output, p.effects[&f.name].inspects)?;
        }
        for c in &p.model.claims {
            let actual = p.type_expr(&c.body, &env, true, false)?;
            if c.kind == ClaimKind::Property {
                if actual != Ty::Temporal {
                    return Err(Error::new(
                        c.span,
                        "property requires an explicit operator: always, a supported temporal form, or top-level reachable",
                    ));
                }
                validate_temporal(&c.body)?;
            } else {
                p.require(&Ty::named("Bool"), &actual, c.span)?;
            }
        }
        Ok(p)
    }
    fn message_type(&self, actor: &str) -> &Type {
        &self.functions[&self.actors[actor].handler]
            .params
            .last()
            .expect("validated handler arity")
            .1
    }
    pub fn resolve(&self, t: &Type, span: Span) -> Result<Ty> {
        if t.name == "Actor" && t.args.len() == 1 {
            let actor = &t.args[0];
            if !actor.args.is_empty() || !self.model.actors.iter().any(|a| a.name == actor.name) {
                return Err(Error::new(
                    span,
                    "Actor<ActorName> requires a declared actor",
                ));
            }
            return Ok(Ty::Address(actor.name.clone()));
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
        if ["Bool", "Int", "String", "unit"].contains(&n.as_str())
            || self.model.types.iter().any(|d| d.name == n)
        {
            Ok(Ty::Named(n))
        } else {
            Err(Error::new(span, format!("unknown type `{n}`")))
        }
    }
    fn require(&self, expected: &Ty, actual: &Ty, span: Span) -> Result<()> {
        if expected.merge(actual).is_some() {
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
    ) -> Result<()> {
        let actual = self.block_type(body, env, property)?;
        self.require(
            output,
            &actual,
            body.last().map_or(Span::default(), |s| s.span),
        )
    }
    fn block_type(
        &self,
        body: &[Stmt],
        env: &mut BTreeMap<String, Ty>,
        property: bool,
    ) -> Result<Ty> {
        let mut result = Ty::named("unit");
        for (index, s) in body.iter().enumerate() {
            result = match &s.kind {
                StmtKind::Inputs(inputs) => {
                    for input in inputs {
                        let Ty::Address(actor) =
                            self.type_expr(&input.target, env, false, false)?
                        else {
                            return Err(Error::new(
                                input.span,
                                "input target must be an actor reference",
                            ));
                        };
                        self.require(
                            &self.resolve(self.message_type(&actor), input.span)?,
                            &self.type_expr(&input.value, env, false, false)?,
                            input.span,
                        )?;
                    }
                    Ty::named("unit")
                }
                StmtKind::Let(n, e) => {
                    let t = if let ExprKind::Call(target, args) = &e.kind
                        && target.path().as_deref() == Some("choose")
                    {
                        if property {
                            return Err(Error::new(e.span, "choose is a handler-only effect"));
                        }
                        let [
                            Expr {
                                kind: ExprKind::List(xs),
                                ..
                            },
                        ] = args.as_slice()
                        else {
                            return Err(Error::new(
                                e.span,
                                "choose requires a nonempty list literal",
                            ));
                        };
                        if xs.is_empty() {
                            return Err(Error::new(
                                e.span,
                                "choose requires a nonempty list literal",
                            ));
                        }
                        let mut ty = Ty::Never;
                        for candidate in xs {
                            ty = ty
                                .merge(&self.type_expr(candidate, env, false, false)?)
                                .ok_or_else(|| {
                                    Error::new(
                                        candidate.span,
                                        "choose candidates must have a single compatible type",
                                    )
                                })?;
                        }
                        ty
                    } else {
                        self.type_expr(e, env, property, true)?
                    };
                    if matches!(t, Ty::Result(..)) && !body.get(index + 1).is_some_and(|next| matches!(
                        &next.kind, StmtKind::Match(Expr { kind: ExprKind::Name(bound), .. }, _) if bound == n)) {
                        return Err(Error::new(s.span, "a Result binding must be matched immediately"));
                    }
                    if self.is_global(n) || env.insert(n.clone(), t).is_some() {
                        return Err(Error::new(s.span, "local shadowing is not supported"));
                    }
                    Ty::named("unit")
                }
                StmtKind::Expr(e) => {
                    let t = self.type_expr(e, env, property, true)?;
                    if index + 1 < body.len() && matches!(t, Ty::Result(..)) {
                        return Err(Error::new(e.span, "handle the Result with match"));
                    }
                    t
                }
                StmtKind::Match(e, arms) => {
                    let ty = self.type_expr(e, env, property, false)?;
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
                    let mut branch_type = Ty::Never;
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
                        let t = self.block_type(statements, &mut inner, property)?;
                        if index + 1 < body.len() && matches!(t, Ty::Result(..)) {
                            return Err(Error::new(
                                s.span,
                                "handle the Result returned by this match",
                            ));
                        }
                        // Non-tail matches are statements: branch results are discarded.
                        // Tail matches must agree on the returned type on every branch.
                        if index + 1 == body.len() {
                            branch_type = branch_type.merge(&t).ok_or_else(|| {
                                Error::new(s.span, "match branches return incompatible types")
                            })?;
                        }
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
                    if index + 1 == body.len() {
                        branch_type
                    } else {
                        Ty::named("unit")
                    }
                }
            };
        }
        Ok(result)
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
                    ("Some", Ty::Option(t))
                    | ("Ok", Ty::Result(t, _))
                    | ("Err", Ty::Result(_, t)) => vec![*t.clone()],
                    _ => {
                        let c = self
                            .constructors
                            .get(n)
                            .ok_or_else(|| Error::new(span, "unknown pattern constructor"))?;
                        self.require(&Ty::named(&c.ty), ty, span)?;
                        if !c.fields.is_empty() {
                            return Err(Error::new(
                                span,
                                "record constructor patterns are not supported; bind the record instead",
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
                for (p, t) in ps.iter().zip(&args) {
                    if matches!(p, Pattern::Variant(..)) {
                        return Err(Error::new(
                            span,
                            "nested constructor patterns are not supported; match the bound payload separately",
                        ));
                    }
                    self.pattern_types(p, t, env, span)?;
                }
                Ok(())
            }
        }
    }
    pub fn type_expr(
        &self,
        e: &Expr,
        env: &BTreeMap<String, Ty>,
        property: bool,
        effect: bool,
    ) -> Result<Ty> {
        let pure = |e: &Expr| self.type_expr(e, env, property, false);
        let boolty = Ty::named("Bool");
        match &e.kind {
            ExprKind::Bool(_) => Ok(boolty),
            ExprKind::Int(_) => Ok(Ty::named("Int")),
            ExprKind::String(_) => Ok(Ty::named("String")),
            ExprKind::Unit => Ok(Ty::named("unit")),
            ExprKind::Name(n) => {
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
                    ty = ty.merge(&pure(x)?).ok_or_else(|| {
                        Error::new(x.span, "list elements must have a single compatible type")
                    })?;
                }
                Ok(Ty::List(Box::new(ty)))
            }
            ExprKind::Field(x, n) => match pure(x)? {
                Ty::Input(actor) => match n.as_str() {
                    "submitted" | "processed" => Ok(boolty),
                    "payload" => self.resolve(self.message_type(&actor), e.span),
                    "target" => Ok(Ty::Address(actor)),
                    _ => Err(Error::new(e.span, "unknown input observation field")),
                },
                Ty::Instance(actor) => match n.as_str() {
                    "created" => Ok(boolty),
                    "reference" => Ok(Ty::Option(Box::new(Ty::Address(actor)))),
                    "state" => {
                        let ty = self.actors[&actor].state.as_ref().ok_or_else(|| {
                            Error::new(e.span, "stateless instances have no state field")
                        })?;
                        Ok(Ty::Option(Box::new(self.resolve(ty, e.span)?)))
                    }
                    _ => Err(Error::new(e.span, "unknown instance observation field")),
                },
                Ty::Message(actor) => match n.as_str() {
                    "sent" | "processed" | "external" => Ok(boolty),
                    "payload" => Ok(Ty::Option(Box::new(
                        self.resolve(self.message_type(&actor), e.span)?,
                    ))),
                    "target" => Ok(Ty::Option(Box::new(Ty::Address(actor)))),
                    _ => Err(Error::new(e.span, "unknown message observation field")),
                },
                Ty::Named(t) => {
                    let cs: Vec<_> = self.constructors.values().filter(|c| c.ty == t).collect();
                    if cs.len() != 1 {
                        return Err(Error::new(e.span, "field access requires a record type"));
                    }
                    self.resolve(
                        cs[0]
                            .fields
                            .get(n)
                            .ok_or_else(|| Error::new(e.span, format!("unknown field `{n}`")))?,
                        e.span,
                    )
                }
                _ => Err(Error::new(e.span, "field access requires a record")),
            },
            ExprKind::Call(f, args) => {
                let path = f.path().unwrap_or_default();
                if let Some(function) = self.functions.get(&path) {
                    let fx = self.effects[&path];
                    if ((fx.sends || fx.chooses || fx.spawns) && (property || !effect))
                        || (fx.inspects && !property)
                    {
                        return Err(Error::new(
                            e.span,
                            "function effects are not allowed here (send/choice/spawn helpers must be direct statements/bindings; inspectors are specification-only)",
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
                if path == "spawn" {
                    let Some(Expr {
                        kind: ExprKind::Name(name),
                        ..
                    }) = args.first()
                    else {
                        return Err(Error::new(e.span, "spawn requires an actor definition"));
                    };
                    let actor = self
                        .actors
                        .get(name)
                        .ok_or_else(|| Error::new(e.span, "unknown actor definition"))?;
                    if property || !effect {
                        return Err(Error::new(
                            e.span,
                            "spawn must be a direct main/handler statement or binding",
                        ));
                    }
                    let params = actor
                        .initializer
                        .as_ref()
                        .map(|n| self.functions[n].params.as_slice())
                        .unwrap_or_default();
                    if args.len() != params.len() + 1 {
                        return Err(Error::new(
                            e.span,
                            "spawn initializer argument count mismatch",
                        ));
                    }
                    for ((_, ty), arg) in params.iter().zip(&args[1..]) {
                        self.require(&self.resolve(ty, arg.span)?, &pure(arg)?, arg.span)?;
                    }
                    return Ok(Ty::Address(name.clone()));
                }
                if path == "choose" {
                    return Err(Error::new(
                        e.span,
                        "choose must be the complete initializer of a local let binding",
                    ));
                }
                if path == "send" {
                    if property || !effect || args.len() != 2 {
                        return Err(Error::new(
                            e.span,
                            "send(address, message) is a direct handler effect",
                        ));
                    }
                    let Ty::Address(actor) = pure(&args[0])? else {
                        return Err(Error::new(
                            args[0].span,
                            "send requires a typed actor address",
                        ));
                    };
                    self.require(
                        &self.resolve(self.message_type(&actor), e.span)?,
                        &pure(&args[1])?,
                        args[1].span,
                    )?;
                    return Ok(Ty::named("unit"));
                }
                if path == "instances" {
                    if !property || args.len() != 1 {
                        return Err(Error::new(
                            e.span,
                            "instances is a specification-only view of one actor definition",
                        ));
                    }
                    let actor = args[0]
                        .path()
                        .filter(|a| self.actors.contains_key(a))
                        .ok_or_else(|| {
                            Error::new(e.span, "instances requires an actor definition")
                        })?;
                    return Ok(Ty::List(Box::new(Ty::Instance(actor))));
                }
                if path == "inputs" || path == "messages" {
                    if !property || args.len() != 1 {
                        return Err(Error::new(
                            e.span,
                            "inputs/messages is a specification-only view of one actor declaration",
                        ));
                    }
                    let actor = args[0]
                        .path()
                        .filter(|n| self.actors.contains_key(n))
                        .ok_or_else(|| {
                            Error::new(
                                e.span,
                                "inputs/messages requires an actor definition, not a reference",
                            )
                        })?;
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
                        self.require(&self.resolve(t, x.span)?, &pure(x)?, x.span)?;
                    }
                    return Ok(Ty::named(&c.ty));
                }
                Err(Error::new(
                    e.span,
                    format!("unknown or unsupported operation `{path}`"),
                ))
            }
            ExprKind::Unary(op, x) => {
                let t = pure(x)?;
                match op.as_str() {
                    "reachable" => Err(Error::new(
                        e.span,
                        "reachable must be the whole property body with a pure state predicate",
                    )),
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
                var, domain, body, ..
            } => {
                if !property {
                    return Err(Error::new(e.span, "quantifiers are property-only"));
                }
                // In collection position a declared type denotes its finite
                // domain, even when its constructor has the same name.
                let domain_type = if let ExprKind::Name(n) = &domain.kind
                    && !env.contains_key(n)
                    && self.model.types.iter().any(|d| &d.name == n)
                {
                    Ty::List(Box::new(self.resolve(&Type::named(n), domain.span)?))
                } else {
                    pure(domain)?
                };
                let Ty::List(t) = domain_type else {
                    return Err(Error::new(
                        domain.span,
                        "quantifier requires a finite collection/domain",
                    ));
                };
                let mut env = env.clone();
                if self.is_global(var) || env.insert(var.clone(), *t).is_some() {
                    return Err(Error::new(e.span, "quantifier shadows variable/global"));
                }
                let t = self.type_expr(body, &env, property, false)?;
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
}

/// An explicit whitelist, not arbitrary temporal or branching-time logic.
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
                || matches!(&domain.kind, Call(f, _) if matches!(f.path().as_deref(), Some("inputs" | "messages" | "instances")));
            if !stable {
                return Err(Error::new(
                    domain.span,
                    "temporal quantification requires a stable type domain or inputs/messages/instances view",
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
                        matches!(&q.kind, Unary(g, r) if g == "always" && state(r))
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
