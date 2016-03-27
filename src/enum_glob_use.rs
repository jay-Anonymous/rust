//! lint on `use`ing all variants of an enum

use rustc::front::map::Node::NodeItem;
use rustc::front::map::definitions::DefPathData;
use rustc::lint::{LateLintPass, LintPass, LateContext, LintArray, LintContext};
use rustc::ty::TyEnum;
use rustc_front::hir::*;
use syntax::ast::NodeId;
use syntax::codemap::Span;
use utils::span_lint;

/// **What it does:** Warns when `use`ing all variants of an enum
///
/// **Why is this bad?** It is usually better style to use the prefixed name of an enum variant, rather than importing variants
///
/// **Known problems:** Old-style enums that prefix the variants are still around
///
/// **Example:** `use std::cmp::Ordering::*;`
declare_lint! { pub ENUM_GLOB_USE, Allow,
    "finds use items that import all variants of an enum" }

pub struct EnumGlobUse;

impl LintPass for EnumGlobUse {
    fn get_lints(&self) -> LintArray {
        lint_array!(ENUM_GLOB_USE)
    }
}

impl LateLintPass for EnumGlobUse {
    fn check_mod(&mut self, cx: &LateContext, m: &Mod, _: Span, _: NodeId) {
        // only check top level `use` statements
        for item in &m.item_ids {
            self.lint_item(cx, cx.krate.item(item.id));
        }
    }
}

impl EnumGlobUse {
    fn lint_item(&self, cx: &LateContext, item: &Item) {
        if item.vis == Visibility::Public {
            return; // re-exports are fine
        }
        if let ItemUse(ref item_use) = item.node {
            if let ViewPath_::ViewPathGlob(_) = item_use.node {
                if let Some(def) = cx.tcx.def_map.borrow().get(&item.id) {
                    if let Some(node_id) = cx.tcx.map.as_local_node_id(def.def_id()) {
                        if let Some(NodeItem(it)) = cx.tcx.map.find(node_id) {
                            if let ItemEnum(..) = it.node {
                                span_lint(cx, ENUM_GLOB_USE, item.span, "don't use glob imports for enum variants");
                            }
                        }
                    } else {
                        let dp = cx.sess().cstore.relative_def_path(def.def_id());
                        if  let Some(dpa) = dp.data.last() {
                            if let  DefPathData::TypeNs(_) = dpa.data {
                                if let TyEnum(..) = cx.sess().cstore.item_type(&cx.tcx, def.def_id()).ty.sty {
                                    span_lint(cx, ENUM_GLOB_USE, item.span, "don't use glob imports for enum variants");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
