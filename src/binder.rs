use crate::{catalog::Catalog, parser::Node as ParseNode};

enum BoundStatement {}

struct Binder<'a> {
    catalog: &'a Catalog,
}

impl<'a> Binder<'a> {
    pub fn bind_statement(node: ParseNode) -> BoundStatement {
        todo!()
    }
}
