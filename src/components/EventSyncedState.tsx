import { useEffect, useMemo, useRef, useState } from "react";
import { emit, listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/tauri";
import {AppToaster} from "./Toaster";
import {Intent} from "@blueprintjs/core";

const LISTENERS: Record<string, Promise<UnlistenFn>[]> = {};

export function useEventSyncedState<T extends {}>(
  sync_event: string
): [T | null, { set: (next: T) => Promise<void>; loading: boolean }] {
  const [loading, setLoading] = useState(false);
  const [state, setState] = useState(null as T | null);

  const [event_name, sync] = useMemo(
    () => sync_event.split("::"),
    [sync_event]
  );

  // initial sync
  useEffect(() => {
    const getter = `get_${event_name}`;
    setLoading(true)
    invoke(getter).then((value) => {
      requestAnimationFrame(() => setState(value as T))
      setLoading(false)
    }).catch((e) => {
      AppToaster.show({message: `Unable to synchronise ${event_name}`, intent: Intent.DANGER})
      setLoading(false)
    });
  }, [event_name]);

  useEffect(() => {
    const getter = `get_${event_name}`;

    function listener() {
      return listen(sync_event, async () => {
        try {
          setLoading(true)
          let value: T = await invoke(getter);
          requestAnimationFrame(() => setState(value))
          setLoading(false)
        } catch(e) {
          AppToaster.show({message: `Unable to synchronise ${event_name}`, intent: Intent.DANGER})
        }
      });
    }

    let promiseFn = listener();

    if (LISTENERS[sync_event]) {
      LISTENERS[sync_event].push(promiseFn);
    } else {
      LISTENERS[sync_event] = [promiseFn];
    }

    return () => {
      if (LISTENERS[sync_event]) {
        const listner = LISTENERS[sync_event].find(
          (prom) => prom === promiseFn
        );
        if (listner != null) {
          listner.then((fn) => fn());
        }
      }
    };
  }, [event_name, sync_event]);

  return [
    state,
    {
      set: async (value) => {
        const setter = `set_${event_name}`;
        await invoke(setter, value);
      },
      loading,
    },
  ];
}
