use eyre::{OptionExt, WrapErr};

macro_rules! bail_at {
    ($node:expr, $fmt_str:literal) => {
        bail_at!($node, $fmt_str, )
    };
    ($node:expr, $fmt_str:literal, $($fmt:tt),*) => {
        bail_at!(@$node.data.borrow(), $fmt_str, )
    };
    (@$node:expr, $fmt_str:literal) => {
        bail_at!(@$node, $fmt_str, )
    };
    (@$node:expr, $fmt_str:literal, $($fmt:tt),*) => {
        return Err(eyre::eyre!($fmt_str, $($fmt)*)).wrap_err_with(
            || eyre::eyre!("unexpected content on line {}", $node.sourcepos.start.line),
        )
    }
}

macro_rules! ensure_at {
    ($node:expr, $e:expr, $fmt_str:literal) => {
        ensure_at!($node, $e, $fmt_str, )
    };
    ($node:expr, $e:expr, $fmt_str:literal, $($fmt:tt),*) => {
        if !($e) {
            bail_at!($node, $fmt_str);
        }
    };
    (@$node:expr, $e:expr, $fmt_str:literal) => {
        ensure_at!(@$node, $e, $fmt_str, )
    };
    (@$node:expr, $e:expr, $fmt_str:literal, $($fmt:tt),*) => {
        if !($e) {
            bail_at!(@$node, $fmt_str);
        }
    }
}

macro_rules! require_node {
    ($iter:ident, $expected:expr, $fmt_str:literal) => {
        require_node!($iter, $expected, $fmt_str, )
    };
    ($iter:ident, $expected:expr, $fmt_str:literal, $($fmt:tt),*) => {
        {
            let node = $iter.next().ok_or_eyre("unexpected EOF")?;
            ensure_at!(node, node.data.borrow().value == $expected, $fmt_str, $($fmt,)*);
            node
        }
    };
}

const FIELD_REQUIRED: &str = "Required.";
const FIELD_NUMBER: &str = "Must be a number.";
const FIELD_REGEX: &str = "Must match the following regex: ";
const FIELD_AT_LEAST_ONE: &str = "Must select at least one option.";
const FIELD_AT_MOST_ONE: &str = "Must select at most one option.";
const FIELD_EXACTLY_ONE: &str = "Must select exactly one option.";
const FIELD_CHECKBOX_REQUIRED: &str = "(required) ";

#[derive(serde::Deserialize, Debug)]
pub struct YamlTemplate {
    #[serde(flatten)]
    pub metadata: super::TemplateMetadata,
    pub body: Vec<TemplateItem>,
}

static MD_OPTIONS: std::sync::LazyLock<comrak::Options<'static>> = std::sync::LazyLock::new(|| {
    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.render.r#unsafe = true;
    options
});

impl YamlTemplate {
    pub fn generate_form(&self) -> eyre::Result<String> {
        use comrak::nodes::{NodeList, NodeValue};

        let arena = &comrak::Arena::new();
        let output = arena.alloc(NodeValue::Document.into());
        for item in &self.body {
            if !item.visibility().form {
                continue;
            }
            match item {
                TemplateItem::Markdown { attributes, .. } => {
                    append_markdown(arena, output, &attributes.value);
                }
                TemplateItem::TextArea {
                    attributes,
                    validations,
                    ..
                } => {
                    append_header(arena, output, 3, &attributes.label);
                    if let Some(description) = &attributes.description {
                        append_markdown(arena, output, description);
                    }

                    if validations.required {
                        append_node(
                            arena,
                            output,
                            NodeValue::Raw(format!("{FIELD_REQUIRED}\n\n")),
                        );
                    }

                    // Comrak ignores the `fence_char` and `fence_length` fields
                    // when formatting to markdown, so a Raw node is needed instead.
                    append_node(
                        arena,
                        output,
                        NodeValue::Raw(format!(
                            "~~~~~ {info}\n{literal}\n~~~~~\n\n",
                            info = attributes.render.as_deref().unwrap_or("markdown"),
                            literal = attributes.value,
                        )),
                    );
                }
                TemplateItem::Input {
                    attributes,
                    validations,
                    ..
                } => {
                    append_header(arena, output, 3, &attributes.label);
                    if let Some(description) = &attributes.description {
                        append_markdown(arena, output, description);
                    }

                    if validations.required {
                        append_node(
                            arena,
                            output,
                            NodeValue::Raw(format!("{FIELD_REQUIRED}\n\n")),
                        );
                    }

                    if validations.is_number {
                        append_node(arena, output, NodeValue::Raw(format!("{FIELD_NUMBER}\n\n")));
                    }

                    if let Some(regex) = &validations.regex {
                        append_node(
                            arena,
                            output,
                            NodeValue::Raw(format!("{FIELD_REGEX}`{regex}`.\n\n")),
                        );
                    }

                    let textarea_blockquote = append_node(arena, output, NodeValue::BlockQuote);
                    append_markdown_inline(arena, textarea_blockquote, &attributes.value);
                }
                TemplateItem::Dropdown {
                    attributes,
                    validations,
                    ..
                } => {
                    append_header(arena, output, 3, &attributes.label);
                    if let Some(description) = &attributes.description {
                        append_markdown(arena, output, description);
                    }

                    let requirements = match (validations.required, attributes.multiple) {
                        (true, true) => Some(FIELD_AT_LEAST_ONE),
                        (true, false) => Some(FIELD_EXACTLY_ONE),
                        (false, true) => None,
                        (false, false) => Some(FIELD_AT_MOST_ONE),
                    };
                    if let Some(requirements) = requirements {
                        append_node(arena, output, NodeValue::Raw(format!("{requirements}\n\n")));
                    }

                    let list_cfg = NodeList {
                        tight: true,
                        ..Default::default()
                    };
                    let list = append_node(arena, output, NodeValue::List(list_cfg));
                    for list_option in &attributes.options {
                        let list_item = append_node(arena, list, task_item(None));
                        append_markdown_inline(arena, list_item, list_option);
                    }
                    append_node(arena, output, comrak::nodes::NodeValue::Raw("\n".into()));
                }
                TemplateItem::Checkboxes { attributes, .. } => {
                    append_header(arena, output, 3, &attributes.label);
                    if let Some(description) = &attributes.description {
                        append_markdown(arena, output, description);
                    }

                    let list_cfg = NodeList {
                        tight: true,
                        ..Default::default()
                    };
                    let list = append_node(arena, output, NodeValue::List(list_cfg));
                    for list_option in &attributes.options {
                        if list_option.visible.form {
                            let list_item = append_node(arena, list, task_item(None));
                            let label = if list_option.required {
                                &format!("{FIELD_CHECKBOX_REQUIRED}{}", list_option.label)
                            } else {
                                &list_option.label
                            };
                            append_markdown_inline(arena, list_item, label);
                        }
                    }
                    append_node(arena, output, comrak::nodes::NodeValue::Raw("\n".into()));
                }
            }
        }
        let mut output_str = String::new();
        comrak::format_commonmark(output, &MD_OPTIONS, &mut output_str)?;
        Ok(output_str)
    }

    pub fn parse_form<'a, 's: 'a>(&'s self, form: &str) -> eyre::Result<Vec<Option<FieldValue>>> {
        use comrak::nodes::{NodeCodeBlock, NodeList, NodeValue};

        let arena = &comrak::Arena::new();

        let form = comrak::parse_document(arena, form, &MD_OPTIONS);
        let mut form_iter = form.children();

        let mut output = Vec::new();

        let num_regex = std::cell::LazyCell::new(|| {
            let regex_s = r#"^-?[0-9]+(?:\.[0-9]+)?(?:(?:e|E)(?:-|\+)?[0-9]+)?$"#;
            regex::Regex::new(regex_s).expect("invalid regex (bug in forgejo-cli)")
        });

        for item in &self.body {
            if !item.visibility().form {
                output.push(None);
                continue;
            }
            if let Some(header) = item.header() {
                validate_header(arena, &mut form_iter, 3, header)?;
            }
            if let Some(description) = item.description() {
                validate_description(arena, &mut form_iter, description)?;
            }
            match item {
                // this is already covered by the description validation
                TemplateItem::Markdown { .. } => {
                    output.push(None);
                }

                TemplateItem::TextArea {
                    attributes,
                    validations,
                    ..
                } => {
                    if validations.required {
                        validate_description(arena, &mut form_iter, FIELD_REQUIRED)?;
                    }

                    let node = form_iter.next().ok_or_eyre("unexpected EOF")?;

                    let render = attributes.render.as_deref().unwrap_or("markdown");

                    let node_data = node.data.borrow();
                    match &node_data.value {
                        NodeValue::CodeBlock(block)
                            if matches!(
                                **block,
                                NodeCodeBlock {
                                    fenced: true,
                                    fence_char: b'~',
                                    // fence_length: Intentionally not checked for
                                    // in case the user needs to extend the fence
                                    ..
                                }
                            ) && block.info == render =>
                        {
                            ensure_at!(
                                @node_data,
                                !(validations.required && block.literal.is_empty()),
                                "missing required field",
                            );
                            output.push(Some(FieldValue::Input(block.literal.to_owned())));
                        }

                        _ => bail_at!(@node_data, "expected `{render}` codeblock"),
                    }
                }

                TemplateItem::Input { validations, .. } => {
                    if validations.required {
                        validate_description(arena, &mut form_iter, FIELD_REQUIRED)?;
                    }
                    if validations.is_number {
                        validate_description(arena, &mut form_iter, FIELD_NUMBER)?;
                    }
                    if let Some(regex) = &validations.regex {
                        validate_description(
                            arena,
                            &mut form_iter,
                            &format!("{FIELD_REGEX}`{regex}`."),
                        )?;
                    }

                    let field =
                        require_node!(form_iter, NodeValue::BlockQuote, "expected block quote");
                    ensure_at!(
                        field,
                        !(validations.required && field.first_child().is_none()),
                        "missing required field",
                    );
                    ensure_at!(
                        field,
                        field
                            .first_child()
                            .zip(field.last_child())
                            .is_none_or(|(a, b)| a.same_node(b)),
                        "cannot submit multiline value",
                    );

                    let new_doc = arena.alloc(NodeValue::Document.into());
                    for child in field.children() {
                        new_doc.append(child);
                    }

                    let mut body = String::new();
                    comrak::format_commonmark(new_doc, &MD_OPTIONS, &mut body)?;
                    if body.ends_with("\r\n") {
                        body.pop();
                        body.pop();
                    } else if body.ends_with("\n") {
                        body.pop();
                    }
                    if validations.is_number {
                        ensure_at!(
                            field,
                            num_regex.is_match(body.trim()),
                            "submitted value must be a number",
                        );
                    }
                    if let Some(regex_str) = &validations.regex {
                        let regex = regex::Regex::new(regex_str).wrap_err("invalid regex")?;
                        ensure_at!(
                            field,
                            regex.is_match(&body),
                            "must match regex \"{regex_str}\""
                        );
                    }
                    output.push(Some(FieldValue::Input(body)));
                }

                TemplateItem::Dropdown {
                    attributes,
                    validations,
                    ..
                } => {
                    let requirements = match (validations.required, attributes.multiple) {
                        (true, true) => Some(FIELD_AT_LEAST_ONE),
                        (true, false) => Some(FIELD_EXACTLY_ONE),
                        (false, true) => None,
                        (false, false) => Some(FIELD_AT_MOST_ONE),
                    };
                    if let Some(requirements) = requirements {
                        validate_description(arena, &mut form_iter, requirements)?;
                    }

                    let list_cfg = NodeList {
                        is_task_list: true,
                        tight: true,
                        padding: 2,
                        start: 1,
                        bullet_char: b'-',
                        ..Default::default()
                    };
                    let list = require_node!(form_iter, NodeValue::List(list_cfg), "expected list");

                    let mut ticked = Vec::new();

                    let mut children = list.children();
                    let mut is_first_ticked = true;
                    for option in &attributes.options {
                        let child = children.next().ok_or_eyre("unexpected end of list")?;
                        let child_data = child.data.borrow();
                        let is_ticked = match child_data.value {
                            NodeValue::TaskItem(item) => item.symbol.is_some(),
                            _ => bail_at!(@child_data, "expected task list"),
                        };
                        validate_contents(arena, child, option).wrap_err("dropdown")?;

                        if is_ticked {
                            eyre::ensure!(
                                is_first_ticked || attributes.multiple,
                                "only one item can be selected"
                            );
                            is_first_ticked = false;
                        }

                        ticked.push(is_ticked);
                    }
                    if let Some(extra_child) = children.next() {
                        bail_at!(extra_child, "unexpected extra item");
                    }
                    ensure_at!(
                        list,
                        !(validations.required && is_first_ticked),
                        "at least one item must be checked"
                    );
                    output.push(Some(FieldValue::Checkboxes(ticked)));
                }

                TemplateItem::Checkboxes { attributes, .. } => {
                    let list_cfg = NodeList {
                        is_task_list: true,
                        tight: true,
                        padding: 2,
                        start: 1,
                        bullet_char: b'-',
                        ..Default::default()
                    };
                    let list = require_node!(form_iter, NodeValue::List(list_cfg), "expected list");

                    let mut ticked = Vec::new();

                    let mut children = list.children();
                    for option in &attributes.options {
                        if option.visible.form {
                            let child = children.next().ok_or_eyre("unexpected end of list")?;
                            let child_data = child.data.borrow();
                            let is_ticked = match child_data.value {
                                NodeValue::TaskItem(item) => item.symbol.is_some(),
                                _ => bail_at!(@child_data, "expected task list"),
                            };
                            let label = if option.required {
                                &format!("{FIELD_CHECKBOX_REQUIRED}{}", option.label)
                            } else {
                                &option.label
                            };
                            validate_contents(arena, child, label).wrap_err("checkbox")?;
                            ensure_at!(@child_data, is_ticked || !option.required, "option is required");

                            ticked.push(is_ticked);
                        } else {
                            ticked.push(false);
                        }
                    }
                    if let Some(extra_child) = children.next() {
                        bail_at!(extra_child, "unexpected extra item");
                    }
                    output.push(Some(FieldValue::Checkboxes(ticked)));
                }
            }
        }

        Ok(output)
    }

    pub fn generate_content<'a, 's: 'a>(
        &'s self,
        form: Vec<Option<FieldValue>>,
    ) -> eyre::Result<String> {
        use comrak::nodes::{NodeCodeBlock, NodeList, NodeValue};

        let arena = &comrak::Arena::new();

        let output = arena.alloc(NodeValue::Document.into());
        for (item, field_value) in self.body.iter().zip(form) {
            if !item.visibility().content {
                continue;
            }
            match item {
                TemplateItem::Markdown { attributes, .. } => {
                    append_markdown(arena, output, &attributes.value);
                }
                TemplateItem::TextArea { attributes, .. } => {
                    append_header(arena, output, 3, &attributes.label);
                    match field_value {
                        Some(FieldValue::Input(body)) => {
                            if body.trim().is_empty() {
                                append_node(
                                    arena,
                                    output,
                                    NodeValue::Raw("_No response_\n\n".into()),
                                );
                            } else if let Some(render) = &attributes.render {
                                append_node(
                                    arena,
                                    output,
                                    NodeValue::CodeBlock(Box::new(NodeCodeBlock {
                                        fenced: true,
                                        info: render.into(),
                                        literal: body,
                                        ..Default::default()
                                    })),
                                );
                            } else {
                                append_node(arena, output, NodeValue::Raw(body));
                                append_node(arena, output, NodeValue::Raw("\n".into()));
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                TemplateItem::Input { attributes, .. } => {
                    append_header(arena, output, 3, &attributes.label);
                    let value = match field_value {
                        Some(FieldValue::Input(body)) if !body.trim().is_empty() => body,
                        Some(FieldValue::Checkboxes(_)) => unreachable!(),
                        _ => "_No response_".into(),
                    };
                    append_node(arena, output, NodeValue::Raw(value));
                    append_node(arena, output, NodeValue::Raw("\n\n".into()));
                }
                TemplateItem::Dropdown { attributes, .. } => {
                    append_header(arena, output, 3, &attributes.label);
                    let ticked = match field_value {
                        Some(FieldValue::Input(_)) => unreachable!(),
                        Some(FieldValue::Checkboxes(ticked)) => ticked,
                        None => vec![],
                    };
                    let p = append_node(arena, output, NodeValue::Paragraph);
                    let ticked_iter = ticked.into_iter().chain(std::iter::repeat(false));
                    let mut is_first_ticked = true;
                    for (option, is_ticked) in attributes.options.iter().zip(ticked_iter) {
                        if is_ticked {
                            if !is_first_ticked {
                                append_node(arena, p, NodeValue::Raw(", ".into()));
                            }
                            append_node(arena, p, NodeValue::Raw(option.clone()));
                            is_first_ticked = false;
                        }
                    }
                    if is_first_ticked {
                        append_node(arena, output, NodeValue::Raw("_No response_\n\n".into()));
                    }
                }
                TemplateItem::Checkboxes { attributes, .. } => {
                    append_header(arena, output, 3, &attributes.label);
                    let ticked = match field_value {
                        Some(FieldValue::Input(_)) => unreachable!(),
                        Some(FieldValue::Checkboxes(ticked)) => ticked,
                        None => vec![],
                    };
                    let list_cfg = NodeList {
                        tight: true,
                        ..Default::default()
                    };
                    let list = append_node(arena, output, NodeValue::List(list_cfg));
                    let ticked_iter = ticked.into_iter().chain(std::iter::repeat(false));
                    for (option, is_ticked) in attributes.options.iter().zip(ticked_iter) {
                        if option.visible.content {
                            let list_item =
                                append_node(arena, list, task_item(is_ticked.then_some('x')));
                            append_node(arena, list_item, NodeValue::Raw(option.label.clone()));
                        }
                    }
                }
            }
        }
        let mut output_str = String::new();
        comrak::format_commonmark(output, &MD_OPTIONS, &mut output_str)?;
        Ok(output_str)
    }
}

fn append_node<'a>(
    arena: &'a comrak::Arena<'a>,
    parent: &'a comrak::nodes::AstNode<'a>,
    value: comrak::nodes::NodeValue,
) -> &'a comrak::nodes::AstNode<'a> {
    let node = arena.alloc(value.into());
    parent.append(node);
    node
}

fn append_markdown<'a>(
    arena: &'a comrak::Arena<'a>,
    parent: &'a comrak::nodes::AstNode<'a>,
    md: &str,
) {
    append_markdown_inline(arena, parent, md);
    if md.ends_with("\n") {
        append_node(arena, parent, comrak::nodes::NodeValue::Raw("\n".into()));
    } else {
        append_node(arena, parent, comrak::nodes::NodeValue::Raw("\n\n".into()));
    }
}

fn append_markdown_inline<'a>(
    arena: &'a comrak::Arena<'a>,
    parent: &'a comrak::nodes::AstNode<'a>,
    md: &str,
) {
    append_node(arena, parent, comrak::nodes::NodeValue::Raw(md.into()));
}

fn validate_contents<'a>(
    arena: &'a comrak::Arena<'a>,
    parent: &'a comrak::nodes::AstNode<'a>,
    md: &str,
) -> eyre::Result<()> {
    let parsed = comrak::parse_document(arena, md, &MD_OPTIONS);
    ensure_at!(parent, children_eq(parent, parsed), "modified content");
    Ok(())
}

fn validate_description<'a>(
    arena: &'a comrak::Arena<'a>,
    form: &mut comrak::arena_tree::Children<'a, std::cell::RefCell<comrak::nodes::Ast>>,
    md: &str,
) -> eyre::Result<()> {
    let parsed = comrak::parse_document(arena, md, &MD_OPTIONS);
    for a in parsed.children() {
        let b = form.next().ok_or_eyre("unexpected EOF")?;
        ensure_at!(b, nodes_eq(a, b), "modified content");
    }
    Ok(())
}

fn append_header<'a>(
    arena: &'a comrak::Arena<'a>,
    parent: &'a comrak::nodes::AstNode<'a>,
    level: u8,
    content: &str,
) {
    use comrak::nodes::{NodeHeading, NodeValue};
    let header = append_node(
        arena,
        parent,
        NodeValue::Heading(NodeHeading {
            level,
            setext: false,
            closed: true,
        }),
    );
    append_node(arena, header, NodeValue::Raw(content.into()));
}

fn validate_header<'a>(
    arena: &'a comrak::Arena<'a>,
    form: &mut comrak::arena_tree::Children<'a, std::cell::RefCell<comrak::nodes::Ast>>,
    level: u8,
    content: &str,
) -> eyre::Result<()> {
    use comrak::nodes::{NodeHeading, NodeValue};

    let form_heading = require_node!(
        form,
        NodeValue::Heading(NodeHeading {
            level,
            setext: false,
            closed: false,
        }),
        "expected header"
    );
    let parsed = comrak::parse_document(arena, content, &MD_OPTIONS);
    let parsed_inline = parsed.first_child().ok_or_eyre("invalid label")?;
    ensure_at!(
        form_heading,
        children_eq(form_heading, parsed_inline),
        "expected header"
    );
    Ok(())
}

fn nodes_eq<'a>(a: &'a comrak::nodes::AstNode<'a>, b: &'a comrak::nodes::AstNode<'a>) -> bool {
    if a.data.borrow().value != b.data.borrow().value {
        return false;
    }
    children_eq(a, b)
}

fn children_eq<'a>(a: &'a comrak::nodes::AstNode<'a>, b: &'a comrak::nodes::AstNode<'a>) -> bool {
    let mut iter_a = a.children();
    let mut iter_b = b.children();
    loop {
        match (iter_a.next(), iter_b.next()) {
            (Some(a), Some(b)) => {
                if !nodes_eq(a, b) {
                    return false;
                }
            }
            (Some(_), None) | (None, Some(_)) => return false,
            (None, None) => break,
        }
    }
    true
}

fn task_item(symbol: Option<char>) -> comrak::nodes::NodeValue {
    comrak::nodes::NodeValue::TaskItem(comrak::nodes::NodeTaskItem {
        symbol,
        symbol_sourcepos: (0, 0, 0, 0).into(),
    })
}

pub enum FieldValue {
    Input(String),
    Checkboxes(Vec<bool>),
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TemplateItem {
    Markdown {
        attributes: MarkdownItemAttributes,
        #[serde(default = "TemplateVisibility::form")]
        visible: TemplateVisibility,
    },
    TextArea {
        attributes: TextAreaItemAttributes,
        #[serde(default)]
        validations: RequiredValidation,
        #[serde(default = "TemplateVisibility::both")]
        visible: TemplateVisibility,
    },
    Input {
        attributes: InputItemAttributes,
        #[serde(default)]
        validations: InputItemValidation,
        #[serde(default = "TemplateVisibility::both")]
        visible: TemplateVisibility,
    },
    Dropdown {
        attributes: DropdownItemAttributes,
        #[serde(default)]
        validations: RequiredValidation,
        #[serde(default = "TemplateVisibility::both")]
        visible: TemplateVisibility,
    },
    Checkboxes {
        attributes: CheckboxesItemAttributes,
        #[serde(default = "TemplateVisibility::both")]
        visible: TemplateVisibility,
    },
}

impl TemplateItem {
    fn visibility(&self) -> TemplateVisibility {
        match self {
            TemplateItem::Markdown { visible, .. }
            | TemplateItem::TextArea { visible, .. }
            | TemplateItem::Input { visible, .. }
            | TemplateItem::Dropdown { visible, .. }
            | TemplateItem::Checkboxes { visible, .. } => *visible,
        }
    }

    fn header(&self) -> Option<&str> {
        match self {
            TemplateItem::Markdown { .. } => None,
            TemplateItem::TextArea { attributes, .. } => Some(&attributes.label),
            TemplateItem::Input { attributes, .. } => Some(&attributes.label),
            TemplateItem::Dropdown { attributes, .. } => Some(&attributes.label),
            TemplateItem::Checkboxes { attributes, .. } => Some(&attributes.label),
        }
    }

    fn description(&self) -> Option<&str> {
        match self {
            TemplateItem::Markdown { attributes, .. } => Some(&attributes.value),
            TemplateItem::TextArea { attributes, .. } => attributes.description.as_deref(),
            TemplateItem::Input { attributes, .. } => attributes.description.as_deref(),
            TemplateItem::Dropdown { attributes, .. } => attributes.description.as_deref(),
            TemplateItem::Checkboxes { attributes, .. } => attributes.description.as_deref(),
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct MarkdownItemAttributes {
    value: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct TextAreaItemAttributes {
    label: String,
    description: Option<String>,
    #[serde(default)]
    value: String,
    render: Option<String>,
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct RequiredValidation {
    #[serde(default)]
    required: bool,
}

#[derive(serde::Deserialize, Debug)]
pub struct InputItemAttributes {
    label: String,
    description: Option<String>,
    #[serde(default)]
    value: String,
}

#[derive(serde::Deserialize, Debug, Default)]
pub struct InputItemValidation {
    #[serde(default)]
    required: bool,
    #[serde(default)]
    is_number: bool,
    regex: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct DropdownItemAttributes {
    label: String,
    description: Option<String>,
    #[serde(default)]
    multiple: bool,
    options: Vec<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct CheckboxesItemAttributes {
    label: String,
    description: Option<String>,
    options: Vec<CheckboxOption>,
}

#[derive(serde::Deserialize, Debug)]
pub struct CheckboxOption {
    label: String,
    #[serde(default)]
    required: bool,
    #[serde(default = "TemplateVisibility::both")]
    visible: TemplateVisibility,
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(from = "Vec<String>")]
pub struct TemplateVisibility {
    form: bool,
    content: bool,
}

impl TemplateVisibility {
    fn none() -> Self {
        Self {
            form: false,
            content: false,
        }
    }
    fn form() -> Self {
        Self {
            form: true,
            content: false,
        }
    }
    fn both() -> Self {
        Self {
            form: true,
            content: true,
        }
    }
}

impl From<Vec<String>> for TemplateVisibility {
    fn from(vec: Vec<String>) -> Self {
        vec.iter().fold(Self::none(), |mut v, s| {
            match &**s {
                "content" => v.content = true,
                "form" => v.form = true,
                _ => (),
            };
            v
        })
    }
}
