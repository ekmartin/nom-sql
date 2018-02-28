use std::{fmt, str};

use common::{column_identifier_no_alias, integer_literal, opt_multispace, Literal};
use column::Column;

#[derive(Debug, Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ArithmeticBase {
    Column(Column),
    Scalar(Literal),
}

#[derive(Debug, Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ArithmeticExpression {
    pub op: ArithmeticOperator,
    pub left: ArithmeticBase,
    pub right: ArithmeticBase,
}

impl ArithmeticExpression {
    pub fn new(
        op: ArithmeticOperator,
        left: ArithmeticBase,
        right: ArithmeticBase,
    ) -> Self {
        Self {
            op: op,
            left: left,
            right: right,
        }
    }
}

impl fmt::Display for ArithmeticOperator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ArithmeticOperator::Add => write!(f, "+"),
            ArithmeticOperator::Subtract => write!(f, "-"),
            ArithmeticOperator::Multiply => write!(f, "*"),
            ArithmeticOperator::Divide => write!(f, "/"),
        }
    }
}

impl fmt::Display for ArithmeticBase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ArithmeticBase::Column(ref col) => write!(f, "{}", col),
            ArithmeticBase::Scalar(ref lit) => write!(f, "{}", lit.to_string()),
        }
    }
}

impl fmt::Display for ArithmeticExpression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.left, self.op, self.right)
    }
}

/// Parse standard math operators.
/// TODO(malte): this doesn't currently observe operator precedence.
named!(pub arithmetic_operator<&[u8], ArithmeticOperator>,
    alt_complete!(
          map!(tag!("+"), |_| ArithmeticOperator::Add)
        | map!(tag!("-"), |_| ArithmeticOperator::Subtract)
        | map!(tag!("*"), |_| ArithmeticOperator::Multiply)
        | map!(tag!("/"), |_| ArithmeticOperator::Divide)
    )
);

/// Base case for nested arithmetic expressions: column name or literal.
named!(pub arithmetic_base<&[u8], ArithmeticBase>,
    alt_complete!(
          map!(integer_literal, |il| ArithmeticBase::Scalar(il))
        | map!(column_identifier_no_alias, |ci| ArithmeticBase::Column(ci))
    )
);

/// Parse simple arithmetic expressions combining literals, and columns and literals.
/// TODO(malte): this doesn't currently support nested expressions.
named!(pub arithmetic_expression<&[u8], ArithmeticExpression>,
    complete!(do_parse!(
        left: arithmetic_base >>
        opt_multispace >>
        op: arithmetic_operator >>
        opt_multispace >>
        right: arithmetic_base >>
        (ArithmeticExpression {
            op: op,
            left: left,
            right: right,
        })
    ))
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_arithmetic_expressions() {
        use column::FunctionExpression;
        use super::ArithmeticOperator::*;
        use super::ArithmeticBase::Scalar;
        use super::ArithmeticBase::Column as ABColumn;

        let lit_ae = [
            "5 + 42",
            "5+42",
            "5 * 42",
            "5 - 42",
            "5 / 42",
            "2 * 10",
        ];

        // N.B. trailing space in "5 + foo " is required because `sql_identifier`'s keyword
        // detection requires a follow-up character (in practice, there always is one because we
        // use semicolon-terminated queries).
        let col_lit_ae = [
            "foo+5",
            "foo + 5",
            "5 + foo ",
            "foo * bar",
            "MAX(foo)-3333",
        ];

        let expected_lit_ae = [
            ArithmeticExpression::new(Add, Scalar(5.into()), Scalar(42.into())),
            ArithmeticExpression::new(Add, Scalar(5.into()), Scalar(42.into())),
            ArithmeticExpression::new(Multiply, Scalar(5.into()), Scalar(42.into())),
            ArithmeticExpression::new(Subtract, Scalar(5.into()), Scalar(42.into())),
            ArithmeticExpression::new(Divide, Scalar(5.into()), Scalar(42.into())),
            ArithmeticExpression::new(
                Multiply,
                Scalar(2.into()),
                Scalar(10.into()),
            ),
        ];
        let expected_col_lit_ae = [
            ArithmeticExpression::new(Add, ABColumn("foo".into()), Scalar(5.into())),
            ArithmeticExpression::new(Add, ABColumn("foo".into()), Scalar(5.into())),
            ArithmeticExpression::new(Add, Scalar(5.into()), ABColumn("foo".into())),
            ArithmeticExpression::new(
                Multiply,
                ABColumn("foo".into()),
                ABColumn("bar".into()),
            ),
            ArithmeticExpression::new(
                Subtract,
                ABColumn(Column {
                    name: String::from("max(foo)"),
                    table: None,
                    function: Some(Box::new(FunctionExpression::Max("foo".into()))),
                }),
                Scalar(3333.into()),
            ),
        ];

        for (i, e) in lit_ae.iter().enumerate() {
            let res = arithmetic_expression(e.as_bytes());
            assert!(res.is_done());
            assert_eq!(res.unwrap().1, expected_lit_ae[i]);
        }

        for (i, e) in col_lit_ae.iter().enumerate() {
            let res = arithmetic_expression(e.as_bytes());
            assert!(res.is_done());
            assert_eq!(res.unwrap().1, expected_col_lit_ae[i]);
        }
    }

    #[test]
    fn it_displays_arithmetic_expressions() {
        use super::ArithmeticOperator::*;
        use super::ArithmeticBase::Scalar;
        use super::ArithmeticBase::Column as ABColumn;

        let expressions = [
            ArithmeticExpression::new(Add, ABColumn("foo".into()), Scalar(5.into())),
            ArithmeticExpression::new(Subtract, Scalar(5.into()), ABColumn("foo".into())),
            ArithmeticExpression::new(
                Multiply,
                ABColumn("foo".into()),
                ABColumn("bar".into()),
            ),
            ArithmeticExpression::new(Divide, Scalar(10.into()), Scalar(2.into())),
            ArithmeticExpression::new(
                Add,
                Scalar(10.into()),
                Scalar(2.into()),
            ),
        ];

        let expected_strings = ["foo + 5", "5 - foo", "foo * bar", "10 / 2", "10 + 2"];
        for (i, e) in expressions.iter().enumerate() {
            assert_eq!(expected_strings[i], format!("{}", e));
        }
    }
}
