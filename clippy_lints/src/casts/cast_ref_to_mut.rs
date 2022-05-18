use clippy_utils::diagnostics::span_lint;
use rustc_hir::{Expr, ExprKind, MutTy, Mutability, TyKind, UnOp};
use rustc_lint::LateContext;
use rustc_middle::ty;

use super::CAST_REF_TO_MUT;

pub(super) fn check(cx: &LateContext<'_>, expr: &Expr<'_>) {
    if let ExprKind::Unary(UnOp::Deref, e) = &expr.kind
        && let ExprKind::Cast(e, t) = &e.kind
        && let TyKind::Ptr(MutTy { mutbl: Mutability::Mut, .. }) = t.kind
        && let ExprKind::Cast(e, t) = &e.kind
        && let TyKind::Ptr(MutTy { mutbl: Mutability::Not, .. }) = t.kind
        && let ty::Ref(..) = cx.typeck_results().node_type(e.hir_id).kind()
    {
        span_lint(
            cx,
            CAST_REF_TO_MUT,
            expr.span,
            "casting `&T` to `&mut T` may cause undefined behavior, consider instead using an `UnsafeCell`",
        );
    }
}
