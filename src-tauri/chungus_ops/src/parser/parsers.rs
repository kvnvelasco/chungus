use crate::parser::Import;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until};
use nom::character::complete::char;
use nom::character::complete::multispace0;
use nom::error::ErrorKind;
use nom::sequence::{delimited, terminated, tuple};
use nom::IResult;
use std::path::PathBuf;

pub fn all_possible_import_types(content: &str) -> IResult<&str, Import> {
  alt((
    parse_require_statement,
    parse_import_statement,
    parse_import_promise,
    parse_export_from,
  ))(content)
}

fn parse_import_statement(module_contents: &str) -> IResult<&str, Import> {
  let from = delimited(multispace0, tag("from"), multispace0);
  let import = terminated(tag("import"), multispace0);
  let (next, output) = tuple((import, take_until("from"), from, path_string))(module_contents)?;

  let (_import_text, meat, _from, path) = output;
  let only_contains_type_regex = regex::Regex::new(r#"\{\s*type\s+\w+,?\s+\}"#).unwrap();
  if meat.starts_with("type") || only_contains_type_regex.is_match(&meat) {
    return Err(nom::Err::Error(nom::error::Error::new(
      next,
      ErrorKind::Fail,
    )));
  }

  Ok((next, Import::Import(path)))
}

fn parse_import_promise(module_contents: &str) -> IResult<&str, Import> {
  let (next, output) = delimited(tag("import("), path_string, tag(")"))(module_contents)?;

  Ok((next, Import::AsyncImport(output)))
}

fn parse_require_statement(module_contents: &str) -> IResult<&str, Import> {
  let (next, output) = delimited(tag("require("), path_string, tag(")"))(module_contents)?;

  Ok((next, Import::Require(output)))
}

fn parse_export_from(module_contents: &str) -> IResult<&str, Import> {
  let from = delimited(multispace0, tag("from"), multispace0);
  let export = terminated(tag("export"), multispace0);
  let (next, output) = tuple((export, take_until("from"), from, path_string))(module_contents)?;

  let (_, _, _, path) = output;
  Ok((next, Import::ExportFrom(path)))
}

fn path_string(input: &str) -> IResult<&str, PathBuf> {
  let (next, output) = alt((
    delimited(char('\''), is_not("'"), char('\'')),
    delimited(char('"'), is_not("\""), char('"')),
  ))(input)?;

  Ok((next, PathBuf::from(output)))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::parser::Import;
  use crate::parser::Import::AsyncImport;
  use std::path::PathBuf;

  #[test]
  fn parses_import_statements() {
    let source = r#"import potato from "ramda" "#;
    let output = parse_import_statement(&source);

    assert!(output.is_ok(), "{:?}", &output);
    assert_eq!(
      output.unwrap(),
      (" ", Import::Import(PathBuf::from("ramda")))
    );

    let source = r#"import { something, x } from "ramda"; "#;

    let output = parse_import_statement(&source);

    assert!(output.is_ok(), "{:?}", &output);
    assert_eq!(
      output.unwrap(),
      ("; ", Import::Import(PathBuf::from("ramda")))
    );

    let source = r#"import { 
            something, 
            x 
        } from "ramda"; "#;

    let output = parse_import_statement(&source);

    assert!(output.is_ok(), "{:?}", &output);
    assert_eq!(
      output.unwrap(),
      ("; ", Import::Import(PathBuf::from("ramda")))
    );
  }

  #[test]
  fn it_parses_async_imports() {
    let source = r#"import("ramda")"#;
    let output = parse_import_promise(&source);

    assert!(output.is_ok(), "{:?}", &output);
    assert_eq!(output.unwrap(), ("", AsyncImport(PathBuf::from("ramda"))))
  }

  #[test]
  fn it_parses_export_froms() {
    let source = r#"export * from "./local""#;
    let output = parse_export_from(&source);

    assert!(output.is_ok(), "{:?}", &output);
    assert_eq!(
      output.unwrap(),
      ("", Import::ExportFrom(PathBuf::from("./local")))
    )
  }

  #[test]
  fn it_parses_requires() {
    let source = r#"require("ramda")"#;
    let output = parse_require_statement(&source);

    assert!(output.is_ok(), "{:?}", &output);
    assert_eq!(
      output.unwrap(),
      ("", Import::Require(PathBuf::from("ramda")))
    )
  }
}
