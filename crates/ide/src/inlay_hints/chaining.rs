//! Implementation of "chaining" inlay hints.
use hir::Semantics;
use ide_db::RootDatabase;
use syntax::{
    ast::{self, AstNode},
    Direction, NodeOrToken, SyntaxKind, T,
};

use crate::{FileId, InlayHint, InlayHintsConfig, InlayKind, InlayTooltip};

use super::label_of_ty;

pub(super) fn hints(
    acc: &mut Vec<InlayHint>,
    sema: &Semantics<'_, RootDatabase>,
    config: &InlayHintsConfig,
    file_id: FileId,
    expr: &ast::Expr,
) -> Option<()> {
    if !config.chaining_hints {
        return None;
    }

    if matches!(expr, ast::Expr::RecordExpr(_)) {
        return None;
    }

    let descended = sema.descend_node_into_attributes(expr.clone()).pop();
    let desc_expr = descended.as_ref().unwrap_or(expr);

    let mut tokens = expr
        .syntax()
        .siblings_with_tokens(Direction::Next)
        .filter_map(NodeOrToken::into_token)
        .filter(|t| match t.kind() {
            SyntaxKind::WHITESPACE if !t.text().contains('\n') => false,
            SyntaxKind::COMMENT => false,
            _ => true,
        });

    // Chaining can be defined as an expression whose next sibling tokens are newline and dot
    // Ignoring extra whitespace and comments
    let next = tokens.next()?.kind();
    if next == SyntaxKind::WHITESPACE {
        let mut next_next = tokens.next()?.kind();
        while next_next == SyntaxKind::WHITESPACE {
            next_next = tokens.next()?.kind();
        }
        if next_next == T![.] {
            let ty = sema.type_of_expr(desc_expr)?.original;
            if ty.is_unknown() {
                return None;
            }
            if matches!(expr, ast::Expr::PathExpr(_)) {
                if let Some(hir::Adt::Struct(st)) = ty.as_adt() {
                    if st.fields(sema.db).is_empty() {
                        return None;
                    }
                }
            }
            acc.push(InlayHint {
                range: expr.syntax().text_range(),
                kind: InlayKind::ChainingHint,
                label: label_of_ty(sema, desc_expr, config, ty)?,
                tooltip: Some(InlayTooltip::HoverRanged(file_id, expr.syntax().text_range())),
            });
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use crate::{
        inlay_hints::tests::{
            check_expect, check_with_config, DISABLED_CONFIG, DISABLED_CONFIG_WITH_LINKS,
            TEST_CONFIG,
        },
        InlayHintsConfig,
    };

    #[track_caller]
    fn check_chains(ra_fixture: &str) {
        check_with_config(InlayHintsConfig { chaining_hints: true, ..DISABLED_CONFIG }, ra_fixture);
    }

    #[test]
    fn chaining_hints_ignore_comments() {
        check_expect(
            InlayHintsConfig {
                type_hints: false,
                chaining_hints: true,
                ..DISABLED_CONFIG_WITH_LINKS
            },
            r#"
struct A(B);
impl A { fn into_b(self) -> B { self.0 } }
struct B(C);
impl B { fn into_c(self) -> C { self.0 } }
struct C;

fn main() {
    let c = A(B(C))
        .into_b() // This is a comment
        // This is another comment
        .into_c();
}
"#,
            expect![[r#"
                [
                    InlayHint {
                        range: 147..172,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "B",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 63..64,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                147..172,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 147..154,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "A",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 7..8,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                147..154,
                            ),
                        ),
                    },
                ]
            "#]],
        );
    }

    #[test]
    fn chaining_hints_without_newlines() {
        check_chains(
            r#"
struct A(B);
impl A { fn into_b(self) -> B { self.0 } }
struct B(C);
impl B { fn into_c(self) -> C { self.0 } }
struct C;

fn main() {
    let c = A(B(C)).into_b().into_c();
}"#,
        );
    }

    #[test]
    fn disabled_location_links() {
        check_expect(
            InlayHintsConfig { chaining_hints: true, ..DISABLED_CONFIG },
            r#"
    struct A { pub b: B }
    struct B { pub c: C }
    struct C(pub bool);
    struct D;

    impl D {
        fn foo(&self) -> i32 { 42 }
    }

    fn main() {
        let x = A { b: B { c: C(true) } }
            .b
            .c
            .0;
        let x = D
            .foo();
    }"#,
            expect![[r#"
                [
                    InlayHint {
                        range: 143..190,
                        kind: ChainingHint,
                        label: [
                            "C",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                143..190,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 143..179,
                        kind: ChainingHint,
                        label: [
                            "B",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                143..179,
                            ),
                        ),
                    },
                ]
            "#]],
        );
    }

    #[test]
    fn struct_access_chaining_hints() {
        check_expect(
            InlayHintsConfig { chaining_hints: true, ..DISABLED_CONFIG_WITH_LINKS },
            r#"
struct A { pub b: B }
struct B { pub c: C }
struct C(pub bool);
struct D;

impl D {
    fn foo(&self) -> i32 { 42 }
}

fn main() {
    let x = A { b: B { c: C(true) } }
        .b
        .c
        .0;
    let x = D
        .foo();
}"#,
            expect![[r#"
                [
                    InlayHint {
                        range: 143..190,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "C",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 51..52,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                143..190,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 143..179,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "B",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 29..30,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                143..179,
                            ),
                        ),
                    },
                ]
            "#]],
        );
    }

    #[test]
    fn generic_chaining_hints() {
        check_expect(
            InlayHintsConfig { chaining_hints: true, ..DISABLED_CONFIG_WITH_LINKS },
            r#"
struct A<T>(T);
struct B<T>(T);
struct C<T>(T);
struct X<T,R>(T, R);

impl<T> A<T> {
    fn new(t: T) -> Self { A(t) }
    fn into_b(self) -> B<T> { B(self.0) }
}
impl<T> B<T> {
    fn into_c(self) -> C<T> { C(self.0) }
}
fn main() {
    let c = A::new(X(42, true))
        .into_b()
        .into_c();
}
"#,
            expect![[r#"
                [
                    InlayHint {
                        range: 246..283,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "B",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 23..24,
                                    },
                                ),
                            },
                            "<",
                            InlayHintLabelPart {
                                text: "X",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 55..56,
                                    },
                                ),
                            },
                            "<i32, bool>>",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                246..283,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 246..265,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "A",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 7..8,
                                    },
                                ),
                            },
                            "<",
                            InlayHintLabelPart {
                                text: "X",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 55..56,
                                    },
                                ),
                            },
                            "<i32, bool>>",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                246..265,
                            ),
                        ),
                    },
                ]
            "#]],
        );
    }

    #[test]
    fn shorten_iterator_chaining_hints() {
        check_expect(
            InlayHintsConfig { chaining_hints: true, ..DISABLED_CONFIG_WITH_LINKS },
            r#"
//- minicore: iterators
use core::iter;

struct MyIter;

impl Iterator for MyIter {
    type Item = ();
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

fn main() {
    let _x = MyIter.by_ref()
        .take(5)
        .by_ref()
        .take(5)
        .by_ref();
}
"#,
            expect![[r#"
                [
                    InlayHint {
                        range: 174..241,
                        kind: ChainingHint,
                        label: [
                            "impl Iterator<Item = ()>",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                174..241,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 174..224,
                        kind: ChainingHint,
                        label: [
                            "impl Iterator<Item = ()>",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                174..224,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 174..206,
                        kind: ChainingHint,
                        label: [
                            "impl Iterator<Item = ()>",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                174..206,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 174..189,
                        kind: ChainingHint,
                        label: [
                            "&mut ",
                            InlayHintLabelPart {
                                text: "MyIter",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 24..30,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                174..189,
                            ),
                        ),
                    },
                ]
            "#]],
        );
    }

    #[test]
    fn hints_in_attr_call() {
        check_expect(
            TEST_CONFIG,
            r#"
//- proc_macros: identity, input_replace
struct Struct;
impl Struct {
    fn chain(self) -> Self {
        self
    }
}
#[proc_macros::identity]
fn main() {
    let strukt = Struct;
    strukt
        .chain()
        .chain()
        .chain();
    Struct::chain(strukt);
}
"#,
            expect![[r#"
                [
                    InlayHint {
                        range: 124..130,
                        kind: TypeHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "Struct",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 7..13,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                124..130,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 145..185,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "Struct",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 7..13,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                145..185,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 145..168,
                        kind: ChainingHint,
                        label: [
                            "",
                            InlayHintLabelPart {
                                text: "Struct",
                                linked_location: Some(
                                    FileRange {
                                        file_id: FileId(
                                            0,
                                        ),
                                        range: 7..13,
                                    },
                                ),
                            },
                            "",
                        ],
                        tooltip: Some(
                            HoverRanged(
                                FileId(
                                    0,
                                ),
                                145..168,
                            ),
                        ),
                    },
                    InlayHint {
                        range: 222..228,
                        kind: ParameterHint,
                        label: [
                            "self",
                        ],
                        tooltip: Some(
                            HoverOffset(
                                FileId(
                                    0,
                                ),
                                42,
                            ),
                        ),
                    },
                ]
            "#]],
        );
    }
}
