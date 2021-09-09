use crate::error::CoreError;
use crate::module::Location;
use crate::resolve::Resolver;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Debug)]
pub struct WebpackReport {
  pub chunk_mapping: HashMap<Location, Vec<Chunk>>,
  pub chunk_id_map: HashMap<usize, Chunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
  pub id: usize,
  pub name: String,
  pub initial: bool,
  pub parents: Vec<usize>,
  pub siblings: Vec<usize>,
  pub children: Vec<usize>,
  pub parsed_size: usize,
}

pub mod v4 {
  use super::Chunk as WebpackChunk;
  use crate::error::CoreError;
  use crate::module::Location;
  use crate::resolve::Resolver;
  use crate::webpack_report::WebpackReport;
  use regex::Regex;
  use serde::Deserialize;
  use std::collections::HashMap;
  use std::io::Read;
  use std::path::PathBuf;

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct WebpackReportRaw {
    // Javascript is stupid and does stuff like this constantly. If you have multiple configurations in your
    // webpack root. You end up with 1 or more configurations.
    modules: Option<Vec<Module>>,
    chunks: Option<Vec<Chunk>>,

    children: Option<Vec<WebpackReportRaw>>,
  }

  #[derive(Debug, Deserialize, Clone)]
  #[serde(rename_all = "camelCase")]
  struct Chunk {
    entry: bool,
    children: Vec<usize>,
    initial: bool,
    parents: Vec<usize>,
    siblings: Vec<usize>,
    id: usize,
    size: usize,
    names: Vec<String>,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Module {
    id: Option<usize>,
    name: String,
    chunks: Vec<usize>,
    // reasons: Vec<Reason>,
    modules: Option<Vec<Module>>,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct Reason {
    module: Option<String>,
    user_request: String,
  }

  #[tracing::instrument(skip(resolve, value))]
  pub fn create_report_from_reader(
    resolve: &Resolver,
    value: impl Read,
  ) -> Result<WebpackReport, CoreError> {
    WebpackReportRaw::parse_from_reader(resolve, value)
  }
  impl WebpackReportRaw {
    pub fn parse_from_reader(
      resolve: &Resolver,
      value: impl Read,
    ) -> Result<WebpackReport, CoreError> {
      // Replace all of the loader paths
      let path_normalisation_regex = Regex::new(".+!").unwrap();
      let webpack_reports: Vec<Self> = {
        let raw_reports: Self = serde_json::from_reader(value)?;

        if let Some(children) = raw_reports.children {
          children
        } else {
          vec![raw_reports]
        }
      };

      let mut output_map = HashMap::new();
      let mut chunk_map = HashMap::new();
      for webpack_report in webpack_reports {
        assert!(
          webpack_report.chunks.is_some(),
          "No chunk map defintion found in report"
        );
        assert!(
          webpack_report.modules.is_some(),
          "No module definitions found in report"
        );

        let chunks = webpack_report.chunks.unwrap();

        for chunk in chunks {
          tracing::info!("Adding chunk {}", chunk.id);
          chunk_map.insert(
            chunk.id,
            WebpackChunk {
              id: chunk.id,
              siblings: chunk.siblings,
              initial: chunk.initial,
              parents: chunk.parents,
              children: chunk.children,
              name: chunk.names.join(", "),
              parsed_size: chunk.size,
            },
          );
        }

        let mut queue = webpack_report.modules.unwrap();

        loop {
          if let Some(module) = queue.pop() {
            // Not sure what to do here yet
            if module.name.contains(" (ignored)")
              || module.name.contains(" sync ")
              || module.name.starts_with("external ")
            {
              continue;
            }
            // this is a composite module
            if let Some(mut child_modules) = module.modules {
              for child_module in &mut child_modules {
                // pass down the chunk mapping we have to the children. they're also in those chunks
                child_module.chunks = module.chunks.clone()
              }
              queue.append(&mut child_modules);
              continue;
            }

            let path = resolve
              .resolve_root
              .as_ref()
              .join(&*path_normalisation_regex.replace_all(&module.name, ""));

            let location = Location::new(path)?;
            let mut chunks: Vec<WebpackChunk> = module
              .chunks
              .iter()
              .map(|id| chunk_map.get(id).unwrap().clone())
              .collect();
            let entry = output_map.entry(location).or_insert(vec![]);
            entry.append(&mut chunks);
          } else {
            break;
          }
        }
      }

      tracing::info!("Webpack report generation complete");

      Ok(WebpackReport {
        chunk_id_map: chunk_map,
        chunk_mapping: output_map,
      })
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;
    use crate::resolve::Resolver;
    use std::path::Path;

    #[test]
    fn it_can_parse_a_report() {
      let report = include_str!("sample_javascript/compilation-stats.json");
      let resolver = Resolver::new(
        &Location::new(Path::new(
          "/home/kevin_velasco/data_disk/Projects/mathspace/mathspace/local_modules",
        ))
        .unwrap(),
        vec![],
      );

      let report = WebpackReportRaw::parse_from_string(&resolver, &report).unwrap();

      dbg!(report);
      todo!()
    }
  }
}
