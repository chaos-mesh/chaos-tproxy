use crate::handler::http::selector::Selector;
use crate::handler::http::action::Actions;

#[derive(Debug, Clone)]
pub struct Rule {
    pub target: Target,
    pub selector: Selector,
    pub actions: Actions,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Target {
    Request,
    Response,
}