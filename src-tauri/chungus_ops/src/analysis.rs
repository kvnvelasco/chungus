use crate::module::{Location, Module, ModuleKind, RelativePath};
use crate::resolve::Resolver;
use crate::DependencyCache;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::error::CoreError;
use crate::file::{find_highest_path, FileTree};
use crate::logging::ClientSideLogger;
use crate::webpack_report::{Chunk, WebpackReport};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Analysis {
  node_map: HashMap<Location, usize>,
  file_tree: Option<FileTree>,
  analysis_groups: Vec<Arc<RwLock<AnalysisNode>>>,
  // location and chunk as keys
  #[serde(skip)]
  analysis_group_map: HashMap<(RelativePath, Option<usize>), usize>,

  all_nodes: Vec<Arc<RwLock<AnalysisNode>>>,
  entrypoint: Arc<RwLock<AnalysisNode>>,
  chunks: HashMap<usize, Chunk>,
}

#[derive(Debug, Serialize, Deserialize, Eq, Clone)]
struct AnalysisNode {
  identifier: String,
  stem: Option<PathBuf>,
  full_path: Location,
  is_node_module: bool,
  depth: usize,
  inclusions: Vec<usize>,
  immediate_children: Vec<usize>,
  // A marker that indiciates that this module is no longer present in the final bundle
  tree_shaken: bool,
  chunk: Option<usize>,
  resolver_relative_path: RelativePath,
  incoming: HashSet<usize>,
  outgoing: HashSet<usize>,
}

impl Hash for AnalysisNode {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.chunk.hash(state);
    self.full_path.hash(state);
  }
}

impl PartialEq for AnalysisNode {
  fn eq(&self, other: &Self) -> bool {
    self.chunk.eq(&other.chunk) && (self.full_path.eq(&other.full_path))
  }
}

impl PartialOrd for AnalysisNode {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.chunk.partial_cmp(&other.chunk).map(|cmp| {
      cmp.then(
        self
          .full_path
          .partial_cmp(&other.full_path)
          .unwrap_or(Ordering::Equal),
      )
    })
  }
}

impl Ord for AnalysisNode {
  fn cmp(&self, other: &Self) -> Ordering {
    self.partial_cmp(other).unwrap()
  }
}

impl AnalysisNode {
  pub fn all_possible_group_paths(&self, resolver: &Resolver) -> GroupPaths {
    GroupPaths {
      current_path: self.resolver_relative_path.to_path_buf(),
      resolver_root: resolver.resolve_root.clone(),
    }
  }
}

struct GroupPaths {
  current_path: PathBuf,
  resolver_root: Location,
}

impl Iterator for GroupPaths {
  type Item = Result<RelativePath, CoreError>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.current_path.pop() {
      Some(RelativePath::make_from_path(
        &self.current_path,
        &self.resolver_root,
      ))
    } else {
      None
    }
  }
}

impl Analysis {
  #[tracing::instrument(skip(resolver, cache, logger))]
  pub fn create_from_cache(
    resolver: &Resolver,
    cache: &DependencyCache,
    entrypoint: &Location,
    logger: &impl ClientSideLogger,
  ) -> Result<Self, CoreError> {
    tracing::info!("Creating analysis at entry: {:?}", &entrypoint);
    // do some wild iteraton
    let resolver_relative_path = entrypoint.make_relative_to(&resolver.resolve_root)?;
    let root_node = Arc::new(RwLock::new(AnalysisNode {
      identifier: entrypoint.as_ref().to_string_lossy().to_string(),
      inclusions: vec![],
      immediate_children: vec![],
      depth: entrypoint.as_ref().components().count(),
      chunk: None,
      tree_shaken: false,
      stem: None,
      resolver_relative_path: resolver_relative_path.clone(),
      is_node_module: false,
      full_path: entrypoint.clone(),
      incoming: Default::default(),
      outgoing: Default::default(),
    }));

    let (initial_analysis_groups, initial_analysis_group_map) = {
      let mut groups = vec![];
      let mut hashmap = HashMap::new();
      let group_paths = GroupPaths {
        resolver_root: resolver.resolve_root.clone(),
        current_path: resolver_relative_path.to_path_buf(),
      };

      for (index, group_path) in group_paths.enumerate() {
        let chunk = root_node.read().chunk;
        let location = group_path?;
        let full_path = Location::new(resolver.resolve_root.as_ref().join(&*location))?;

        let analysis_node = Arc::new(RwLock::new(AnalysisNode {
          identifier: full_path.as_ref().to_string_lossy().to_string(),
          chunk: chunk,
          inclusions: vec![0],
          immediate_children: if index == 0 { vec![0] } else { vec![] },
          tree_shaken: false,
          stem: None,
          resolver_relative_path: location.clone(),
          is_node_module: false,
          depth: full_path.as_ref().components().count(),
          full_path,
          incoming: Default::default(),
          outgoing: Default::default(),
        }));

        groups.push(analysis_node);
        hashmap.insert((location.clone(), chunk), groups.len() - 1);
      }

      (groups, hashmap)
    };

    let mut analysis = Self {
      file_tree: None,
      analysis_group_map: initial_analysis_group_map,
      chunks: Default::default(),
      analysis_groups: initial_analysis_groups,

      all_nodes: vec![root_node.clone()],
      entrypoint: root_node.clone(),
      node_map: {
        let mut map = HashMap::new();
        map.insert(entrypoint.clone(), 0);
        map
      },
    };

    analysis.populate(resolver, cache, &*logger)?;

    {
      let highest_path = find_highest_path(
        analysis
          .analysis_groups
          .iter()
          .map(|g| g.read().full_path.clone()),
      );
      if let Some(highest_path) = highest_path {
        use std::iter::FromIterator;
        let filter = HashSet::from_iter(
          analysis
            .all_nodes
            .iter()
            .map(|node| node.read().full_path.clone()),
        );
        logger.message("Creating analysis navigation tree");
        let tree = FileTree::open_from_root_path(&resolver, highest_path.as_ref(), &Some(filter))?;

        analysis.file_tree = Some(tree);
      };
    }

    tracing::info!("Created analysis at entry: {:?}", &entrypoint);
    Ok(analysis)
  }

  #[tracing::instrument(skip(self, webpack_report))]
  pub fn augment_with_webpack_report(
    &mut self,
    webpack_report: &WebpackReport,
    entrypoint_chunk_preference: usize,
  ) {
    tracing::info!(
      "Augmentic analysis of {:?} with webpack report",
      &self.entrypoint.read().full_path
    );

    let chunks_in_entrypoint = webpack_report
      .chunk_mapping
      .get(&self.entrypoint.read().full_path)
      .unwrap();

    let chunk = chunks_in_entrypoint
      .get(entrypoint_chunk_preference)
      .unwrap();

    tracing::info!(
      "Entrypoint chunk index {} found {}",
      entrypoint_chunk_preference,
      chunk.id
    );

    use std::iter::FromIterator;
    let entrypoint_chunk_children = {
      let mut set = HashSet::new();
      for chunk in chunk.children.iter().chain(chunk.siblings.iter()) {
        set.insert(*chunk);
      }

      // also include ourselves if we're in the same chunk
      set.insert(chunk.id);
      set
    };

    tracing::info!("Entrypoint children found {:?}", &entrypoint_chunk_children);
    let mut extra_nodes = vec![];
    for analysis_node_group in self.analysis_groups.iter() {
      let mut identified_chunks = HashSet::<usize>::new();
      for analysis_node in analysis_node_group
        .read()
        .inclusions
        .iter()
        .map(|c| self.all_nodes.get(*c).unwrap())
      {
        if let Some(chunk) = analysis_node.read().chunk {
          identified_chunks.insert(chunk);
          continue;
        }

        let chunks = webpack_report
          .chunk_mapping
          .get(&analysis_node.read().full_path);

        if chunks.is_none() {
          // this file has been removed from the final bundle by some optimization.
          tracing::info!(
            "Node {:?} cannot be found in the chunk map",
            &analysis_node.read().full_path
          );
          analysis_node.write().tree_shaken = true;
          continue;
        }

        let chunks = chunks.unwrap();
        let chunk = chunks
          .iter()
          .find(|c| entrypoint_chunk_children.contains(&c.id));

        if chunk.is_none() {
          // this file has been removed from the final bundle by some optimization.
          tracing::info!(
            "Node {:?} has been tree shaken",
            &analysis_node.read().full_path
          );
          analysis_node.write().tree_shaken = true;
          continue;
        }

        let chunk = chunk.unwrap();
        identified_chunks.insert(chunk.id);

        tracing::info!(
          "Assigning chunk {} to {:?}",
          chunk.id,
          &analysis_node.read().stem
        );
        let mut write_guard = analysis_node.write();
        write_guard.chunk = Some(chunk.id);
        write_guard.identifier = format!("{}?c={}", &write_guard.identifier, chunk.id);
      }
      let mut iterator = identified_chunks.into_iter();
      analysis_node_group.write().chunk = iterator.next();

      while let Some(more_chunk) = iterator.next() {
        let mut inner_node = (*analysis_node_group.read()).clone();
        inner_node.chunk = Some(more_chunk);
        extra_nodes.push(Arc::new(RwLock::new(inner_node)))
      }
    }

    self.analysis_groups.extend(extra_nodes.into_iter());

    self.chunks = {
      let mut map = HashMap::new();
      for chunk_id in entrypoint_chunk_children.iter() {
        if let Some(chunk) = webpack_report.chunk_id_map.get(chunk_id) {
          map.insert(*chunk_id, chunk.clone());
        }
      }
      map
    }
  }

  #[tracing::instrument(skip(self, resolver, cache, logger))]
  fn populate(
    &mut self,
    resolver: &Resolver,
    cache: &DependencyCache,
    logger: &impl ClientSideLogger,
  ) -> Result<(), CoreError> {
    let mut queue = vec![(self.entrypoint.clone(), 0usize)];

    while !queue.is_empty() {
      tracing::debug!("Populating analysis, {} items in queue", queue.len());
      let (next, own_index) = queue.pop().unwrap();

      let module = {
        let next_guard = next.read();
        cache.get(&next_guard.full_path)
      };

      if module.is_none() {
        tracing::warn!("Module {:?} could not be found ", next.read().full_path);
        continue;
      }

      let module = module.unwrap();

      let dependencies: Vec<Location> = module
        .dependencies
        .iter()
        .filter_map(|dep| dep.location())
        .collect();

      let mut outgoing = HashSet::new();
      for dependency in dependencies {
        logger.message(format!("Processing {:?}", &dependency));
        tracing::trace!("Processing dependency at {:?}", &dependency);
        let is_node_module = cache
          .get(&dependency)
          .map(|dependency| dependency.kind == ModuleKind::NodeModule)
          .unwrap_or(false);

        if let Some((target_node, index)) = self
          .node_map
          .get(&dependency)
          .map(|dep| {
            self
              .all_nodes
              .get(*dep)
              .map(|target_node| (target_node, *dep))
          })
          .flatten()
        {
          // attach ourselves to that nodes incoming
          {
            tracing::trace!(
              "Found existing node in tree, attaching self to outgoing node at [{}] {:?}",
              index,
              &dependency
            );
            let mut target_node = target_node.write();
            target_node.incoming.insert(own_index);

            // we also want to attach ourselves to every analysis group that contains the target node;
            for group_path in target_node.all_possible_group_paths(&resolver) {
              let relative_path = group_path?;
              // Invariant. If this node exists in the tree, all of it's groups must also exist.
              let index = self
                .analysis_group_map
                .get(&(relative_path, target_node.chunk))
                .unwrap();
              let mut analysis_group = self.analysis_groups.get(*index).unwrap().write();
              analysis_group.incoming.insert(own_index);
            }
          }
          outgoing.insert(index);
        } else {
          tracing::debug!("Creating new analysis node from {:?}", &module);
          use std::iter::FromIterator;

          let new_analysis_node = Arc::new(RwLock::new(AnalysisNode {
            identifier: dependency.as_ref().to_string_lossy().to_string(),
            immediate_children: vec![],
            inclusions: vec![],
            tree_shaken: false,
            chunk: None,
            is_node_module,
            depth: dependency.as_ref().components().count(),
            stem: Some(
              dependency
                .as_ref()
                .components()
                .rev()
                .take(1)
                .collect::<PathBuf>(),
            ),
            resolver_relative_path: dependency.make_relative_to(&resolver.resolve_root)?,
            outgoing: Default::default(),
            incoming: HashSet::from_iter(vec![own_index]),
            full_path: dependency.clone(),
          }));

          for (index, group_address) in new_analysis_node
            .read()
            .all_possible_group_paths(&resolver)
            .enumerate()
          {
            let relative_path = group_address?;
            if let Some(existing_group) = self
              .analysis_group_map
              .get(&(relative_path.clone(), new_analysis_node.read().chunk))
            {
              let analysis_group = self.analysis_groups.get(*existing_group).unwrap();
              // this happens before insertion so we don't do a -1 here;
              analysis_group.write().inclusions.push(self.all_nodes.len());
              if index == 0 {
                analysis_group
                  .write()
                  .immediate_children
                  .push(self.all_nodes.len())
              }
            } else {
              let location = Location::new(resolver.resolve_root.as_ref().join(&*relative_path))?;
              let analysis_node = AnalysisNode {
                identifier: location.as_ref().to_string_lossy().to_string(),
                stem: None,
                inclusions: vec![self.all_nodes.len()],
                immediate_children: if index == 0 {
                  vec![self.all_nodes.len()]
                } else {
                  vec![]
                },
                depth: location.as_ref().components().count(),
                full_path: location,
                is_node_module: false,
                tree_shaken: false,
                chunk: None,
                resolver_relative_path: relative_path.clone(),
                incoming: HashSet::from_iter(vec![own_index]),
                outgoing: Default::default(),
              };

              self
                .analysis_groups
                .push(Arc::new(RwLock::new(analysis_node)));
              self.analysis_group_map.insert(
                (relative_path.clone(), new_analysis_node.read().chunk),
                self.analysis_groups.len() - 1,
              );
            }
          }

          self.all_nodes.push(new_analysis_node.clone());
          self.node_map.insert(dependency, self.all_nodes.len() - 1);

          queue.push((new_analysis_node.clone(), self.all_nodes.len() - 1));
          outgoing.insert(self.all_nodes.len() - 1);
        }
      }

      {
        let mut next_write = next.write();
        next_write.outgoing = outgoing.clone();

        // write out the stem
        let stem = next_write
          .full_path
          .as_ref()
          .components()
          .rev()
          .take(1)
          .collect::<PathBuf>();
        next_write.stem = Some(stem)
      }
      // update all of the associated analysis groups
      {
        for group_path in next.read().all_possible_group_paths(&resolver) {
          let location = group_path?;
          // this must exist
          let index = self
            .analysis_group_map
            .get(&(location.clone(), next.read().chunk))
            .unwrap();

          // invariant
          let mut analysis_group = self.analysis_groups.get(*index).unwrap().write();
          for item in outgoing.iter() {
            analysis_group.outgoing.insert(*item);
          }
        }
      }
    }
    Ok(())
  }
}
