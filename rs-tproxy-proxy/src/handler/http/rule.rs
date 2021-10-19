use super::action::Actions;
use super::selector::Selector;

#[derive(Debug, Clone)]
pub struct Rule {
    pub target: Target,
    pub selector: Selector,
    pub actions: Actions,
    pub plugins: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Target {
    Request,
    Response,
}
