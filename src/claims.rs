//! Classify explicit properties as safety, temporal or reachability obligations.
use crate::syntax::{ClaimKind, ExprKind, Model};

pub(crate) fn classify(model: &mut Model) {
    for claim in &mut model.claims {
        if claim.kind != ClaimKind::Property {
            continue;
        }
        if let ExprKind::Unary(operator, predicate) = &claim.body.kind {
            // Never turn GF/FG, nested reachability or other temporal formulas
            // into predicates. The type/temporal checker rejects invalid mixtures.
            if predicate.temporal() {
                continue;
            }
            let kind = match operator.as_str() {
                "always" => ClaimKind::Invariant,
                "reachable" => ClaimKind::Cover,
                _ => continue,
            };
            claim.kind = kind;
            claim.body = *predicate.clone();
        }
    }
}
