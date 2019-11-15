#![allow(missing_docs)]

use crate::Result;
use lazy_static::lazy_static;
use pest::iterators::{Pair, Pairs};
use pest::prec_climber::{Assoc, Operator, PrecClimber};
use pest::Parser;
use pest_derive::*;

/// Helper struct to parse metric event expressions.
#[derive(Parser)]
#[grammar = "pmu/metric_parser.pest"]
struct MetricExprParser;

lazy_static! {
    /// `PrecClimber` used internally to parse an matric event's expression.
    static ref CLIMBER: PrecClimber<Rule> = {
        PrecClimber::new(vec![
            Operator::new(Rule::comma, Assoc::Left),
            Operator::new(Rule::add, Assoc::Left) | Operator::new(Rule::sub, Assoc::Left),
            Operator::new(Rule::mul, Assoc::Left) | Operator::new(Rule::div, Assoc::Left),
        ])
    };
}

/// Parsed (sub)expression from a `PmuEvent` dealing with derived events.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricExpr {
    /// A constant number.
    Num(f32),
    /// A PMU event.
    Var(String),
    /// Addition operator.
    Add(Box<MetricExpr>, Box<MetricExpr>),
    /// Subtraction operator.
    Sub(Box<MetricExpr>, Box<MetricExpr>),
    /// Multiplication operator.
    Mul(Box<MetricExpr>, Box<MetricExpr>),
    /// Division operator.
    Div(Box<MetricExpr>, Box<MetricExpr>),
    /// If-Else block
    If(Box<MetricExpr>, Box<MetricExpr>, Box<MetricExpr>),
    /// Min block
    Min(Box<MetricExpr>),
    /// Comma seperated expression to be used with `Min`.
    Comma(Box<MetricExpr>, Box<MetricExpr>),
}

impl MetricExpr {
    /// Create an new `MetricExpr` from a supplied `&str`.
    pub fn parse_str(input: &str) -> Result<MetricExpr> {
        let expr = MetricExprParser::parse(Rule::expr, input)?;
        Ok(MetricExpr::_to_expr(expr))
    }

    /// Recursive call to transform `Pair` objects into `MetricExpr`s.
    fn _to_expr(expr: Pairs<Rule>) -> MetricExpr {
        CLIMBER.climb(
            expr,
            |pair: Pair<Rule>| match pair.as_rule() {
                Rule::num => MetricExpr::Num(pair.as_str().parse().unwrap()),
                Rule::ident => MetricExpr::Var(pair.as_str().into()),
                Rule::min => MetricExpr::Min(Box::new(MetricExpr::_to_expr(pair.into_inner()))),
                Rule::expr => MetricExpr::_to_expr(pair.into_inner()),
                _ => unreachable!(),
            },
            |lhs: MetricExpr, op: Pair<Rule>, rhs: MetricExpr| match op.as_rule() {
                Rule::add => MetricExpr::Add(Box::new(lhs), Box::new(rhs)),
                Rule::sub => MetricExpr::Sub(Box::new(lhs), Box::new(rhs)),
                Rule::mul => MetricExpr::Mul(Box::new(lhs), Box::new(rhs)),
                Rule::div => MetricExpr::Div(Box::new(lhs), Box::new(rhs)),
                Rule::comma => MetricExpr::Comma(Box::new(lhs), Box::new(rhs)),
                _ => unreachable!(),
            },
        )
    }

    /// Get names of all counters used in this expression.
    pub fn get_counters(&self) -> Vec<&String> {
        macro_rules! body {
            ($a:expr, $b:expr) => {{
                let mut tmp = $a.get_counters();
                tmp.extend($b.get_counters());
                tmp
            }};
        };
        match self {
            MetricExpr::Var(ref x) => vec![x],
            MetricExpr::Add(ref a, ref b) => body!(a, b),
            MetricExpr::Sub(ref a, ref b) => body!(a, b),
            MetricExpr::Mul(ref a, ref b) => body!(a, b),
            MetricExpr::Div(ref a, ref b) => body!(a, b),
            MetricExpr::Min(ref a) => a.get_counters(),
            MetricExpr::Comma(ref a, ref b) => body!(a, b),
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_expression() {
        let test_strings = vec![
            "1 - ( (IDQ_UOPS_NOT_DELIVERED.CORE / (4 * cycles)) + (( UOPS_ISSUED.ANY - UOPS_RETIRED.RETIRE_SLOTS + 4 * INT_MISC.RECOVERY_CYCLES ) / (4 * cycles)) + (UOPS_RETIRED.RETIRE_SLOTS / (4 * cycles)) )",
            "1 - CPU_CLK_THREAD_UNHALTED.ONE_THREAD_ACTIVE / ( CPU_CLK_THREAD_UNHALTED.REF_XCLK_ANY / 2 ) if #SMT_on else 0",
            "min( 1 , IDQ.MITE_UOPS / ( (UOPS_RETIRED.RETIRE_SLOTS / INST_RETIRED.ANY) * 16 * ( ICACHE.HIT + ICACHE.MISSES ) / 4.0 ) )",
        ];

        let metrics: Vec<MetricExpr> = test_strings
            .iter()
            .map(|x| {
                let expr = MetricExpr::parse_str(&x);
                assert!(expr.is_ok());
                expr.unwrap()
            })
            .collect();
        assert_eq!(test_strings.len(), metrics.len());

        let events: Vec<Vec<&String>> = metrics
            .iter()
            .map(|x| {
                let counters = x.get_counters();
                assert!(!counters.is_empty());
                counters
            })
            .collect();
        assert_eq!(test_strings.len(), events.len());
    }
}
