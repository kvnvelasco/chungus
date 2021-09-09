import React, { useEffect, useState } from "react";

import "./App.css";
import {
  Classes,
  Colors,
  Icon,
  ITreeNode,
  Spinner,
  Tab,
  Tabs,
  Tag,
  Text,
  Tree,
} from "@blueprintjs/core";

import "@blueprintjs/core/lib/css/blueprint.css";
import { ConfigurationPanel } from "./components/Configuration";
import {
  ApplicationState,
  FileNode,
  RootTree,
  useAnalyseEntrypoint,
  useApplicationState,
  useEntrypointAnalysis,
} from "./components/state";
import { TreeMap } from "./components/D3TreeMap";

function create_root_tree(
  configuration: ApplicationState,
  expansion_set: Set<string>,
  tree: RootTree
): ITreeNode[] {
  return tree.file_node.children
    .filter((child) => {
      const splits = child.path.split(".");
      return (
        child.children.length > 0 ||
        configuration.extensions.includes(splits[splits.length - 1])
      );
    })
    .map((child) => create_tree_nodes(configuration, expansion_set, child));
}
function create_tree_nodes(
  configuration: ApplicationState,
  expansion_set: Set<string>,
  node: FileNode
): ITreeNode {
  let has_children = node.children.length > 0;
  const open = expansion_set.has(node.full_path);

  const children = open
    ? node.children.map((child: any) =>
        create_tree_nodes(configuration, expansion_set, child)
      )
    : [];

  return {
    id: node.full_path,
    secondaryLabel: node.valid_entrypoint ? (
      <Tag className={Classes.INTENT_SUCCESS}> Entry </Tag>
    ) : undefined,
    nodeData: {
      entry: node.valid_entrypoint,
      isFolder: node.is_folder,
    },
    label: <span>{node.path}</span>,
    isExpanded: open,
    icon: has_children ? "folder-open" : "document",
    hasCaret: false,
    childNodes: children,
  };
}

function App() {
  const [expansionSet, setExpansion] = React.useState(new Set() as Set<string>);
  const [analysisSet, setAnalysisSet] = React.useState(
    new Set() as Set<string>
  );
  const { state: applicationState, loading } = useApplicationState();
  const [tab, setTab] = useState("configuration");

  const [analysis] = useEntrypointAnalysis();
  const [selectedNode, setSelectedNode] = useState(null as any);

  const analysisTree = React.useMemo(
    () =>
      applicationState && analysis
        ? create_root_tree(applicationState, analysisSet, analysis.file_tree)
        : null,
    [applicationState, analysisSet, analysis]
  );

  const analysisSelectionSet = React.useMemo(() => {
    const analysisSelectionSet = new Set() as Set<string>;
    if (analysisTree != null) {
      // go over every single tree node in the tree and find only the visible nodes

      // @ts-ignore
      function traverseTree(node: ITreeNode) {
        if (node.isExpanded) {
          node.childNodes?.forEach((child) => {
            // @ts-ignore
            if (child.nodeData?.isFolder === false) {
              analysisSelectionSet.add(`${child.id}`);
            }
            traverseTree(child);
          });

        } else {
          analysisSelectionSet.add(`${node.id}`);
        }
      }
      analysisTree.forEach((node) => traverseTree(node));
    }

    return analysisSelectionSet;
  }, [analysisTree]);

  const analyze = useAnalyseEntrypoint();

  useEffect(() => {
    if (analysis != null) {
      setSelectedNode(analysis.all_nodes[0].full_path);
      // split all of the entrypoint paths and open up all of them recursively as the default selection set
      const splits = analysis.entrypoint.full_path.split("/");
      setAnalysisSet(() => {
        let currentPath = "/";
        const pathSet = new Set(["/"]) as Set<string>;
        for (const split of splits) {
          if (split === "") {
            continue;
          }
          currentPath += `${split}`;
          pathSet.add(currentPath);
          currentPath += "/";
        }
        return pathSet;
      });
    }
  }, [analysis]);

  const rootTree = React.useMemo(() => {
    if (applicationState && applicationState.root_tree)
      return create_root_tree(
        applicationState,
        expansionSet,
        applicationState.root_tree
      );
  }, [expansionSet, applicationState]);

  return (
    <div style={{ display: "flex", height: "100vh", position: "relative" }}>
      <div
        style={{
          width: 400,
          padding: 20,
          flexShrink: 0,
          height: "100%",
          overflow: "scroll",
          backgroundColor: Colors.LIGHT_GRAY5,
        }}
      >
        <Tabs
          defaultSelectedTabId="configuration"
          onChange={(id) => setTab(id as string)}
        >
          <Tab id="project" disabled={!rootTree}>
            Project
          </Tab>
          <Tab id="analysis" disabled={!analysis}>
            Analysis
          </Tab>
          <Tabs.Expander />
          <Tab id="configuration">
            <Icon icon="settings" size={16} />
          </Tab>
        </Tabs>

        <div style={{ paddingTop: 20 }}>
          {rootTree && tab === "project" && (
            <>
              <Tree
                onNodeClick={(node) => {
                  // @ts-ignore
                  if (node.nodeData?.entry) {
                    analyze(node.id);
                    return;
                  }

                  if (expansionSet.has(`${node.id}`)) {
                    expansionSet.delete(`${node.id}`);
                  } else {
                    expansionSet.add(`${node.id}`);
                  }
                  setExpansion(new Set(expansionSet));
                }}
                contents={rootTree}
              />
            </>
          )}
          {tab === "analysis" && analysisTree != null && (
            <Tree
              onNodeClick={(node) => {
                // @ts-ignore
                if (node.childNodes?.length === 0 && node.nodeData?.isFolder === false) {
                  setSelectedNode(`${node.id}`)
                }
                if (analysisSet.has(`${node.id}`)) {
                  analysisSet.delete(`${node.id}`);
                } else {
                  analysisSet.add(`${node.id}`);
                }

                setAnalysisSet(new Set(analysisSet));
              }}
              contents={analysisTree}
            />
          )}

          {tab === "configuration" && <ConfigurationPanel />}
        </div>
      </div>
      <div
        style={{
          flexGrow: 1,
          overflow: "hidden",
          height: "100%",
        }}
      >
        <TreeMap selectedNode={selectedNode} selectedPaths={analysisSelectionSet} />
      </div>
    </div>
  );
}

export default App;
