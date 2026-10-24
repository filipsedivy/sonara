//! The building blocks of the declarative tool contract: input models, their fields and the
//! kinds of value a field accepts, mirroring the pydantic models of the prototype
//! (`sonara.agent.schemas.models`).
//!
//! Every model in the table is plain `static` data. The JSON Schemas (`super::schema`) and the
//! argument validation (`super::validation`) are both generated from it, so the published
//! contract and the accepted arguments cannot drift apart.

use std::fmt;
use std::sync::OnceLock;

use regex::Regex;

/// A number as it is written in the prototype's models. `le=999` and `le=0.25` stay an integer
/// and a float, because the JSON Schema keeps the distinction (`"maximum": 999` on a float
/// field) and the golden schemas record it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    /// An integer literal, for example `ge=10`.
    Int(i64),
    /// A float literal, for example `le=0.25`.
    Float(f64),
}

impl Number {
    /// The value as a float, for comparisons on float fields.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "the bounds in the table are small integers, exactly representable as f64"
    )]
    pub const fn as_f64(self) -> f64 {
        match self {
            Self::Int(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

impl fmt::Display for Number {
    /// Formats the bound the way pydantic-core (written in Rust) formats it in error messages:
    /// with Rust's `Display`, so `999.0` prints as `999` and `0.25` as `0.25`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
        }
    }
}

/// Numeric constraints of an `int` or `float` field (pydantic's `ge`, `gt`, `le` and `lt`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberBounds {
    /// Inclusive minimum.
    pub ge: Option<Number>,
    /// Exclusive minimum.
    pub gt: Option<Number>,
    /// Inclusive maximum.
    pub le: Option<Number>,
    /// Exclusive maximum.
    pub lt: Option<Number>,
}

impl NumberBounds {
    /// No constraints.
    pub const NONE: Self = Self {
        ge: None,
        gt: None,
        le: None,
        lt: None,
    };

    /// Adds an inclusive minimum.
    #[must_use]
    pub const fn ge(mut self, bound: Number) -> Self {
        self.ge = Some(bound);
        self
    }

    /// Adds an exclusive minimum.
    #[must_use]
    pub const fn gt(mut self, bound: Number) -> Self {
        self.gt = Some(bound);
        self
    }

    /// Adds an inclusive maximum.
    #[must_use]
    pub const fn le(mut self, bound: Number) -> Self {
        self.le = Some(bound);
        self
    }

    /// Adds an exclusive maximum.
    #[must_use]
    pub const fn lt(mut self, bound: Number) -> Self {
        self.lt = Some(bound);
        self
    }
}

/// Length constraints of a string (in characters) or a list (in items).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LengthBounds {
    /// Minimum length.
    pub min_length: Option<usize>,
    /// Maximum length.
    pub max_length: Option<usize>,
}

impl LengthBounds {
    /// No constraints.
    pub const NONE: Self = Self {
        min_length: None,
        max_length: None,
    };

    /// Adds a minimum length.
    #[must_use]
    pub const fn min_length(mut self, length: usize) -> Self {
        self.min_length = Some(length);
        self
    }

    /// Adds a maximum length.
    #[must_use]
    pub const fn max_length(mut self, length: usize) -> Self {
        self.max_length = Some(length);
        self
    }
}

/// A `pattern` constraint. pydantic matches patterns with the Rust `regex` crate by default
/// (`regex_engine="rust-regex"`, which the prototype keeps), so the same crate reproduces it
/// exactly: `\d` matches any Unicode digit and `$` matches only at the very end, unlike Python's
/// `re` (checked against the prototype: `'a\n'` does not match `^a$`).
#[derive(Debug)]
pub struct Pattern {
    source: &'static str,
    compiled: OnceLock<Regex>,
}

impl Pattern {
    /// A pattern; it is compiled on first use.
    #[must_use]
    pub const fn new(source: &'static str) -> Self {
        Self {
            source,
            compiled: OnceLock::new(),
        }
    }

    /// The pattern as written in the model, as it appears in schemas and error messages.
    #[must_use]
    pub const fn source(&self) -> &'static str {
        self.source
    }

    /// The compiled expression.
    ///
    /// # Panics
    ///
    /// If the pattern does not compile. Every pattern of the table is compiled by a unit test,
    /// so this cannot happen at run time.
    #[must_use]
    pub fn regex(&self) -> &Regex {
        self.compiled.get_or_init(|| {
            Regex::new(self.source).expect("the table's patterns are compiled by a unit test")
        })
    }
}

/// Constraints of a `str` field.
#[derive(Debug, Clone, Copy)]
pub struct StrRules {
    /// Length in characters (Unicode scalar values, as Python's `len`).
    pub length: LengthBounds,
    /// A pattern the (stripped) string must match.
    pub pattern: Option<&'static Pattern>,
}

impl StrRules {
    /// No constraints.
    pub const NONE: Self = Self {
        length: LengthBounds::NONE,
        pattern: None,
    };

    /// Adds a minimum length.
    #[must_use]
    pub const fn min_length(mut self, length: usize) -> Self {
        self.length = self.length.min_length(length);
        self
    }

    /// Adds a maximum length.
    #[must_use]
    pub const fn max_length(mut self, length: usize) -> Self {
        self.length = self.length.max_length(length);
        self
    }

    /// Adds a pattern.
    #[must_use]
    pub const fn pattern(mut self, pattern: &'static Pattern) -> Self {
        self.pattern = Some(pattern);
        self
    }

    /// Whether any constraint is set. pydantic-core names such a validator `constrained-str`.
    #[must_use]
    pub const fn is_constrained(&self) -> bool {
        self.length.min_length.is_some()
            || self.length.max_length.is_some()
            || self.pattern.is_some()
    }
}

/// The kind of value a field accepts: the annotation of a pydantic field.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// `bool`.
    Bool,
    /// `int` with optional constraints.
    Int(NumberBounds),
    /// `float` with optional constraints.
    Float(NumberBounds),
    /// `str` with optional constraints.
    Str(StrRules),
    /// `Literal["a", "b", …]`.
    StrLiteral(&'static [&'static str]),
    /// `Literal[1, 2, …]`.
    IntLiteral(&'static [i64]),
    /// `list[item]` with optional length constraints.
    List(&'static Kind, LengthBounds),
    /// `dict[str, value]`.
    Dict(&'static Kind),
    /// `A | B | …` (pydantic's smart-mode union), in declaration order.
    Union(&'static [Kind]),
    /// `X | None`.
    Nullable(&'static Kind),
    /// A nested model.
    Model(&'static Model),
    /// `Annotated[inner, Field(description=…)]`: a shared annotated type such as `Position`,
    /// whose description belongs to the type rather than to a field.
    Described(&'static Kind, &'static str),
}

impl Kind {
    /// `int` without constraints.
    pub const INT: Self = Self::Int(NumberBounds::NONE);
    /// `float` without constraints.
    pub const FLOAT: Self = Self::Float(NumberBounds::NONE);
    /// `str` without constraints.
    pub const STR: Self = Self::Str(StrRules::NONE);
}

/// A literal default value (`Field(120.0)`, `= None`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constant {
    /// `None`.
    Null,
    /// A boolean.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A float; it stays a float in schemas and normalized arguments (`120.0`).
    Float(f64),
    /// A string.
    Str(&'static str),
}

/// A `default_factory`. pydantic omits factory defaults from the JSON Schema; validation still
/// fills them in.
#[derive(Debug, Clone, Copy)]
pub enum Factory {
    /// `default_factory=list`.
    EmptyList,
    /// `default_factory=dict`.
    EmptyDict,
    /// `default_factory=Model`: an instance with every field at its default.
    Model(&'static Model),
    /// `default_factory=lambda: [Model()]`: a list with one default instance.
    ModelList(&'static Model),
}

/// How a missing field is filled in.
#[derive(Debug, Clone, Copy)]
pub enum FieldDefault {
    /// The field is required.
    Required,
    /// A literal default.
    Value(Constant),
    /// A default factory.
    Factory(Factory),
}

/// A pydantic `field_validator` in `after` mode: it receives the validated value and returns
/// the message of the `ValueError` it raises, if any.
pub type AfterValidator = fn(&serde_json::Value) -> Option<&'static str>;

/// One field of an input model.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    /// The field name, which is also the argument name.
    pub name: &'static str,
    /// The accepted values.
    pub kind: Kind,
    /// The default, if the field is optional.
    pub default: FieldDefault,
    /// `Field(description=…)`.
    pub description: Option<&'static str>,
    /// A `field_validator` that runs after the value validated.
    pub after: Option<AfterValidator>,
}

impl Field {
    /// A required field without a description.
    #[must_use]
    pub const fn new(name: &'static str, kind: Kind) -> Self {
        Self {
            name,
            kind,
            default: FieldDefault::Required,
            description: None,
            after: None,
        }
    }

    /// Sets a literal default.
    #[must_use]
    pub const fn default_value(mut self, value: Constant) -> Self {
        self.default = FieldDefault::Value(value);
        self
    }

    /// Sets a default factory.
    #[must_use]
    pub const fn default_factory(mut self, factory: Factory) -> Self {
        self.default = FieldDefault::Factory(factory);
        self
    }

    /// Sets the description.
    #[must_use]
    pub const fn description(mut self, text: &'static str) -> Self {
        self.description = Some(text);
        self
    }

    /// Sets an `after` field validator.
    #[must_use]
    pub const fn after(mut self, validator: AfterValidator) -> Self {
        self.after = Some(validator);
        self
    }

    /// Whether the field has no default.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        matches!(self.default, FieldDefault::Required)
    }
}

/// An input model: a pydantic `BaseModel` with `extra="forbid"`. Every model of the prototype
/// forbids extra fields, so the table has no setting for it.
#[derive(Debug)]
pub struct Model {
    /// The pydantic class name. It names the model in `$defs`, titles nested models and appears
    /// in the "valid dictionary or instance of …" error.
    pub name: &'static str,
    /// `str_strip_whitespace=True`: set for the `ToolInput` models, unset for the plain nested
    /// models (`NoteInput`, `SendSpec`, …). It applies to every string the model validates
    /// itself, including list items and dict keys, but not to nested models.
    pub strip_whitespace: bool,
    /// The fields in pydantic's order (base classes first, in reverse method resolution order).
    pub fields: &'static [Field],
}

impl Model {
    /// A model derived from the prototype's `ToolInput` (strips whitespace).
    #[must_use]
    pub const fn tool_input(name: &'static str, fields: &'static [Field]) -> Self {
        Self {
            name,
            strip_whitespace: true,
            fields,
        }
    }

    /// A plain nested model (`BaseModel` with `extra="forbid"` only).
    #[must_use]
    pub const fn plain(name: &'static str, fields: &'static [Field]) -> Self {
        Self {
            name,
            strip_whitespace: false,
            fields,
        }
    }

    /// The field called `name`.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }
}
