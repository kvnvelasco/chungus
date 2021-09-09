use crate::parser::parsers::all_possible_import_types;
use std::any::TypeId;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
mod parsers;

#[derive(Debug)]
pub enum ParseError {
    Unknown(Box<dyn std::error::Error + 'static>),
}

impl Display for ParseError {
    fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::Unknown(e) => Some(e.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub enum Import {
    Require(PathBuf),
    AsyncImport(PathBuf),
    ExportFrom(PathBuf),
    Import(PathBuf),
    NodeDependency(PathBuf),
}

impl AsRef<Path> for Import {
    fn as_ref(&self) -> &Path {
        match self {
            Import::Require(p) => &p,
            Import::AsyncImport(p) => &p,
            Import::ExportFrom(p) => &p,
            Import::Import(p) => &p,
            Import::NodeDependency(p) => &p,
        }
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq)]
#[repr(transparent)]
pub struct UnresolvedImport(pub Import);

#[cfg(test)]
impl UnresolvedImport {
    pub fn new_from_import(import: Import) -> Self {
        Self(import)
    }
}

impl AsRef<Path> for UnresolvedImport {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl UnresolvedImport {
    pub fn import_kind(&self) -> &Import {
        &self.0
    }
    pub fn parse_many(module_contents: impl AsRef<str>) -> Result<Vec<Self>, ParseError> {
        let mut contents = module_contents.as_ref().to_owned();
        let mut output = vec![];

        loop {
            let result = all_possible_import_types(&contents);
            match result {
                Ok((remaining, out)) => {
                    output.push(UnresolvedImport(out));
                    contents = remaining.to_owned();
                }
                Err(_) => {
                    if contents.is_empty() {
                        break;
                    }
                    contents.remove(0);
                }
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::UnresolvedImport;

    #[test]
    fn it_parses_mutliple_import_statements() {
        let source = r#"
            import * as poato from "ramda";
            import { a, b, c } from "gallileo";
            
            import bazooka from "bazooka";
            import Component from "./components/component";
            
            export * from './local';
        "#;

        let output = UnresolvedImport::parse_many(source);
        assert!(output.is_ok(), "{:?}", &output);
        let output = output.unwrap();

        use super::Import::*;

        assert_eq!(
            output,
            vec![
                UnresolvedImport(Import("ramda".into())),
                UnresolvedImport(Import("gallileo".into())),
                UnresolvedImport(Import("bazooka".into())),
                UnresolvedImport(Import("./components/component".into())),
                UnresolvedImport(ExportFrom("./local".into()))
            ]
        )
    }

    #[test]
    fn it_can_parse_javascript_in_odd_ways() {
        let source = r#"
            import * as poato from "ramda";
            import { a, b, c } from "gallileo";
            
            import bazooka from "bazooka";
            import Component from "./components/component";
            
            export default function Component() {
              const module = import('./async/Component');
            }
            
        "#;

        let output = UnresolvedImport::parse_many(source);
        assert!(output.is_ok(), "{:?}", &output);
        let output = output.unwrap();
        use super::Import::*;
        assert_eq!(
            output,
            vec![
                UnresolvedImport(Import("ramda".into())),
                UnresolvedImport(Import("gallileo".into())),
                UnresolvedImport(Import("bazooka".into())),
                UnresolvedImport(Import("./components/component".into())),
                UnresolvedImport(AsyncImport("./async/Component".into()))
            ]
        )
    }
}
