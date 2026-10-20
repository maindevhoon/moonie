import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import {
  ArrowRight,
  Check,
  Circle,
  Clock3,
  ExternalLink,
  Focus,
  LayoutList,
  Moon,
  Plus,
  Settings2,
  Sparkles,
  Trash2,
  WandSparkles,
  X,
} from "lucide-react";
import "./App.css";

type Draft = {
  title: string;
  notes: string;
  dueAt: string | null;
  scheduledAt: string | null;
  estimateMin: number | null;
  dependencyIds: string[];
  recurrence: string;
  sourceUrl: string | null;
};
type Task = Draft & {
  id: string;
  status: string;
  createdAt: string;
  snoozedUntil: string | null;
};
type Suggestion = { task: Task; reason: string };
type Model = { id: string; name: string };
type FocusStatus = { seconds: number; active: boolean };
const emptyDraft = (): Draft => ({
  title: "",
  notes: "",
  dueAt: null,
  scheduledAt: null,
  estimateMin: null,
  dependencyIds: [],
  recurrence: "none",
  sourceUrl: null,
});
const dateInput = (value: string | null) =>
  value
    ? new Date(value).toLocaleString("sv-SE").replace(" ", "T").slice(0, 16)
    : "";
const iso = (value: string) => (value ? new Date(value).toISOString() : null);
const dateLabel = (value: string | null) =>
  value
    ? new Date(value).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      })
    : "";
const errorText = (e: unknown) =>
  typeof e === "string"
    ? e
    : e instanceof Error
      ? e.message
      : "Something went wrong";
const windowLabel = getCurrentWindow().label;
document.documentElement.dataset.window = windowLabel;
const defaultShortcut = navigator.platform.toLowerCase().includes("mac")
  ? "Command+Shift+Space"
  : "Control+Shift+Space";
const openSource = (
  event: React.MouseEvent<HTMLAnchorElement>,
  url: string,
) => {
  event.preventDefault();
  openUrl(url).catch(() => {});
};

function MascotWindow() {
  const [state, setState] = useState("idle");
  const [prefs, setPrefs] = useState<Record<string, string>>({});
  const [visible, setVisible] = useState(!document.hidden);
  useEffect(() => {
    let offs: (() => void)[] = [];
    let disposed = false;
    let transition: ReturnType<typeof setTimeout> | undefined;
    const refresh = () =>
      invoke<Record<string, string>>("get_settings")
        .then(setPrefs)
        .catch(() => {});
    const visibility = () => setVisible(!document.hidden);
    refresh();
    document.addEventListener("visibilitychange", visibility);
    Promise.all([
      listen<string>("mascot-state", (event) => {
        clearTimeout(transition);
        setState(event.payload);
        if (!["thinking", "focused", "resting"].includes(event.payload)) {
          transition = setTimeout(() => setState("idle"), 3200);
        }
      }),
      listen("settings-changed", refresh),
    ]).then((fns) => {
      if (disposed) fns.forEach((fn) => fn());
      else offs = fns;
    });
    return () => {
      disposed = true;
      clearTimeout(transition);
      document.removeEventListener("visibilitychange", visibility);
      offs.forEach((fn) => fn());
    };
  }, []);
  return (
    <div
      className="mascot-window"
      data-state={state}
      data-paused={
        !visible ||
        prefs.animationPaused === "true" ||
        prefs.reducedMotion === "true"
      }
    >
      <div
        className="drag-handle"
        data-tauri-drag-region
        aria-label="Drag Moonie"
      />
      <button
        className="mascot-click"
        onClick={() => invoke("show_panel")}
        title="Open Moonie"
      >
        <img
          src="/moonie.png"
          alt="Moonie, a cream cat with a golden crescent"
        />
      </button>
      <span className="mascot-caption">
        {state === "thinking"
          ? "Thinking…"
          : state === "celebration"
            ? "Well done! ✨"
            : state === "reminder"
              ? "A gentle nudge"
              : "Here with you ✦"}
      </span>
    </div>
  );
}

function TaskEditor({
  draft,
  onChange,
  compact = false,
  tasks = [],
  currentId,
}: {
  draft: Draft;
  onChange: (d: Draft) => void;
  compact?: boolean;
  tasks?: Task[];
  currentId?: string;
}) {
  const set = (part: Partial<Draft>) => onChange({ ...draft, ...part });
  return (
    <div className={`task-editor ${compact ? "compact" : ""}`}>
      <input
        aria-label="Task title"
        placeholder="What needs doing?"
        value={draft.title}
        maxLength={160}
        onChange={(e) => set({ title: e.target.value })}
      />
      <textarea
        aria-label="Task notes"
        placeholder="Details that help you get started"
        rows={compact ? 2 : 3}
        value={draft.notes}
        onChange={(e) => set({ notes: e.target.value })}
      />
      <div className="editor-fields">
        <label>
          Due{" "}
          <input
            type="datetime-local"
            value={dateInput(draft.dueAt)}
            onChange={(e) => set({ dueAt: iso(e.target.value) })}
          />
        </label>
        <label>
          Schedule{" "}
          <input
            type="datetime-local"
            value={dateInput(draft.scheduledAt)}
            onChange={(e) => set({ scheduledAt: iso(e.target.value) })}
          />
        </label>
        <label>
          Minutes{" "}
          <input
            type="number"
            min="1"
            max="1440"
            value={draft.estimateMin ?? ""}
            onChange={(e) =>
              set({
                estimateMin: e.target.value ? Number(e.target.value) : null,
              })
            }
          />
        </label>
        <label>
          Repeat{" "}
          <select
            value={draft.recurrence}
            onChange={(e) => set({ recurrence: e.target.value })}
          >
            <option value="none">Never</option>
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
          </select>
        </label>
      </div>
      {tasks.length > 0 && (
        <div className="dependency-field">
          <span>After these tasks</span>
          {tasks
            .filter((t) => t.id !== currentId && t.status === "open")
            .map((t) => (
              <label key={t.id}>
                <input
                  type="checkbox"
                  checked={draft.dependencyIds.includes(t.id)}
                  onChange={(e) =>
                    set({
                      dependencyIds: e.target.checked
                        ? [...draft.dependencyIds, t.id]
                        : draft.dependencyIds.filter((id) => id !== t.id),
                    })
                  }
                />
                {t.title}
              </label>
            ))}
        </div>
      )}
      {!draft.dueAt && !draft.scheduledAt && (
        <small className="date-hint">
          No date set — review if this task has a deadline.
        </small>
      )}
      {draft.sourceUrl && (
        <a
          className="source-link"
          href={draft.sourceUrl}
          onClick={(e) => openSource(e, draft.sourceUrl!)}
        >
          <ExternalLink size={14} /> Source webpage
        </a>
      )}
    </div>
  );
}

function MainWindow() {
  const [view, setView] = useState<"today" | "all" | "settings">("today");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [suggestion, setSuggestion] = useState<Suggestion | null>(null);
  const [thought, setThought] = useState("");
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [draftBatchId, setDraftBatchId] = useState("");
  const [activeRequestId, setActiveRequestId] = useState("");
  const [summary, setSummary] = useState("");
  const [manual, setManual] = useState(false);
  const [manualDraft, setManualDraft] = useState<Draft>(emptyDraft());
  const [editing, setEditing] = useState<string | null>(null);
  const [editDraft, setEditDraft] = useState<Draft>(emptyDraft());
  const [busy, setBusy] = useState(false);
  const actionLock = useRef(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState("");
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [models, setModels] = useState<Model[]>([]);
  const [keys, setKeys] = useState({ openrouter: "", tinyfish: "" });
  const [shortcutInput, setShortcutInput] = useState(defaultShortcut);
  const [keyStatus, setKeyStatus] = useState({
    openrouter: false,
    tinyfish: false,
  });
  const [notificationAllowed, setNotificationAllowed] = useState(false);
  const [available, setAvailable] = useState("");
  const [focusSeconds, setFocusSeconds] = useState(0);
  const [focusRunning, setFocusRunning] = useState(false);
  const openTasks = useMemo(
    () => tasks.filter((t) => t.status === "open"),
    [tasks],
  );
  const todayTasks = useMemo(
    () =>
      openTasks.filter(
        (t) =>
          !t.scheduledAt ||
          new Date(t.scheduledAt).getTime() <= Date.now() + 86400000,
      ),
    [openTasks],
  );
  const completed = tasks.filter((t) => t.status === "done").length;
  const refresh = async (minutes = available) => {
    const [items, next] = await Promise.all([
      invoke<Task[]>("list_tasks"),
      invoke<Suggestion | null>("next_task", {
        availableMin: minutes ? Number(minutes) : null,
      }),
    ]);
    setTasks(items);
    setSuggestion(next);
  };
  const refreshSettings = async () => {
    const [prefs, openrouter, tinyfish] = await Promise.all([
      invoke<Record<string, string>>("get_settings"),
      invoke<boolean>("has_api_key", { provider: "openrouter" }),
      invoke<boolean>("has_api_key", { provider: "tinyfish" }),
    ]);
    setSettings(prefs);
    setKeyStatus({ openrouter, tinyfish });
  };
  useEffect(() => {
    refresh().catch((e) => setError(errorText(e)));
    refreshSettings().catch((e) => setError(errorText(e)));
    isPermissionGranted()
      .then(setNotificationAllowed)
      .catch(() => {});
    invoke<FocusStatus>("get_focus")
      .then((f) => {
        setFocusSeconds(f.seconds);
        setFocusRunning(f.active);
      })
      .catch(() => {});
    let offs: (() => void)[] = [];
    let disposed = false;
    Promise.all([
      listen("tasks-changed", () => refresh().catch(() => {})),
      listen<string>("request-progress", (e) => setProgress(e.payload)),
      listen("settings-changed", () => refreshSettings().catch(() => {})),
    ]).then((fns) => {
      if (disposed) fns.forEach((fn) => fn());
      else offs = fns;
    });
    return () => {
      disposed = true;
      offs.forEach((fn) => fn());
    };
  }, []);
  useEffect(
    () => setShortcutInput(settings.shortcut || defaultShortcut),
    [settings.shortcut],
  );
  useEffect(() => {
    if (!focusRunning) return;
    const timer = setInterval(() => setFocusSeconds((s) => s + 1), 1000);
    return () => clearInterval(timer);
  }, [focusRunning]);
  const run = async (fn: () => Promise<void>) => {
    if (actionLock.current) return;
    actionLock.current = true;
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(errorText(e));
    } finally {
      actionLock.current = false;
      setBusy(false);
      setProgress("");
    }
  };
  const capture = () => {
    if (actionLock.current) return;
    const requestId = crypto.randomUUID();
    setActiveRequestId(requestId);
    return run(async () => {
      try {
        const match = thought.trim().match(/^https?:\/\/\S+$/);
        const response = await invoke<{
          drafts: Draft[];
          sourceSummary: string | null;
        }>("create_draft", {
          request: {
            requestId,
            text: match ? "" : thought,
            sourceUrl: match ? match[0] : null,
          },
        });
        setDrafts(response.drafts);
        setDraftBatchId(crypto.randomUUID());
        setSummary(response.sourceSummary ?? "");
      } finally {
        setActiveRequestId("");
      }
    });
  };
  const cancelCapture = () =>
    invoke("cancel_draft", { requestId: activeRequestId }).catch((e) =>
      setError(errorText(e)),
    );
  const approve = () =>
    run(async () => {
      const valid = drafts.filter((d) => d.title.trim());
      if (!valid.length) throw new Error("Add a title before approving.");
      await invoke("approve_drafts", { drafts: valid, batchId: draftBatchId });
      setDrafts([]);
      setThought("");
      setSummary("");
      await refresh();
    });
  const saveManual = () =>
    run(async () => {
      await invoke("save_manual_task", { draft: manualDraft });
      setManual(false);
      setManualDraft(emptyDraft());
      await refresh();
    });
  const update = (task: Task, patch?: Partial<Task>) =>
    run(async () => {
      const data = patch ? { ...task, ...patch } : { ...task, ...editDraft };
      await invoke("update_task", {
        id: task.id,
        draft: {
          title: data.title,
          notes: data.notes,
          dueAt: data.dueAt,
          scheduledAt: data.scheduledAt,
          estimateMin: data.estimateMin,
          dependencyIds: data.dependencyIds,
          recurrence: data.recurrence,
          sourceUrl: data.sourceUrl,
        },
        status: data.status,
        snoozedUntil: data.snoozedUntil,
      });
      setEditing(null);
      await refresh();
    });
  const deleteTask = (task: Task) => {
    if (!window.confirm(`Delete “${task.title}”?`)) return;
    run(async () => {
      await invoke("delete_task", { id: task.id });
      await refresh();
    });
  };
  const saveKey = (provider: "openrouter" | "tinyfish") =>
    run(async () => {
      await invoke("set_api_key", { provider, key: keys[provider] });
      setKeys((k) => ({ ...k, [provider]: "" }));
      await refreshSettings();
    });
  const loadModels = () =>
    run(async () => {
      setModels(await invoke<Model[]>("get_models"));
    });
  const setSetting = (key: string, value: string) =>
    run(async () => {
      await invoke("set_setting", { key, value });
      setSettings((s) => ({ ...s, [key]: value }));
    });
  const focusAction = (action: "start" | "pause" | "reset" | "break") =>
    invoke<FocusStatus>("focus_action", { action })
      .then((f) => {
        setFocusSeconds(f.seconds);
        setFocusRunning(f.active);
      })
      .catch((e) => setError(errorText(e)));
  const startEdit = (task: Task) => {
    setEditing(task.id);
    setEditDraft({
      title: task.title,
      notes: task.notes,
      dueAt: task.dueAt,
      scheduledAt: task.scheduledAt,
      estimateMin: task.estimateMin,
      dependencyIds: task.dependencyIds,
      recurrence: task.recurrence,
      sourceUrl: task.sourceUrl,
    });
  };
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">☾</span>
          <div>
            <strong>Moonie</strong>
            <small>your calm companion</small>
          </div>
        </div>
        <div className="side-label">WORKSPACE</div>
        <nav>
          <button
            className={view === "today" ? "active" : ""}
            onClick={() => setView("today")}
          >
            <Sparkles size={18} /> Today <span>{openTasks.length}</span>
          </button>
          <button
            className={view === "all" ? "active" : ""}
            onClick={() => setView("all")}
          >
            <LayoutList size={18} /> All tasks
          </button>
          <button
            className={view === "settings" ? "active" : ""}
            onClick={() => setView("settings")}
          >
            <Settings2 size={18} /> Settings
          </button>
        </nav>
        <div className="sidebar-bottom">
          <img src="/moonie.png" alt="" />
          <p>
            Small steps count.
            <br />
            <span>Moonie is here with you.</span>
          </p>
        </div>
      </aside>
      <main className="workspace">
        <header className="topbar">
          <span className="eyebrow">MOONIE / {view.toUpperCase()}</span>
          <span className="top-date">
            {new Date().toLocaleDateString(undefined, {
              weekday: "long",
              month: "long",
              day: "numeric",
            })}
          </span>
        </header>
        {error && (
          <div className="error" role="alert">
            {error}
            <button onClick={() => setError("")} aria-label="Dismiss">
              <X size={16} />
            </button>
          </div>
        )}
        {view === "settings" ? (
          <div className="settings-page">
            <div className="page-heading">
              <span className="eyebrow gold">PERSONAL SPACE</span>
              <h1>Make Moonie yours.</h1>
              <p>Your keys stay in your computer's credential store.</p>
            </div>
            <div className="settings-grid">
              <section className="panel">
                <h2>AI connections</h2>
                <p>
                  Connect OpenRouter to turn thoughts into drafts, and TinyFish
                  to read a URL you paste.
                </p>
                {(["openrouter", "tinyfish"] as const).map((provider) => (
                  <div className="key-row" key={provider}>
                    <div>
                      <strong>
                        {provider === "openrouter" ? "OpenRouter" : "TinyFish"}
                      </strong>
                      <small>
                        {keyStatus[provider] ? "Connected" : "Key needed"}
                      </small>
                    </div>
                    <input
                      type="password"
                      placeholder="Paste API key"
                      value={keys[provider]}
                      onChange={(e) =>
                        setKeys((k) => ({ ...k, [provider]: e.target.value }))
                      }
                    />
                    <button
                      className="secondary"
                      onClick={() => saveKey(provider)}
                      disabled={busy || !keys[provider]}
                    >
                      Save
                    </button>
                  </div>
                ))}
                <div className="setting-field">
                  <label>OpenRouter model</label>
                  <div className="inline">
                    <select
                      value={settings.model ?? ""}
                      onChange={(e) => setSetting("model", e.target.value)}
                    >
                      <option value="">Select a model</option>
                      {settings.model &&
                        !models.some((m) => m.id === settings.model) && (
                          <option value={settings.model}>
                            {settings.model}
                          </option>
                        )}
                      {models.map((m) => (
                        <option key={m.id} value={m.id}>
                          {m.name}
                        </option>
                      ))}
                    </select>
                    <button
                      className="secondary"
                      onClick={loadModels}
                      disabled={busy || !keyStatus.openrouter}
                    >
                      Load models
                    </button>
                  </div>
                </div>
              </section>
              <section className="panel">
                <h2>Companion</h2>
                <p>A gentle presence that follows your pace.</p>
                <label className="toggle">
                  <span>Show floating Moonie</span>
                  <input
                    type="checkbox"
                    defaultChecked
                    onChange={(e) =>
                      invoke("set_mascot_visible", {
                        visible: e.target.checked,
                      }).catch((err) => setError(errorText(err)))
                    }
                  />
                </label>
                <div className="permission-row">
                  <span>
                    Reminders{" "}
                    {notificationAllowed ? "enabled" : "need permission"}
                  </span>
                  <button
                    className="secondary"
                    onClick={() =>
                      requestPermission()
                        .then((p) => setNotificationAllowed(p === "granted"))
                        .catch((e) => setError(errorText(e)))
                    }
                    disabled={notificationAllowed}
                  >
                    {notificationAllowed ? "Enabled" : "Enable"}
                  </button>
                </div>
                <label className="toggle">
                  <span>Quiet mode</span>
                  <input
                    type="checkbox"
                    checked={settings.quiet === "true"}
                    onChange={(e) =>
                      setSetting("quiet", String(e.target.checked))
                    }
                  />
                </label>
                <label className="toggle">
                  <span>Reduced motion</span>
                  <input
                    type="checkbox"
                    checked={settings.reducedMotion === "true"}
                    onChange={(e) =>
                      setSetting("reducedMotion", String(e.target.checked))
                    }
                  />
                </label>
                <label className="toggle">
                  <span>Pause animation</span>
                  <input
                    type="checkbox"
                    checked={settings.animationPaused === "true"}
                    onChange={(e) =>
                      setSetting("animationPaused", String(e.target.checked))
                    }
                  />
                </label>
                <div className="setting-field">
                  <label>Mascot size</label>
                  <select
                    value={settings.mascotSize ?? "medium"}
                    onChange={(e) => {
                      setSetting("mascotSize", e.target.value);
                      invoke("set_mascot_size", { size: e.target.value }).catch(
                        (err) => setError(errorText(err)),
                      );
                    }}
                  >
                    <option value="small">Small</option>
                    <option value="medium">Medium</option>
                    <option value="large">Large</option>
                  </select>
                </div>
                <div className="setting-field">
                  <label>Open shortcut</label>
                  <div className="inline">
                    <input
                      aria-label="Open shortcut"
                      value={shortcutInput}
                      onChange={(e) => setShortcutInput(e.target.value)}
                      placeholder="Command+Shift+Space"
                    />
                    <button
                      className="secondary"
                      onClick={() =>
                        run(async () => {
                          await invoke("set_shortcut", {
                            shortcut: shortcutInput,
                          });
                          await refreshSettings();
                        })
                      }
                    >
                      Save
                    </button>
                  </div>
                </div>
                <button className="text-button" onClick={() => invoke("quit")}>
                  Quit Moonie
                </button>
              </section>
            </div>
          </div>
        ) : (
          <>
            <div className="page-heading">
              <span className="eyebrow gold">
                {view === "today"
                  ? "A LITTLE CLARITY FOR TODAY"
                  : "EVERYTHING IN ONE PLACE"}
              </span>
              <h1>
                {view === "today"
                  ? "One thing at a time."
                  : "Your tasks, your pace."}
              </h1>
              <p>
                {view === "today"
                  ? "Capture what’s on your mind. Moonie will help find the next step."
                  : `${openTasks.length} open · ${completed} complete`}
              </p>
            </div>
            <section className="capture-card">
              <div className="capture-icon">
                <WandSparkles size={24} />
              </div>
              <div className="capture-body">
                <h2>What’s on your mind?</h2>
                <p>
                  Drop a thought or paste a public webpage URL. Moonie will
                  prepare tasks for you to review.
                </p>
                <textarea
                  value={thought}
                  onChange={(e) => setThought(e.target.value)}
                  placeholder="I need to prepare for Friday’s interview…"
                  rows={3}
                />
                <div className="capture-actions">
                  <span>
                    {progress || "Nothing is saved until you approve it."}
                  </span>
                  <div>
                    {activeRequestId && (
                      <button className="secondary" onClick={cancelCapture}>
                        Cancel
                      </button>
                    )}
                    <button
                      className="secondary"
                      onClick={() => setManual(!manual)}
                    >
                      <Plus size={17} /> Add manually
                    </button>
                    <button
                      className="primary"
                      onClick={capture}
                      disabled={busy || !thought.trim()}
                    >
                      {busy ? "Working…" : "Make a plan"}
                      <ArrowRight size={17} />
                    </button>
                  </div>
                </div>
              </div>
            </section>
            {manual && (
              <section className="panel manual-panel">
                <div className="section-heading">
                  <h2>New task</h2>
                  <button
                    className="icon-button"
                    onClick={() => setManual(false)}
                  >
                    <X size={18} />
                  </button>
                </div>
                <TaskEditor
                  draft={manualDraft}
                  onChange={setManualDraft}
                  tasks={tasks}
                />
                <div className="right">
                  <button
                    className="primary"
                    disabled={busy || !manualDraft.title.trim()}
                    onClick={saveManual}
                  >
                    Save task
                  </button>
                </div>
              </section>
            )}
            {drafts.length > 0 && (
              <section className="drafts-section">
                <div className="section-heading">
                  <div>
                    <span className="eyebrow gold">REVIEW BEFORE SAVING</span>
                    <h2>Moonie’s suggested tasks</h2>
                  </div>
                  <button className="text-button" onClick={() => setDrafts([])}>
                    Discard drafts
                  </button>
                </div>
                {summary && <p className="summary">{summary}</p>}
                {drafts.map((draft, index) => (
                  <div className="draft-card" key={index}>
                    <div className="draft-number">
                      {String(index + 1).padStart(2, "0")}
                    </div>
                    <TaskEditor
                      draft={draft}
                      onChange={(value) =>
                        setDrafts((items) =>
                          items.map((d, i) => (i === index ? value : d)),
                        )
                      }
                      tasks={tasks}
                      compact
                    />
                    <button
                      className="icon-button"
                      onClick={() =>
                        setDrafts((items) =>
                          items.filter((_, i) => i !== index),
                        )
                      }
                      aria-label="Remove draft"
                    >
                      <X size={17} />
                    </button>
                  </div>
                ))}
                <div className="right">
                  <button className="primary" onClick={approve} disabled={busy}>
                    Approve {drafts.length}{" "}
                    {drafts.length === 1 ? "task" : "tasks"} <Check size={17} />
                  </button>
                </div>
              </section>
            )}
            {view === "today" && (
              <div className="dashboard-grid">
                <section className="next-card">
                  <span className="eyebrow">YOUR NEXT STEP</span>
                  {suggestion ? (
                    <>
                      <div className="next-main">
                        <span className="next-icon">
                          <Sparkles size={24} />
                        </span>
                        <h2>{suggestion.task.title}</h2>
                        <p>
                          {suggestion.reason}
                          {suggestion.task.estimateMin
                            ? ` · ${suggestion.task.estimateMin} min`
                            : ""}
                        </p>
                      </div>
                      <button
                        onClick={() =>
                          update(suggestion.task, { status: "done" })
                        }
                      >
                        Mark complete <ArrowRight size={16} />
                      </button>
                    </>
                  ) : (
                    <div className="empty-next">
                      <h2>A clear moment.</h2>
                      <p>
                        Add a task when you’re ready, and Moonie will suggest a
                        place to start.
                      </p>
                    </div>
                  )}
                </section>
                <section className="focus-card">
                  <div className="focus-heading">
                    <span className="eyebrow">FOCUS SPACE</span>
                    <Focus size={20} />
                  </div>
                  <h2>
                    {focusRunning ? "You’ve got this." : "Find your rhythm."}
                  </h2>
                  <div className="timer">
                    {String(Math.floor(focusSeconds / 60)).padStart(2, "0")}:
                    {String(focusSeconds % 60).padStart(2, "0")}
                  </div>
                  <button
                    className="secondary"
                    onClick={() =>
                      focusAction(focusRunning ? "pause" : "start")
                    }
                  >
                    {focusRunning ? "Pause focus" : "Start focus"}
                  </button>
                  <button
                    className="text-button"
                    onClick={() => focusAction("break")}
                  >
                    Break
                  </button>
                  <button
                    className="text-button"
                    onClick={() => focusAction("reset")}
                  >
                    Reset
                  </button>
                </section>
              </div>
            )}
            <section className="tasks-section">
              <div className="section-heading">
                <div>
                  <span className="eyebrow gold">THE LITTLE STEPS</span>
                  <h2>{view === "today" ? "On your list" : "All tasks"}</h2>
                </div>
                <div className="available">
                  <Clock3 size={16} />
                  <select
                    value={available}
                    onChange={(e) => {
                      setAvailable(e.target.value);
                      refresh(e.target.value).catch((err) =>
                        setError(errorText(err)),
                      );
                    }}
                  >
                    <option value="">Any time</option>
                    <option value="15">15 min free</option>
                    <option value="30">30 min free</option>
                    <option value="60">1 hour free</option>
                  </select>
                </div>
              </div>
              {(view === "today" ? todayTasks : tasks).length === 0 ? (
                <div className="empty-list">
                  <Moon size={30} />
                  <h3>All quiet for now.</h3>
                  <p>Add your first task above to begin.</p>
                </div>
              ) : (
                <div className="task-list">
                  {(view === "today" ? todayTasks : tasks).map((task) => (
                    <div
                      className={`task-row ${task.status === "done" ? "completed" : ""}`}
                      key={task.id}
                    >
                      <button
                        className="check-button"
                        onClick={() =>
                          update(task, {
                            status: task.status === "done" ? "open" : "done",
                          })
                        }
                        aria-label={
                          task.status === "done"
                            ? "Reopen task"
                            : "Complete task"
                        }
                      >
                        {task.status === "done" ? (
                          <Check size={17} />
                        ) : (
                          <Circle size={19} />
                        )}
                      </button>
                      <div className="task-info">
                        {editing === task.id ? (
                          <>
                            <TaskEditor
                              draft={editDraft}
                              onChange={setEditDraft}
                              tasks={tasks}
                              currentId={task.id}
                              compact
                            />
                            <div className="edit-actions">
                              <button
                                className="primary"
                                onClick={() => update(task)}
                                disabled={busy}
                              >
                                Save changes
                              </button>
                              <button
                                className="secondary"
                                onClick={() => setEditing(null)}
                              >
                                Cancel
                              </button>
                            </div>
                          </>
                        ) : (
                          <>
                            <button
                              className="task-title"
                              onClick={() => startEdit(task)}
                            >
                              {task.title}
                            </button>
                            {task.notes && <p>{task.notes}</p>}
                            <div className="task-meta">
                              {task.dueAt && (
                                <span>
                                  <Clock3 size={13} /> Due{" "}
                                  {dateLabel(task.dueAt)}
                                </span>
                              )}
                              {task.estimateMin && (
                                <span>{task.estimateMin} min</span>
                              )}
                              {task.dependencyIds.length > 0 && (
                                <span>
                                  {task.dependencyIds.length} prerequisite
                                  {task.dependencyIds.length > 1 ? "s" : ""}
                                </span>
                              )}
                              {task.recurrence !== "none" && (
                                <span>{task.recurrence}</span>
                              )}
                              {task.sourceUrl && (
                                <a
                                  href={task.sourceUrl}
                                  onClick={(e) =>
                                    openSource(e, task.sourceUrl!)
                                  }
                                >
                                  <ExternalLink size={13} /> Source
                                </a>
                              )}
                            </div>
                          </>
                        )}
                      </div>
                      <div className="row-actions">
                        <button
                          className="icon-button"
                          onClick={() =>
                            update(task, {
                              snoozedUntil: new Date(
                                Date.now() + 3600000,
                              ).toISOString(),
                            })
                          }
                          title="Snooze 1 hour"
                        >
                          <Clock3 size={17} />
                        </button>
                        <button
                          className="icon-button"
                          onClick={() => deleteTask(task)}
                          title="Delete"
                        >
                          <Trash2 size={17} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          </>
        )}
      </main>
    </div>
  );
}
export default function App() {
  return windowLabel === "mascot" ? <MascotWindow /> : <MainWindow />;
}
