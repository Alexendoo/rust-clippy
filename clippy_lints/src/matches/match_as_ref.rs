use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::source::snippet_with_applicability;
use clippy_utils::{is_lang_ctor, peel_blocks};
use rustc_errors::Applicability;
use rustc_hir::{Arm, BindingAnnotation, Expr, ExprKind, LangItem, PatKind, QPath};
use rustc_lint::LateContext;
use rustc_middle::ty;

use super::MATCH_AS_REF;

pub(crate) fn check(cx: &LateContext<'_>, ex: &Expr<'_>, arms: &[Arm<'_>], expr: &Expr<'_>) {
    if arms.len() == 2 && arms[0].guard.is_none() && arms[1].guard.is_none() {
        let arm_ref: Option<BindingAnnotation> = if is_none_arm(cx, &arms[0]) {
            is_ref_some_arm(cx, &arms[1])
        } else if is_none_arm(cx, &arms[1]) {
            is_ref_some_arm(cx, &arms[0])
        } else {
            None
        };
        if let Some(rb) = arm_ref {
            let suggestion = if rb == BindingAnnotation::Ref {
                "as_ref"
            } else {
                "as_mut"
            };

            let output_ty = cx.typeck_results().expr_ty(expr);
            let input_ty = cx.typeck_results().expr_ty(ex);

            let cast = if let ty::Adt(_, substs) = input_ty.kind()
                && let input_ty = substs.type_at(0)
                && let ty::Adt(_, substs) = output_ty.kind()
                && let output_ty = substs.type_at(0)
                && let ty::Ref(_, output_ty, _) = *output_ty.kind()
                && input_ty != output_ty
            {
                ".map(|x| x as _)"
            } else {
                ""
            };

            let mut applicability = Applicability::MachineApplicable;
            span_lint_and_sugg(
                cx,
                MATCH_AS_REF,
                expr.span,
                &format!("use `{}()` instead", suggestion),
                "try this",
                format!(
                    "{}.{}(){}",
                    snippet_with_applicability(cx, ex.span, "_", &mut applicability),
                    suggestion,
                    cast,
                ),
                applicability,
            );
        }
    }
}

// Checks if arm has the form `None => None`
fn is_none_arm(cx: &LateContext<'_>, arm: &Arm<'_>) -> bool {
    matches!(arm.pat.kind, PatKind::Path(ref qpath) if is_lang_ctor(cx, qpath, LangItem::OptionNone))
}

// Checks if arm has the form `Some(ref v) => Some(v)` (checks for `ref` and `ref mut`)
fn is_ref_some_arm(cx: &LateContext<'_>, arm: &Arm<'_>) -> Option<BindingAnnotation> {
    if let PatKind::TupleStruct(ref qpath, [first_pat, ..], _) = arm.pat.kind
        && is_lang_ctor(cx, qpath, LangItem::OptionSome)
        && let PatKind::Binding(rb, .., ident, _) = first_pat.kind
        && (rb == BindingAnnotation::Ref || rb == BindingAnnotation::RefMut)
        && let ExprKind::Call(e, args) = peel_blocks(arm.body).kind
        && let ExprKind::Path(ref some_path) = e.kind
        && is_lang_ctor(cx, some_path, LangItem::OptionSome) && args.len() == 1
        && let ExprKind::Path(QPath::Resolved(_, path2)) = args[0].kind
        && path2.segments.len() == 1 && ident.name == path2.segments[0].ident.name
    {
        return Some(rb)
    }
    None
}
