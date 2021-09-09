import React, { useEffect, useState } from "react";

import "./App.css";
import { Classes, ITreeNode, Spinner, Text } from "@blueprintjs/core";

import "@blueprintjs/core/lib/css/blueprint.css";
import {
  useApplicationState,
  useAppLoading,
  useEntrypointAnalysis,
} from "./components/state";
import { TreeMap } from "./components/D3TreeMap";
import { create_root_tree, Sidebar } from "./components/Sidebar";

function App() {
  const [expansionSet, setExpansion] = React.useState(new Set() as Set<string>);
  const [analysisSet, setAnalysisSet] = React.useState(
    new Set() as Set<string>
  );
  const loading = useAppLoading();
  const { state: applicationState } = useApplicationState();

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

  return (
    <div style={{ display: "flex", height: "100vh", position: "relative" }}>
      <Sidebar
        projectOpen={expansionSet}
        setProjectOpen={setExpansion}
        analysisOpen={analysisSet}
        setAnalysisOpen={setAnalysisSet}
        setSelectedNode={setSelectedNode}
        analysisTree={analysisTree}
      />
      <div
        style={{
          flexGrow: 1,
          overflow: "hidden",
          height: "100%",
        }}
      >
        <TreeMap
          selectedNode={selectedNode}
          selectedPaths={analysisSelectionSet}
        />
      </div>
      {loading.loading && (
        <div
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            backgroundColor: "rgba(255,255,255, 0.8)",
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
            flexDirection: "column",
          }}
        >
          <Spinner size={60} />
          <Text style={{ marginTop: 20 }} className={Classes.HEADING}>
            Loading
          </Text>
          <Text className={Classes.HEADING} style={{ textAlign: "center" }}>
            {loading.message}
          </Text>
        </div>
      )}
    </div>
  );
}

export default App;
