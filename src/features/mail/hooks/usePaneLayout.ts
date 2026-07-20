import { useCallback, useEffect, useLayoutEffect, useState } from "react";

const FOLDER_PANE_MIN = 220;
const FOLDER_PANE_MAX = 350;
const MESSAGE_PANE_MIN = 310;
const MESSAGE_PANE_MAX = 520;
const COLLAPSED_FOLDER_PANE_WIDTH = 72;
const READER_AND_DIVIDERS_MIN = 372;
const RESIZABLE_PANES_AVAILABLE_MIN = 500;

interface PaneLayoutState {
  folderPaneCollapsed: boolean;
  folderPaneWidth: number;
  messagePaneWidth: number;
  windowWidth: number;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function constrainPaneWidths(state: PaneLayoutState): PaneLayoutState {
  let folderPaneWidth = clamp(state.folderPaneWidth, FOLDER_PANE_MIN, FOLDER_PANE_MAX);
  let messagePaneWidth = clamp(state.messagePaneWidth, MESSAGE_PANE_MIN, MESSAGE_PANE_MAX);
  const visibleFolderWidth = state.folderPaneCollapsed
    ? COLLAPSED_FOLDER_PANE_WIDTH
    : folderPaneWidth;
  const available = Math.max(
    RESIZABLE_PANES_AVAILABLE_MIN,
    state.windowWidth - READER_AND_DIVIDERS_MIN,
  );
  let overflow = visibleFolderWidth + messagePaneWidth - available;
  if (overflow > 0) {
    const messageReduction = Math.min(overflow, messagePaneWidth - MESSAGE_PANE_MIN);
    messagePaneWidth -= messageReduction;
    overflow -= messageReduction;
  }
  if (overflow > 0 && !state.folderPaneCollapsed) {
    folderPaneWidth -= Math.min(overflow, folderPaneWidth - FOLDER_PANE_MIN);
  }
  return { ...state, folderPaneWidth, messagePaneWidth };
}

export function usePaneLayout(showSidebar: boolean) {
  const [layout, setLayout] = useState<PaneLayoutState>(() => constrainPaneWidths({
    folderPaneCollapsed: false,
    folderPaneWidth: 250,
    messagePaneWidth: 370,
    windowWidth: window.innerWidth,
  }));

  useEffect(() => {
    const handleResize = () => {
      setLayout((current) => constrainPaneWidths({
        ...current,
        windowWidth: window.innerWidth,
      }));
    };
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  const setFolderPaneWidth = useCallback((folderPaneWidth: number) => {
    setLayout((current) => {
      const max = Math.max(
        FOLDER_PANE_MIN,
        Math.min(FOLDER_PANE_MAX, current.windowWidth - current.messagePaneWidth - READER_AND_DIVIDERS_MIN),
      );
      return { ...current, folderPaneWidth: clamp(folderPaneWidth, FOLDER_PANE_MIN, max) };
    });
  }, []);

  const setMessagePaneWidth = useCallback((messagePaneWidth: number) => {
    setLayout((current) => {
      const visibleFolderWidth = current.folderPaneCollapsed
        ? COLLAPSED_FOLDER_PANE_WIDTH
        : current.folderPaneWidth;
      const max = Math.max(
        MESSAGE_PANE_MIN,
        Math.min(MESSAGE_PANE_MAX, current.windowWidth - visibleFolderWidth - READER_AND_DIVIDERS_MIN),
      );
      return { ...current, messagePaneWidth: clamp(messagePaneWidth, MESSAGE_PANE_MIN, max) };
    });
  }, []);

  const setFolderPaneCollapsed = useCallback((folderPaneCollapsed: boolean) => {
    setLayout((current) => constrainPaneWidths({ ...current, folderPaneCollapsed }));
  }, []);

  const visibleFolderWidth = layout.folderPaneCollapsed
    ? COLLAPSED_FOLDER_PANE_WIDTH
    : layout.folderPaneWidth;
  const titlebarSidebarWidth = showSidebar ? visibleFolderWidth : 0;
  const folderPaneMax = Math.max(
    FOLDER_PANE_MIN,
    Math.min(FOLDER_PANE_MAX, layout.windowWidth - layout.messagePaneWidth - READER_AND_DIVIDERS_MIN),
  );
  const messagePaneMax = Math.max(
    MESSAGE_PANE_MIN,
    Math.min(MESSAGE_PANE_MAX, layout.windowWidth - visibleFolderWidth - READER_AND_DIVIDERS_MIN),
  );

  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--shell-sidebar-width", `${titlebarSidebarWidth}px`);
    return () => {
      document.documentElement.style.removeProperty("--shell-sidebar-width");
    };
  }, [titlebarSidebarWidth]);

  return {
    folderPaneCollapsed: layout.folderPaneCollapsed,
    folderPaneMax,
    folderPaneWidth: layout.folderPaneWidth,
    messagePaneMax,
    messagePaneWidth: layout.messagePaneWidth,
    setFolderPaneCollapsed,
    setFolderPaneWidth,
    setMessagePaneWidth,
    visibleFolderWidth,
  };
}
