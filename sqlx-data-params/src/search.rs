use crate::{IntoParams, Params};

#[derive(Clone, Debug, PartialEq)]
pub struct SearchParams {
    pub query: String,
    pub fields: Vec<String>,

    pub case_sensitive: bool,

    pub exact_match: bool,
}

impl SearchParams {
    pub fn new(query: impl Into<String>, fields: impl IntoIterator<Item = String>) -> Self {
        Self {
            query: query.into(),
            fields: fields.into_iter().collect(),
            case_sensitive: false,
            exact_match: false,
        }
    }

    pub fn with_case_sensitive(mut self, sensitive: bool) -> Self {
        self.case_sensitive = sensitive;
        self
    }

    pub fn with_exact_match(mut self, exact: bool) -> Self {
        self.exact_match = exact;
        self
    }
}

impl IntoParams for SearchParams {
    fn into_params(self) -> Params {
        Params {
            filters: None,
            search: Some(self),
            sort_by: None,
            pagination: None,
            limit: None,
            offset: None,
        }
    }
}
