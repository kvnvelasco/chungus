import { useEventSyncedState } from "./EventSyncedState";
import React, { createContext, useContext, useMemo } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { AppToaster } from "./Toaster";
import { Intent } from "@blueprintjs/core";

export type ApplicationState = {
  active_directory: string | null;
  active_entrypoint: string | null;
  extensions: string[];
  webpack_context: string | null;
  root_tree: RootTree | null;
};

export type RootTree = {
  file_node: FileNode;
  root_path: string;
};

export type FileNode = {
  path: string;
  full_path: string;
  is_folder: boolean;
  children: FileNode[];
  valid_entrypoint: boolean;
};

const Context = createContext<{
  state: ApplicationState | null;
  loading: boolean;
  set: any;
}>({ state: null, loading: false, set: () => {} });

export function ApplicationStateProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [applicationState, { loading, set }] =
    useEventSyncedState<ApplicationState>("application_state::sync");

  return (
    <Context.Provider value={{ state: applicationState, loading, set }}>
      {children}
    </Context.Provider>
  );
}

export function useApplicationState(): {
  state: ApplicationState | null;
  loading: boolean;
  set: () => {};
} {
  return useContext(Context);
}

export function useAnalyseEntrypoint() {
  return async (entrypoint: string | number) => {
    try {
      await invoke("create_entrypoint_analysis", { entrypoint });
    } catch (e) {
      AppToaster.show({
        message: "Unable to analyze entrypoint",
        intent: Intent.DANGER,
      });
    }
  };
}

export type Analysis = {
  node_map: Record<string, AnalysisNode>;
  file_tree: RootTree;
  analysis_groups: AnalysisNode[];
  all_nodes: AnalysisNode[];
  entrypoint: AnalysisNode;
  chunks: {};
};

export type AnalysisNode = {
  identifier: string,
  full_path: string;
  stem: string;
  chunk: number;
  incoming: AnalysisNode[];
  outgoing: AnalysisNode[];
};

type RawAnalysis = {
  node_map: Record<string, number>;
  all_nodes: RawAnalysisNode[];
  analysis_groups: RawAnalysisNode[];
  entrypoint: RawAnalysisNode;
};

export type RawAnalysisNode = {
  full_path: string;
  stem: string;
  incoming: number[];
  outgoing: number[];
};

export function useEntrypointAnalysis(): [
  Analysis | null,
  { loading: boolean }
] {
  const [state, { loading }] = useEventSyncedState<RawAnalysis>(
    "entrypoint_analysis::sync"
  );

  // do some preprocessing to link all the things together
  // Warning mutable stuff going on to form cylclic dependencies
  const output = useMemo((): Analysis | null => {
    const oldState = state;
    if (state != null) {
      const state = { ...oldState };
      // @ts-ignore
      for (let node of state.all_nodes) {
        // @ts-ignore
        node.incoming = node.incoming.map((idx) => state.all_nodes[idx] || idx);

        // @ts-ignore
        node.outgoing = node.outgoing.map((idx) => state.all_nodes[idx] || idx);
      }

      // @ts-ignore
      for (let node_group of state.analysis_groups) {
        // @ts-ignore
        node_group.incoming = node_group.incoming.map(
          // @ts-ignore
          (idx) => state.all_nodes[idx] || idx
        );
        // @ts-ignore
        node_group.outgoing = node_group.outgoing.map(
          // @ts-ignore
          (idx) => state.all_nodes[idx] || idx
        );
        // @ts-ignore
        node_group.inclusions = node_group.inclusions.map(
          // @ts-ignore
          (idx) => state.all_nodes[idx] || idx
        );

        // @ts-ignore
        node_group.immediate_children = node_group.immediate_children.map(
          // @ts-ignore
          (idx) => state.all_nodes[idx] || idx
        );
      }

      // @ts-ignore
      state.entrypoint.incoming = state.entrypoint.incoming.map(
        // @ts-ignore
        (idx) => state.all_nodes[idx] || idx
      );

      // @ts-ignore
      state.entrypoint.outgoing = state.entrypoint.outgoing.map(
        // @ts-ignore
        (idx) => state.all_nodes[idx] || idx
      );
      // @ts-ignore

      // sort the things
      state.analysis_groups.sort((a, b) => a.depth - b.depth);

      return state as any;

    }

    return null;
  }, [state]);

  return [output, { loading }];
}
