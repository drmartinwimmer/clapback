use crate::case::{normalize_long_flag, to_kebab_case};
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Error, Expr, ExprArray, ExprLit, Field, Index, Lit, LitBool, LitChar, LitStr,
    Member, Meta, Token,
};

/// Parsed container-level `#[command(...)]` attributes.
#[derive(Debug, Default)]
pub(crate) struct ContainerAttrs {
    /// Static binary name or path (`#[command(program = "...")]` or `#[command(name = "...")]`).
    pub(crate) program: Option<LitStr>,
    /// Static prefix arguments always emitted before field arguments.
    pub(crate) prefix_args: Vec<String>,
    /// Whether `ensure_user_profile_bin_paths` is enabled for this command.
    pub(crate) ensure_user_path: bool,
}

impl ContainerAttrs {
    /// Parses container-level `#[command(...)]` attributes from a slice of [`Attribute`]s.
    pub(crate) fn from_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = Self::default();
        for attr in attrs {
            if attr.path().is_ident("command") {
                attr.parse_nested_meta(|meta| out.apply_meta(&meta))?;
            }
        }
        Ok(out)
    }

    fn apply_meta(&mut self, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("program" | "name") => {
                self.program = Some(meta.value()?.parse()?);
                Ok(())
            }
            Some("arg") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.prefix_args.push(lit.value());
                Ok(())
            }
            Some("args") => {
                self.prefix_args.extend(parse_string_array_value(meta)?);
                Ok(())
            }
            Some("ensure_user_path") => {
                self.ensure_user_path = parse_optional_bool_value(meta)?;
                Ok(())
            }
            _ => Err(meta.error(
                "unsupported container #[command(...)] attribute; expected `program`, `name`, `arg`, `args`, or `ensure_user_path`",
            )),
        }
    }
}

/// Delegation or serialization mode for a struct field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FieldArgKind {
    /// Field only configures `program`, `current_dir`, or `env` and emits no CLI tokens.
    MetadataOnly,
    /// Positional argument (`append_positional`, or `append_last` when `last` is `true`).
    Positional { last: bool },
    /// Named option or flag (`#[arg(long)]`, `#[arg(short)]`).
    Named {
        flag: String,
        false_flag: Option<String>,
        require_equals: bool,
        value_delimiter: Option<char>,
    },
    /// Nested subcommand (`#[command(subcommand)]`).
    Subcommand,
    /// Flattened argument struct (`#[command(flatten)]`).
    Flatten,
}

/// Parsed configuration for a single struct field.
#[derive(Debug)]
pub(crate) struct ParsedField {
    /// Field member accessor (`self.field_name` or `self.0`).
    pub(crate) member: Member,
    /// Static prefix arguments emitted immediately before this field's tokens.
    pub(crate) prefix_args: Vec<String>,
    /// Whether this field overrides the command binary (`#[command(program)]`).
    pub(crate) is_program: bool,
    /// Whether this field sets the command working directory (`#[command(current_dir)]`).
    pub(crate) is_current_dir: bool,
    /// Environment variable names bound to this field (`#[env("...")]`).
    pub(crate) env_vars: Vec<String>,
    /// How this field serializes into CLI arguments.
    pub(crate) arg_kind: FieldArgKind,
}

impl ParsedField {
    /// Parses all attributes on a struct [`Field`] into a [`ParsedField`].
    pub(crate) fn from_field(index: usize, field: &Field) -> syn::Result<Self> {
        let member = match &field.ident {
            Some(ident) => Member::Named(ident.clone()),
            None => Member::Unnamed(Index::from(index)),
        };
        let field_name = field.ident.as_ref().map(ToString::to_string);

        let mut cmd_attrs = FieldCommandAttrs::default();
        let mut raw_arg = RawArgOptions::default();

        for attr in &field.attrs {
            match attr.path().get_ident().map(ToString::to_string).as_deref() {
                Some("env") => raw_arg.env_vars.push(parse_env_attr(attr)?),
                Some("command") => cmd_attrs.parse_attr(attr)?,
                Some("arg") => raw_arg.parse_attr(attr, field_name.as_deref())?,
                _ => {}
            }
        }

        let has_only_metadata =
            (cmd_attrs.is_program || cmd_attrs.is_current_dir || !raw_arg.env_vars.is_empty())
                && !raw_arg.present
                && cmd_attrs.delegate.is_none();

        let arg_kind = raw_arg.resolve_kind(has_only_metadata, cmd_attrs.delegate);

        Ok(Self {
            member,
            prefix_args: cmd_attrs.prefix_args,
            is_program: cmd_attrs.is_program,
            is_current_dir: cmd_attrs.is_current_dir,
            env_vars: raw_arg.env_vars,
            arg_kind,
        })
    }
}

/// Intermediate delegation mode from `#[command(subcommand | flatten)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldCommandDelegate {
    Subcommand,
    Flatten,
}

/// Accumulator for `#[command(...)]` options on a struct field.
#[derive(Debug, Default)]
struct FieldCommandAttrs {
    is_program: bool,
    is_current_dir: bool,
    prefix_args: Vec<String>,
    delegate: Option<FieldCommandDelegate>,
}

impl FieldCommandAttrs {
    fn parse_attr(&mut self, attr: &Attribute) -> syn::Result<()> {
        attr.parse_nested_meta(|meta| self.apply_meta(&meta))
    }

    fn apply_meta(&mut self, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("program") => {
                self.is_program = true;
                Ok(())
            }
            Some("current_dir") => {
                self.is_current_dir = true;
                Ok(())
            }
            Some("subcommand") => {
                self.delegate = Some(FieldCommandDelegate::Subcommand);
                Ok(())
            }
            Some("flatten") => {
                self.delegate = Some(FieldCommandDelegate::Flatten);
                Ok(())
            }
            Some("arg") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.prefix_args.push(lit.value());
                Ok(())
            }
            Some("args") => {
                self.prefix_args.extend(parse_string_array_value(meta)?);
                Ok(())
            }
            _ => Err(meta.error(
                "unsupported field #[command(...)] attribute; expected `program`, `current_dir`, `subcommand`, `flatten`, `arg`, or `args`",
            )),
        }
    }
}

/// Accumulator for `#[arg(...)]` options on a struct field.
#[derive(Debug, Default)]
struct RawArgOptions {
    present: bool,
    long_flag: Option<String>,
    short_flag: Option<String>,
    false_flag: Option<String>,
    require_equals: bool,
    value_delimiter: Option<char>,
    last: bool,
    env_vars: Vec<String>,
}

impl RawArgOptions {
    fn parse_attr(&mut self, attr: &Attribute, field_name: Option<&str>) -> syn::Result<()> {
        self.present = true;
        if matches!(attr.meta, Meta::Path(_)) {
            return Ok(());
        }
        attr.parse_nested_meta(|meta| self.apply_meta(&meta, field_name))
    }

    fn apply_meta(
        &mut self,
        meta: &ParseNestedMeta<'_>,
        field_name: Option<&str>,
    ) -> syn::Result<()> {
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("long") => {
                self.long_flag = Some(parse_long_flag(meta, field_name)?);
                Ok(())
            }
            Some("short") => {
                self.short_flag = Some(parse_short_flag(meta, field_name)?);
                Ok(())
            }
            Some("false_long") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.false_flag = Some(normalize_long_flag(&lit.value()));
                Ok(())
            }
            Some("require_equals") => {
                self.require_equals = parse_optional_bool_value(meta)?;
                Ok(())
            }
            Some("value_delimiter") => {
                let lit: LitChar = meta.value()?.parse()?;
                self.value_delimiter = Some(lit.value());
                Ok(())
            }
            Some("last") => {
                self.last = parse_optional_bool_value(meta)?;
                Ok(())
            }
            Some("trailing_var_arg") => {
                let _trailing = parse_optional_bool_value(meta)?;
                Ok(())
            }
            Some("env") => {
                let lit: LitStr = meta.value()?.parse()?;
                self.env_vars.push(lit.value());
                Ok(())
            }
            _ => Err(meta.error(
                "unsupported #[arg(...)] attribute; expected `long`, `short`, `false_long`, `require_equals`, `value_delimiter`, `last`, `trailing_var_arg`, or `env`",
            )),
        }
    }

    fn resolve_kind(
        &self,
        has_only_metadata: bool,
        delegate: Option<FieldCommandDelegate>,
    ) -> FieldArgKind {
        if let Some(del) = delegate {
            return match del {
                FieldCommandDelegate::Subcommand => FieldArgKind::Subcommand,
                FieldCommandDelegate::Flatten => FieldArgKind::Flatten,
            };
        }

        if has_only_metadata {
            return FieldArgKind::MetadataOnly;
        }

        if let Some(flag) = self.long_flag.clone().or_else(|| self.short_flag.clone()) {
            return FieldArgKind::Named {
                flag,
                false_flag: self.false_flag.clone(),
                require_equals: self.require_equals,
                value_delimiter: self.value_delimiter,
            };
        }

        FieldArgKind::Positional { last: self.last }
    }
}

fn parse_env_attr(attr: &Attribute) -> syn::Result<String> {
    match &attr.meta {
        Meta::List(list) => {
            let lit: LitStr = list.parse_args()?;
            Ok(lit.value())
        }
        Meta::NameValue(nv) => {
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(lit), ..
            }) = &nv.value
            {
                Ok(lit.value())
            } else {
                Err(Error::new_spanned(
                    &nv.value,
                    "expected string literal in #[env = \"...\"]",
                ))
            }
        }
        Meta::Path(_) => Err(Error::new_spanned(
            attr,
            "expected environment variable name, e.g. #[env(\"TMPDIR\")]",
        )),
    }
}

fn parse_long_flag(meta: &ParseNestedMeta<'_>, field_name: Option<&str>) -> syn::Result<String> {
    if meta.input.peek(Token![=]) {
        let lit: LitStr = meta.value()?.parse()?;
        Ok(normalize_long_flag(&lit.value()))
    } else if let Some(ident) = field_name {
        Ok(normalize_long_flag(&to_kebab_case(ident)))
    } else {
        Err(meta.error("#[arg(long)] on tuple struct field requires an explicit flag name"))
    }
}

fn parse_short_flag(meta: &ParseNestedMeta<'_>, field_name: Option<&str>) -> syn::Result<String> {
    if meta.input.peek(Token![=]) {
        let value = meta.value()?;
        if value.peek(LitChar) {
            let lit: LitChar = value.parse()?;
            Ok(format!("-{}", lit.value()))
        } else {
            let lit: LitStr = value.parse()?;
            let val = lit.value();
            let trimmed = val.trim_start_matches('-');
            Ok(format!("-{trimmed}"))
        }
    } else if let Some(ident) = field_name {
        let trimmed = ident.strip_prefix("r#").unwrap_or(ident);
        let first_ch = trimmed
            .chars()
            .next()
            .ok_or_else(|| meta.error("cannot derive short flag from empty identifier"))?;
        Ok(format!("-{first_ch}"))
    } else {
        Err(meta.error("#[arg(short)] on tuple struct field requires an explicit character"))
    }
}

pub(crate) fn parse_optional_bool_value(meta: &ParseNestedMeta<'_>) -> syn::Result<bool> {
    if meta.input.peek(Token![=]) {
        let lit: LitBool = meta.value()?.parse()?;
        Ok(lit.value())
    } else {
        Ok(true)
    }
}

pub(crate) fn parse_string_array_value(meta: &ParseNestedMeta<'_>) -> syn::Result<Vec<String>> {
    let expr: Expr = meta.value()?.parse()?;
    let Expr::Array(ExprArray { elems, .. }) = expr else {
        return Err(Error::new(
            expr.span(),
            "expected array of string literals, e.g. `args = [\"--no-pager\"]`",
        ));
    };

    extract_string_literals(&elems)
}

fn extract_string_literals(elems: &Punctuated<Expr, Token![,]>) -> syn::Result<Vec<String>> {
    let mut values = Vec::with_capacity(elems.len());
    for elem in elems {
        if let Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) = elem
        {
            values.push(lit.value());
        } else {
            return Err(Error::new_spanned(
                elem,
                "expected string literal in `args` array",
            ));
        }
    }
    Ok(values)
}
