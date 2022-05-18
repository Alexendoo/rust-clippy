use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::sugg::Sugg;
use clippy_utils::{meets_msrv, msrvs};
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty::Ty;
use rustc_semver::RustcVersion;

use super::CAST_ABS_TO_UNSIGNED;

pub(super) fn check(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    cast_expr: &Expr<'_>,
    cast_from: Ty<'_>,
    cast_to: Ty<'_>,
    msrv: Option<RustcVersion>,
) {
    if meets_msrv(msrv, msrvs::UNSIGNED_ABS)
        && cast_from.is_integral()
        && cast_to.is_integral()
        && cast_from.is_signed()
        && !cast_to.is_signed()
        && let ExprKind::MethodCall(method_path, args, _) = cast_expr.kind
        && let method_name = method_path.ident.name.as_str()
        && method_name == "abs"
    {
        span_lint_and_sugg(
            cx,
            CAST_ABS_TO_UNSIGNED,
            expr.span,
            &format!("casting the result of `{}::{}()` to {}", cast_from, method_name, cast_to),
            "replace with",
            format!("{}.unsigned_abs()", Sugg::hir(cx, &args[0], "..")),
            Applicability::MachineApplicable,
        );
    }
}
