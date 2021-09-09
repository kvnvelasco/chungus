import {
  ApplicationState,
  FileNode,
  RootTree,
  useAnalyseEntrypoint,
  useApplicationState,
  useEntrypointAnalysis,
} from "./state";
import React, { SetStateAction, useState } from "react";
import {
  Classes,
  Colors,
  Icon,
  ITreeNode,
  Tab,
  Tabs,
  Tag,
  Tree,
} from "@blueprintjs/core";
import { ConfigurationPanel } from "./Configuration";

type Props = {
  projectOpen: Set<string>;
  setProjectOpen: (s: SetStateAction<Set<string>>) => void;
  analysisOpen: Set<string>;
  setAnalysisOpen: (s: SetStateAction<Set<string>>) => void;
  setSelectedNode: (s: SetStateAction<string | null>) => void;
  analysisTree: ITreeNode[] | null;
};

export function Sidebar({
  projectOpen,
  setProjectOpen,
  analysisOpen,
  setAnalysisOpen,
  setSelectedNode,
  analysisTree,
}: Props) {
  const [analysis] = useEntrypointAnalysis();
  const analyze = useAnalyseEntrypoint();

  const [tab, setTab] = useState("configuration");
  const { state: applicationState } = useApplicationState();

  const rootTree = React.useMemo(() => {
    if (applicationState && applicationState.root_tree)
      return create_root_tree(
        applicationState,
        projectOpen,
        applicationState.root_tree
      );
  }, [projectOpen, applicationState]);

  return (
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
              onNodeClick={async (node) => {
                // @ts-ignore
                if (node.nodeData?.entry) {
                  setSelectedNode(null);
                  setAnalysisOpen(new Set());
                  analyze(node.id);
                  return;
                }

                if (projectOpen.has(`${node.id}`)) {
                  projectOpen.delete(`${node.id}`);
                } else {
                  projectOpen.add(`${node.id}`);
                }
                setProjectOpen(new Set(projectOpen));
              }}
              contents={rootTree}
            />
          </>
        )}
        {tab === "analysis" && analysisTree != null && (
          <Tree
            onNodeClick={(node) => {
              // @ts-ignore
              if (
                node.childNodes?.length === 0 &&
                // @ts-ignore
                node.nodeData?.isFolder === false
              ) {
                setSelectedNode(`${node.id}`);
              }
              if (analysisOpen.has(`${node.id}`)) {
                analysisOpen.delete(`${node.id}`);
              } else {
                analysisOpen.add(`${node.id}`);
              }

              setAnalysisOpen(new Set(analysisOpen));
            }}
            contents={analysisTree}
          />
        )}

        {tab === "configuration" && <ConfigurationPanel />}
      </div>
    </div>
  );
}

export function create_root_tree(
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
