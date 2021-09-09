use crate::module::Module;

pub struct Node<'a> {
    pub module: &'a Module,
    pub dependencies: Vec<&'a Module>,
}
