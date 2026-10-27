use crate::handler::http::action::Actions;
use crate::handler::http::selector::Selector;

/// Rule introduces a set of rules would effect the HTTP request/response.
#[derive(Debug, Clone)]
pub struct Rule {
    /// target would indicate which would be affected by the rule, HTTP request or response.
    pub target: Target,
    /// Selectors contains a set of filters to check whether the request/response should be affected.
    pub selector: Selector,
    /// actions introduces the expected modification.
    pub actions: Actions,
}

/// Target introduces the [Rule] should effect on HTTP request or response.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Target {
    Request,
    Response,
}
