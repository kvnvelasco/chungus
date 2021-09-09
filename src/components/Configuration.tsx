import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import {
  Classes,
  FileInput,
  FormGroup,
  InputGroup,
  Intent,
  Spinner,
  Text,
} from "@blueprintjs/core";
import { AppToaster } from "./Toaster";
import { listen } from "@tauri-apps/api/event";
import { useApplicationState } from "./state";

export function ConfigurationPanel() {
  const [loading, setLoading] = useState(false);
  const { state: applicationState, loading: appStateLoading } =
    useApplicationState();

  const load = React.useCallback(async () => {
    try {
      setLoading(true);
      await invoke("create_root_tree");
      setLoading(false);
    } catch (e) {
      setLoading(false);
      AppToaster.show({ message: <>{e}</>, intent: Intent.DANGER });
    }
  }, []);

  return (
    <>
      {loading || appStateLoading ? (
        <Spinner />
      ) : (
        <>
          <FormGroup
            label="File extensions"
            helperText="Input comma separated values to identify valid javascript entrypoints / files"
          >
            <InputGroup value={applicationState?.extensions.join(", ")} />
          </FormGroup>
          <FormGroup label="Project directory">
            <FileInput
              fill
              text={applicationState?.active_directory ?? "Select a directory"}
              onClick={(e) => {
                e.preventDefault();
                load();
              }}
            />
          </FormGroup>
          <Text className={Classes.HEADING}> Webpack Specific Settings </Text>
          <FormGroup
            label="Webpack context"
            helperText="If you have specified a context in webpack.context as the resolve root, specify it here"
          >
            <InputGroup value={applicationState?.webpack_context ?? ""} />
          </FormGroup>

          <FormGroup
            label="Webpack stats file"
            helperText="To display chunk information, include a webpack stats file. Currently only officially supports webpack@4"
          >
            <FileInput
              fill
              text={applicationState?.active_directory ?? "Select a directory"}
              onClick={(e) => {
                e.preventDefault();
                invoke("create_webpack_statstics");
              }}
            />
          </FormGroup>
        </>
      )}
    </>
  );
}

ConfigurationPanel.backendSignal = listen.bind("configuration::open");
