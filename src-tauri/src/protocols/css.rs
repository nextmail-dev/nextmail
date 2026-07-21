use std::{cell::Cell, collections::HashSet, rc::Rc, sync::LazyLock};

use cssparser::{
    AtRuleParser, BasicParseErrorKind, CowRcStr, DeclarationParser, ParseError, Parser,
    ParserInput, ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser,
    StyleSheetParser, ToCss, Token, TokenSerializationType,
};

const MAX_STYLESHEET_BYTES: usize = 256 * 1024;
const MAX_STYLESHEET_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_STYLE_ATTRIBUTE_BYTES: usize = 16 * 1024;
const MAX_SELECTOR_BYTES: usize = 2 * 1024;
const MAX_DECLARATION_VALUE_BYTES: usize = 4 * 1024;
const MAX_MEDIA_QUERY_BYTES: usize = 1024;
const MAX_RULES: usize = 2_048;
const MAX_VALUE_NESTING: usize = 8;

static SAFE_STYLE_PROPERTIES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "align-content",
        "align-items",
        "align-self",
        "background",
        "background-clip",
        "background-color",
        "background-image",
        "background-origin",
        "background-position",
        "background-repeat",
        "background-size",
        "border",
        "border-bottom",
        "border-bottom-color",
        "border-bottom-left-radius",
        "border-bottom-right-radius",
        "border-bottom-style",
        "border-bottom-width",
        "border-collapse",
        "border-color",
        "border-left",
        "border-left-color",
        "border-left-style",
        "border-left-width",
        "border-radius",
        "border-right",
        "border-right-color",
        "border-right-style",
        "border-right-width",
        "border-spacing",
        "border-style",
        "border-top",
        "border-top-color",
        "border-top-left-radius",
        "border-top-right-radius",
        "border-top-style",
        "border-top-width",
        "border-width",
        "box-shadow",
        "box-sizing",
        "clear",
        "color",
        "color-scheme",
        "direction",
        "display",
        "flex",
        "flex-basis",
        "flex-direction",
        "flex-grow",
        "flex-shrink",
        "flex-wrap",
        "float",
        "font",
        "font-family",
        "font-size",
        "font-stretch",
        "font-style",
        "font-variant",
        "font-weight",
        "gap",
        "height",
        "justify-content",
        "letter-spacing",
        "line-height",
        "list-style-position",
        "list-style-type",
        "margin",
        "margin-bottom",
        "margin-left",
        "margin-right",
        "margin-top",
        "max-height",
        "max-width",
        "min-height",
        "min-width",
        "object-fit",
        "object-position",
        "opacity",
        "overflow",
        "overflow-wrap",
        "overflow-x",
        "overflow-y",
        "padding",
        "padding-bottom",
        "padding-left",
        "padding-right",
        "padding-top",
        "table-layout",
        "text-align",
        "text-decoration",
        "text-decoration-color",
        "text-decoration-line",
        "text-decoration-style",
        "text-decoration-thickness",
        "text-indent",
        "text-shadow",
        "text-transform",
        "unicode-bidi",
        "vertical-align",
        "visibility",
        "white-space",
        "width",
        "word-break",
        "word-spacing",
    ]
    .into_iter()
    .collect()
});

pub(super) fn sanitize_style_attribute(source: &str) -> String {
    if source.len() > MAX_STYLE_ATTRIBUTE_BYTES {
        return String::new();
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    sanitize_declaration_block(&mut parser)
}

pub(super) fn sanitize_stylesheet(source: &str) -> String {
    if source.len() > MAX_STYLESHEET_BYTES {
        return String::new();
    }

    let remaining_rules = Rc::new(Cell::new(MAX_RULES));
    let mut input = ParserInput::new(source);
    let mut input = Parser::new(&mut input);
    let mut parser = MailStyleSheetParser {
        depth: 0,
        remaining_rules,
    };
    let output = sanitize_rule_list(&mut input, &mut parser);
    escape_style_raw_text(&output)
}

fn sanitize_rule_list<'i>(input: &mut Parser<'i, '_>, parser: &mut MailStyleSheetParser) -> String {
    let mut output = String::new();
    for rule in StyleSheetParser::new(input, parser).flatten() {
        if rule.is_empty() || output.len() + rule.len() > MAX_STYLESHEET_OUTPUT_BYTES {
            continue;
        }
        output.push_str(&rule);
    }
    output
}

struct MailStyleSheetParser {
    depth: usize,
    remaining_rules: Rc<Cell<usize>>,
}

impl MailStyleSheetParser {
    fn claim_rule(&self) -> bool {
        let remaining = self.remaining_rules.get();
        if remaining == 0 {
            return false;
        }
        self.remaining_rules.set(remaining - 1);
        true
    }
}

impl<'i> QualifiedRuleParser<'i> for MailStyleSheetParser {
    type Prelude = String;
    type QualifiedRule = String;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        sanitize_selector(input)
            .ok_or_else(|| input.new_error(BasicParseErrorKind::QualifiedRuleInvalid))
    }

    fn parse_block<'t>(
        &mut self,
        selector: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let declarations = sanitize_declaration_block(input);
        if declarations.is_empty() || !self.claim_rule() {
            return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
        }
        Ok(format!("{selector}{{{declarations}}}"))
    }
}

impl<'i> AtRuleParser<'i> for MailStyleSheetParser {
    type Prelude = String;
    type AtRule = String;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        if self.depth > 0 || !name.eq_ignore_ascii_case("media") {
            return Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)));
        }
        sanitize_media_query(input)
    }

    fn parse_block<'t>(
        &mut self,
        query: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        if !self.claim_rule() {
            return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid));
        }
        let mut nested_parser = MailStyleSheetParser {
            depth: self.depth + 1,
            remaining_rules: Rc::clone(&self.remaining_rules),
        };
        let rules = sanitize_rule_list(input, &mut nested_parser);
        if rules.is_empty() {
            return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid));
        }
        Ok(format!("@media {query}{{{rules}}}"))
    }
}

fn sanitize_selector(input: &mut Parser<'_, '_>) -> Option<String> {
    let mut writer = CssTokenWriter::default();
    let mut token_count = 0usize;
    while let Ok(token) = input.next_including_whitespace().cloned() {
        token_count += 1;
        if token_count > 128 {
            return None;
        }
        match token {
            Token::Ident(_)
            | Token::Hash(_)
            | Token::IDHash(_)
            | Token::WhiteSpace(_)
            | Token::Colon
            | Token::Comma
            | Token::Delim('.')
            | Token::Delim('*')
            | Token::Delim('>')
            | Token::Delim('+')
            | Token::Delim('~') => writer.write_token(&token).ok()?,
            Token::SquareBracketBlock => {
                writer.write_token(&token).ok()?;
                input
                    .parse_nested_block(|nested| sanitize_attribute_selector(nested, &mut writer))
                    .ok()?;
                writer.push_closing(']');
            }
            _ => return None,
        }
        if writer.output.len() > MAX_SELECTOR_BYTES {
            return None;
        }
    }
    let output = writer.finish();
    (!output.is_empty()).then_some(output)
}

fn sanitize_attribute_selector<'i>(
    input: &mut Parser<'i, '_>,
    writer: &mut CssTokenWriter,
) -> Result<(), ParseError<'i, ()>> {
    while let Ok(token) = input.next_including_whitespace().cloned() {
        match token {
            Token::Ident(_)
            | Token::QuotedString(_)
            | Token::Hash(_)
            | Token::IDHash(_)
            | Token::Number { .. }
            | Token::WhiteSpace(_)
            | Token::IncludeMatch
            | Token::DashMatch
            | Token::PrefixMatch
            | Token::SuffixMatch
            | Token::SubstringMatch
            | Token::Delim('=') => writer.write_token(&token).map_err(|()| {
                input.new_error(BasicParseErrorKind::UnexpectedToken(token.clone()))
            })?,
            _ => {
                return Err(input.new_error(BasicParseErrorKind::UnexpectedToken(token)));
            }
        }
    }
    Ok(())
}

fn sanitize_media_query<'i>(input: &mut Parser<'i, '_>) -> Result<String, ParseError<'i, ()>> {
    let mut parts = Vec::new();
    while let Ok(token) = input.next_including_whitespace().cloned() {
        match token {
            Token::WhiteSpace(_) => {}
            Token::Ident(name)
                if matches_ignore_ascii_case(&name, &["all", "screen", "and", "not", "only"]) =>
            {
                parts.push(name.to_ascii_lowercase())
            }
            Token::Comma => parts.push(",".to_owned()),
            Token::ParenthesisBlock => {
                let feature = input.parse_nested_block(parse_media_feature)?;
                parts.push(format!("({feature})"));
            }
            _ => return Err(input.new_error(BasicParseErrorKind::UnexpectedToken(token))),
        }
    }
    if parts.is_empty() {
        return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid));
    }

    let mut output = String::new();
    for part in parts {
        if part == "," {
            if output.ends_with(' ') {
                output.pop();
            }
            output.push_str(", ");
        } else {
            if !output.is_empty() && !output.ends_with(' ') {
                output.push(' ');
            }
            output.push_str(&part);
        }
    }
    let output = output.trim().to_owned();
    if output.len() > MAX_MEDIA_QUERY_BYTES {
        Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid))
    } else {
        Ok(output)
    }
}

fn parse_media_feature<'i>(input: &mut Parser<'i, '_>) -> Result<String, ParseError<'i, ()>> {
    let feature = input.expect_ident_cloned()?.to_ascii_lowercase();
    input.expect_colon()?;
    let value = match feature.as_str() {
        "prefers-color-scheme" => {
            let value = input.expect_ident_cloned()?.to_ascii_lowercase();
            if !matches!(value.as_str(), "light" | "dark") {
                return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid));
            }
            value
        }
        "orientation" => {
            let value = input.expect_ident_cloned()?.to_ascii_lowercase();
            if !matches!(value.as_str(), "portrait" | "landscape") {
                return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid));
            }
            value
        }
        "min-width" | "max-width" => {
            let token = input.next()?.clone();
            match &token {
                Token::Dimension { value, unit, .. }
                    if value.is_finite()
                        && (0.0..=10_000.0).contains(value)
                        && matches_ignore_ascii_case(unit, &["px", "em", "rem"]) =>
                {
                    token.to_css_string()
                }
                Token::Number { value: 0.0, .. } => "0".to_owned(),
                _ => {
                    return Err(input.new_error(BasicParseErrorKind::UnexpectedToken(token)));
                }
            }
        }
        _ => return Err(input.new_error(BasicParseErrorKind::AtRuleBodyInvalid)),
    };
    input.expect_exhausted()?;
    Ok(format!("{feature}:{value}"))
}

struct MailDeclarationParser;

impl<'i> DeclarationParser<'i> for MailDeclarationParser {
    type Declaration = String;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let name = name.to_ascii_lowercase();
        if !SAFE_STYLE_PROPERTIES.contains(name.as_str()) {
            return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
        }
        let mut writer = CssTokenWriter::default();
        serialize_safe_value(input, 0, &mut writer)?;
        let value = writer.finish();
        if value.is_empty() || value.len() > MAX_DECLARATION_VALUE_BYTES {
            return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
        }
        Ok(format!("{name}:{value}"))
    }
}

impl AtRuleParser<'_> for MailDeclarationParser {
    type Prelude = ();
    type AtRule = String;
    type Error = ();
}

impl QualifiedRuleParser<'_> for MailDeclarationParser {
    type Prelude = ();
    type QualifiedRule = String;
    type Error = ();
}

impl RuleBodyItemParser<'_, String, ()> for MailDeclarationParser {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn sanitize_declaration_block(input: &mut Parser<'_, '_>) -> String {
    let mut parser = MailDeclarationParser;
    RuleBodyParser::new(input, &mut parser)
        .filter_map(Result::ok)
        .collect::<Vec<_>>()
        .join(";")
}

fn serialize_safe_value<'i>(
    input: &mut Parser<'i, '_>,
    depth: usize,
    writer: &mut CssTokenWriter,
) -> Result<(), ParseError<'i, ()>> {
    if depth > MAX_VALUE_NESTING {
        return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
    }

    while let Ok(token) = input.next_including_whitespace().cloned() {
        match token {
            Token::UnquotedUrl(_)
            | Token::BadUrl(_)
            | Token::BadString(_)
            | Token::AtKeyword(_)
            | Token::CurlyBracketBlock
            | Token::SquareBracketBlock
            | Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket
            | Token::CDO
            | Token::CDC
            | Token::Delim('\\')
            | Token::Delim('<') => {
                return Err(input.new_error(BasicParseErrorKind::UnexpectedToken(token)));
            }
            Token::Function(ref name) => {
                if !is_safe_value_function(name) {
                    return Err(input.new_error(BasicParseErrorKind::UnexpectedToken(token)));
                }
                writer.write_token(&token).map_err(|()| {
                    input.new_error(BasicParseErrorKind::UnexpectedToken(token.clone()))
                })?;
                input
                    .parse_nested_block(|nested| serialize_safe_value(nested, depth + 1, writer))?;
                writer.push_closing(')');
            }
            Token::ParenthesisBlock => {
                writer.write_token(&token).map_err(|()| {
                    input.new_error(BasicParseErrorKind::UnexpectedToken(token.clone()))
                })?;
                input
                    .parse_nested_block(|nested| serialize_safe_value(nested, depth + 1, writer))?;
                writer.push_closing(')');
            }
            _ => writer.write_token(&token).map_err(|()| {
                input.new_error(BasicParseErrorKind::UnexpectedToken(token.clone()))
            })?,
        }
        if writer.output.len() > MAX_DECLARATION_VALUE_BYTES {
            return Err(input.new_error(BasicParseErrorKind::QualifiedRuleInvalid));
        }
    }
    Ok(())
}

fn is_safe_value_function(name: &str) -> bool {
    matches_ignore_ascii_case(
        name,
        &[
            "calc",
            "clamp",
            "color",
            "color-mix",
            "hsl",
            "hsla",
            "lab",
            "lch",
            "linear-gradient",
            "max",
            "min",
            "oklab",
            "oklch",
            "radial-gradient",
            "repeating-linear-gradient",
            "repeating-radial-gradient",
            "rgb",
            "rgba",
        ],
    )
}

fn matches_ignore_ascii_case(value: &str, allowed: &[&str]) -> bool {
    allowed
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

#[derive(Default)]
struct CssTokenWriter {
    output: String,
    previous: TokenSerializationType,
}

impl CssTokenWriter {
    fn write_token(&mut self, token: &Token<'_>) -> Result<(), ()> {
        if matches!(token, Token::WhiteSpace(_)) {
            if !self.output.is_empty() && !self.output.ends_with(' ') {
                self.output.push(' ');
            }
            self.previous = TokenSerializationType::WhiteSpace;
            return Ok(());
        }

        let current = token.serialization_type();
        if self.previous.needs_separator_when_before(current) {
            self.output.push_str("/**/");
        }
        token.to_css(&mut self.output).map_err(|_| ())?;
        self.previous = current;
        Ok(())
    }

    fn push_closing(&mut self, value: char) {
        if self.output.ends_with(' ') {
            self.output.pop();
        }
        self.output.push(value);
        self.previous = TokenSerializationType::Other;
    }

    fn finish(self) -> String {
        self.output.trim().to_owned()
    }
}

fn escape_style_raw_text(value: &str) -> String {
    value.replace('<', "\\3c ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_safe_rules_responsive_media_and_dark_mode() {
        let sanitized = sanitize_stylesheet(
            r#"
              .card, body > table { color: #202124; background: linear-gradient(#fff, #f4f4f4); position: fixed; }
              @media (max-width: 640px) { .card { width: 100%; padding: 12px; } }
              @media (prefers-color-scheme: dark) { .card { color: rgb(238, 238, 238); background-color: #252525; } }
            "#,
        );

        assert!(sanitized.contains(".card"));
        assert!(sanitized.contains("background:linear-gradient("));
        assert!(sanitized.contains("@media (max-width:640px)"));
        assert!(sanitized.contains("@media (prefers-color-scheme:dark)"));
        assert!(sanitized.contains("padding:12px"));
        assert!(!sanitized.contains("position"));
    }

    #[test]
    fn rejects_network_active_and_overlay_css_without_losing_safe_declarations() {
        let sanitized = sanitize_stylesheet(
            r#"
              @import url("https://tracker.example/import.css");
              @font-face { font-family: Spy; src: url("https://tracker.example/font.woff2"); }
              .message { color: red; background-image: url("https://tracker.example/pixel"); z-index: 9999; }
              .other { width: expression(alert(1)); padding: 8px; }
            "#,
        );

        assert!(sanitized.contains(".message{color:red}"));
        assert!(sanitized.contains(".other{padding:8px}"));
        for forbidden in [
            "@import",
            "@font-face",
            "url(",
            "tracker.example",
            "z-index",
            "expression",
        ] {
            assert!(!sanitized.contains(forbidden), "retained {forbidden}");
        }
    }

    #[test]
    fn style_attributes_use_the_same_property_and_resource_boundary() {
        let sanitized = sanitize_style_attribute(
            "color:#123;background-image:url(https://tracker.example/pixel);position:fixed;padding:12px",
        );
        assert_eq!(sanitized, "color:#123;padding:12px");
    }

    #[test]
    fn decoded_css_cannot_break_out_of_a_style_raw_text_element() {
        let sanitized = sanitize_stylesheet(
            r#".message { font-family: "\3c /style\3e \3c script\3e alert(1)"; color: red; }"#,
        );
        assert!(sanitized.contains("color:red"));
        assert!(!sanitized.to_ascii_lowercase().contains("</style"));
        assert!(sanitized.contains("\\3c "));
    }

    #[test]
    fn rejects_oversized_stylesheets_and_style_attributes() {
        assert!(sanitize_stylesheet(&"a".repeat(MAX_STYLESHEET_BYTES + 1)).is_empty());
        assert!(sanitize_style_attribute(&"a".repeat(MAX_STYLE_ATTRIBUTE_BYTES + 1)).is_empty());
    }
}
