use clippy_utils::{diagnostics::span_lint_and_sugg, ty::is_type_diagnostic_item};
use rustc_ast::ast::LitKind;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{Ref, Slice};
use rustc_span::{sym, Span};

use super::UNNECESSARY_JOIN;

pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    join_self_arg: &'tcx Expr<'tcx>,
    join_arg: &'tcx Expr<'tcx>,
    span: Span,
) {
    let applicability = Applicability::MachineApplicable;
    let collect_output_adjusted_type = cx.typeck_results().expr_ty_adjusted(join_self_arg);
    // the turbofish for collect is ::<Vec<String>>
    if let Ref(_, ref_type, _) = collect_output_adjusted_type.kind()
        && let Slice(slice) = ref_type.kind()
        && is_type_diagnostic_item(cx, *slice, sym::String)
        // the argument for join is ""
        && let ExprKind::Lit(spanned) = &join_arg.kind
        && let LitKind::Str(symbol, _) = spanned.node
        && symbol.is_empty()
    {
        span_lint_and_sugg(
            cx,
            UNNECESSARY_JOIN,
            span.with_hi(expr.span.hi()),
            r#"called `.collect<Vec<String>>().join("")` on an iterator"#,
            "try using",
            "collect::<String>()".to_owned(),
            applicability,
        );
    }
}
