use nom::multispace;
use std::str;

use common::opt_multispace;
use select::{limit_clause, nested_selection, order_clause, LimitClause, OrderClause,
             SelectStatement};

#[derive(Clone, Debug, Hash, PartialEq, Deserialize, Serialize)]
pub enum CompoundSelectOperator {
    Union,
    DistinctUnion,
    Intersect,
    Except,
}

#[derive(Clone, Debug, Hash, PartialEq, Deserialize, Serialize)]
pub struct CompoundSelectStatement {
    pub selects: Vec<(Option<CompoundSelectOperator>, SelectStatement)>,
    pub order: Option<OrderClause>,
    pub limit: Option<LimitClause>,
}

/// Parse compound operator
named!(compound_op<&[u8], CompoundSelectOperator>,
    alt_complete!(
          do_parse!(
              tag_no_case!("union") >>
              distinct: opt!(
                  preceded!(multispace,
                            alt_complete!(  map!(tag_no_case!("all"), |_| { false })
                                          | map!(tag_no_case!("distinct"), |_| { true }))
                            )) >>
              (match distinct {
                  // DISTINCT is the default in both MySQL and SQLite
                  None => CompoundSelectOperator::DistinctUnion,
                  Some(d) => {
                      if d {
                          CompoundSelectOperator::DistinctUnion
                      } else {
                          CompoundSelectOperator::Union
                      }
                  },
              })
          )
        | map!(tag_no_case!("intersect"), |_| CompoundSelectOperator::Intersect)
        | map!(tag_no_case!("except"), |_| CompoundSelectOperator::Except)
    )
);

/// Parse compound selection
named!(pub compound_selection<&[u8], CompoundSelectStatement>,
    complete!(do_parse!(
        first_select: delimited!(opt!(tag!("(")), nested_selection, opt!(tag!(")"))) >>
        other_selects: many1!(
            complete!(
                do_parse!(opt_multispace >>
                       op: compound_op >>
                       multispace >>
                       opt!(tag!("(")) >>
                       opt_multispace >>
                       select: nested_selection >>
                       opt_multispace >>
                       opt!(tag!(")")) >>
                       (Some(op), select)
                )
            )
        ) >>
        opt_multispace >>
        order: opt!(order_clause) >>
        limit: opt!(limit_clause) >>
        ({
            let mut v = vec![(None, first_select)];
            v.extend(other_selects);

            CompoundSelectStatement {
                selects: v,
                order: order,
                limit: limit,
            }
        })
    ))
);

#[cfg(test)]
mod tests {
    use super::*;
    use column::Column;
    use common::{Field, FieldExpression};
    use table::Table;

    #[test]
    fn union() {
        let qstr = "SELECT id, 1 FROM Vote UNION SELECT id, stars from Rating;";
        let qstr2 = "(SELECT id, 1 FROM Vote) UNION (SELECT id, stars from Rating);";
        let res = compound_selection(qstr.as_bytes());
        let res2 = compound_selection(qstr2.as_bytes());

        let first_select = SelectStatement {
            tables: vec![Table::from("Vote")],
            fields: vec![
                FieldExpression::new(Field::Col(Column::from("id")), None),
                FieldExpression::new(Field::Literal(1.into()), None),
            ],
            ..Default::default()
        };
        let second_select = SelectStatement {
            tables: vec![Table::from("Rating")],
            fields: vec![
                FieldExpression::new(Field::Col(Column::from("id")), None),
                FieldExpression::new(Field::Col(Column::from("stars")), None),
            ],
            ..Default::default()
        };
        let expected = CompoundSelectStatement {
            selects: vec![
                (None, first_select),
                (Some(CompoundSelectOperator::DistinctUnion), second_select),
            ],
            order: None,
            limit: None,
        };

        assert_eq!(res.unwrap().1, expected);
        assert_eq!(res2.unwrap().1, expected);
    }

    #[test]
    fn multi_union() {
        let qstr = "SELECT id, 1 FROM Vote \
                    UNION SELECT id, stars from Rating \
                    UNION DISTINCT SELECT 42, 5 FROM Vote;";
        let res = compound_selection(qstr.as_bytes());

        let first_select = SelectStatement {
            tables: vec![Table::from("Vote")],
            fields: vec![
                FieldExpression::new(Field::Col(Column::from("id")), None),
                FieldExpression::new(Field::Literal(1.into()), None),
            ],
            ..Default::default()
        };
        let second_select = SelectStatement {
            tables: vec![Table::from("Rating")],
            fields: vec![
                FieldExpression::new(Field::Col(Column::from("id")), None),
                FieldExpression::new(Field::Col(Column::from("stars")), None),
            ],
            ..Default::default()
        };
        let third_select = SelectStatement {
            tables: vec![Table::from("Vote")],
            fields: vec![
                FieldExpression::new(Field::Literal(42.into()), None),
                FieldExpression::new(Field::Literal(5.into()), None),
            ],
            ..Default::default()
        };

        let expected = CompoundSelectStatement {
            selects: vec![
                (None, first_select),
                (Some(CompoundSelectOperator::DistinctUnion), second_select),
                (Some(CompoundSelectOperator::DistinctUnion), third_select),
            ],
            order: None,
            limit: None,
        };

        assert_eq!(res.unwrap().1, expected);
    }

    #[test]
    fn union_all() {
        let qstr = "SELECT id, 1 FROM Vote UNION ALL SELECT id, stars from Rating;";
        let res = compound_selection(qstr.as_bytes());

        let first_select = SelectStatement {
            tables: vec![Table::from("Vote")],
            fields: vec![
                FieldExpression::new(Field::Col(Column::from("id")), None),
                FieldExpression::new(Field::Literal(1.into()), None),
            ],
            ..Default::default()
        };
        let second_select = SelectStatement {
            tables: vec![Table::from("Rating")],
            fields: vec![
                FieldExpression::new(Field::Col(Column::from("id")), None),
                FieldExpression::new(Field::Col(Column::from("stars")), None),
            ],
            ..Default::default()
        };
        let expected = CompoundSelectStatement {
            selects: vec![
                (None, first_select),
                (Some(CompoundSelectOperator::Union), second_select),
            ],
            order: None,
            limit: None,
        };

        assert_eq!(res.unwrap().1, expected);
    }
}
