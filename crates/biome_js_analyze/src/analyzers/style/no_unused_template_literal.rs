use crate::JsRuleAction;
use biome_analyze::context::RuleContext;
use biome_analyze::{declare_rule, ActionCategory, Ast, Rule, RuleDiagnostic};
use biome_console::markup;
use biome_diagnostics::Applicability;
use biome_js_factory::make;
use biome_js_syntax::{
    AnyJsExpression, AnyJsLiteralExpression, AnyJsTemplateElement, JsTemplateExpression,
};
use biome_rowan::{AstNode, AstNodeList, BatchMutationExt};

declare_rule! {
    /// Disallow template literals if interpolation and special-character handling are not needed
    ///
    /// ## Examples
    ///
    /// ### Invalid
    ///
    /// ```js,expect_diagnostic
    /// const foo = `bar`
    /// ```
    ///
    /// ```js,expect_diagnostic
    /// const foo = `bar `
    /// ```
    ///
    /// ### Valid
    ///
    /// ```js
    /// const foo = `bar
    /// has newline`;
    /// ```
    ///
    /// ```js
    /// const foo = `"bar"`
    /// ```
    ///
    /// ```js
    /// const foo = `'bar'`
    /// ```
    pub(crate) NoUnusedTemplateLiteral {
        version: "1.0.0",
        name: "noUnusedTemplateLiteral",
        recommended: true,
    }
}

impl Rule for NoUnusedTemplateLiteral {
    type Query = Ast<JsTemplateExpression>;
    type State = ();
    type Signals = Option<Self::State>;
    type Options = ();

    fn run(ctx: &RuleContext<Self>) -> Option<Self::State> {
        let node = ctx.query();

        if node.tag().is_none() && can_convert_to_string_literal(node) {
            Some(())
        } else {
            None
        }
    }

    fn diagnostic(ctx: &RuleContext<Self>, _: &Self::State) -> Option<RuleDiagnostic> {
        let node = ctx.query();

        Some(RuleDiagnostic::new(rule_category!(),node.range(), markup! {
            "Do not use template literals if interpolation and special-character handling are not needed."
        }
        .to_owned() ) )
    }

    fn action(ctx: &RuleContext<Self>, _: &Self::State) -> Option<JsRuleAction> {
        let node = ctx.query();
        let mut mutation = ctx.root().begin();

        // join all template content
        let inner_content = node
            .elements()
            .iter()
            .fold(String::from(""), |mut acc, cur| {
                match cur {
                    AnyJsTemplateElement::JsTemplateChunkElement(ele) => {
                        // Safety: if `ele.template_chunk_token()` is `Err` variant, [can_convert_to_string_lit] should return false,
                        // thus `run` will return None
                        acc += ele.template_chunk_token().unwrap().text();
                        acc
                    }
                    AnyJsTemplateElement::JsTemplateElement(_) => {
                        // Because we know if TemplateLit has any `JsTemplateElement` will return `None` in `run` function
                        unreachable!()
                    }
                }
            });

        mutation.replace_node(
            AnyJsExpression::JsTemplateExpression(node.clone()),
            AnyJsExpression::AnyJsLiteralExpression(
                AnyJsLiteralExpression::JsStringLiteralExpression(
                    make::js_string_literal_expression(make::js_string_literal(&inner_content)),
                ),
            ),
        );

        Some(JsRuleAction {
            category: ActionCategory::QuickFix,
            applicability: Applicability::MaybeIncorrect,
            message: markup! { "Replace with string literal" }.to_owned(),
            mutation,
        })
    }
}

fn can_convert_to_string_literal(node: &JsTemplateExpression) -> bool {
    !node.elements().iter().any(|element| {
        // We want to test if any templateElement has violated rule that can convert to string literal, rules are listed below
        // 1. Variant of element is `JsTemplateElement`
        // 2. Content of `ChunkElement` has any special characters, any of `\n`, `'`, `"`
        match element {
            AnyJsTemplateElement::JsTemplateElement(_) => true,
            AnyJsTemplateElement::JsTemplateChunkElement(chunk) => {
                match chunk.template_chunk_token() {
                    Ok(token) => {
                        // if token text has any special character
                        token
                            .text()
                            .chars()
                            .any(|ch| matches!(ch, '\n' | '\'' | '"'))
                    }
                    Err(_) => {
                        // if we found an error, then just return `true`, which means that this template literal can't be converted to
                        // a string literal
                        true
                    }
                }
            }
        }
    })
}
