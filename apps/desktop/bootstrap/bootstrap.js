(() => {
  "use strict";

  const FIXED_APP_URL = "http://127.0.0.1:8721/app/";
  const POLL_DELAYS_MS = [180, 300, 500, 800, 1200];
  const isChinese = /^zh(?:-|$)/i.test(navigator.language || "");
  const copy = isChinese
    ? {
        eyebrow: "KANBAN TOOL / 本地优先",
        title: "正在连接本地工作区",
        lede: "桌面壳已就绪。打开看板前，我们会先检查固定 loopback host。",
        checking: "正在检查本地 host",
        ready: "本地 host 已就绪",
        recovery: "本地 host 需要处理",
        phaseLabel: "阶段",
        endpointLabel: "固定端点",
        diagnosticLabel: "诊断",
        retry: "重试探测现有 host",
        start: "启动本地 host",
        open: "在浏览器中打开",
        copy: "复制诊断信息",
        actionsLabel: "Bootstrap 操作",
        hint: "只使用固定的 127.0.0.1 URL，不加载远程内容。",
        copied: "诊断信息已复制。",
        copyFailed: "浏览器不允许复制诊断信息。",
        ipcMissing: "Tauri IPC 不可用；请从桌面应用启动此页面。",
        actionFailed: "操作失败",
        phases: {
          probingExisting: "探测现有 host",
          startingLocal: "启动本地 host",
          ready: "已就绪",
          recovery: "恢复模式",
        },
      }
    : {
        eyebrow: "KANBAN TOOL / LOCAL FIRST",
        title: "Connecting to your local workspace",
        lede: "The desktop shell is ready. We are checking the fixed loopback host before opening your board.",
        checking: "Checking local host",
        ready: "Local host is ready",
        recovery: "Local host needs attention",
        phaseLabel: "Phase",
        endpointLabel: "Fixed endpoint",
        diagnosticLabel: "Diagnostic",
        retry: "Retry probe existing host",
        start: "Start local host",
        open: "Open in browser",
        copy: "Copy diagnostics",
        actionsLabel: "Bootstrap actions",
        hint: "Only a fixed 127.0.0.1 URL is used. No remote content is loaded.",
        copied: "Diagnostics copied.",
        copyFailed: "The browser did not allow copying diagnostics.",
        ipcMissing: "Tauri IPC is unavailable; launch this page from the desktop app.",
        actionFailed: "Action failed",
        phases: {
          probingExisting: "Probe existing host",
          startingLocal: "Start local host",
          ready: "Ready",
          recovery: "Recovery",
        },
      };

  const elements = {
    card: document.getElementById("status-card"),
    dot: document.getElementById("status-dot"),
    status: document.getElementById("status-label"),
    phase: document.getElementById("phase"),
    endpoint: document.getElementById("endpoint"),
    diagnostic: document.getElementById("diagnostic"),
    feedback: document.getElementById("feedback"),
    retry: document.getElementById("retry"),
    start: document.getElementById("start"),
    open: document.getElementById("open"),
    copy: document.getElementById("copy"),
  };

  let latest = {
    phase: "startingLocal",
    endpoint: "127.0.0.1:8721",
    appUrl: FIXED_APP_URL,
    diagnostic: null,
    canOpenBrowser: false,
    generation: 0,
    inFlight: true,
    closed: false,
  };
  let pollTimer = null;
  let pollDelayIndex = 0;
  let lastSnapshotKey = "";

  document.documentElement.lang = isChinese ? "zh-CN" : "en";
  document.querySelectorAll("[data-copy]").forEach((node) => {
    const key = node.getAttribute("data-copy");
    if (key && copy[key]) node.textContent = copy[key];
  });
  document.querySelectorAll("[data-copy-aria-label]").forEach((node) => {
    const key = node.getAttribute("data-copy-aria-label");
    if (key && copy[key]) node.setAttribute("aria-label", copy[key]);
  });

  function invoke(command) {
    const api = window.__TAURI__;
    if (!api || !api.core || typeof api.core.invoke !== "function") {
      return Promise.reject(new Error(copy.ipcMissing));
    }
    return api.core.invoke(command);
  }

  function errorText(error) {
    if (error && typeof error === "object") {
      if (typeof error.message === "string") return error.message;
      if (typeof error.kind === "string") return error.kind;
    }
    if (typeof error === "string") return error;
    return copy.actionFailed;
  }

  function phaseText(phase) {
    return copy.phases[phase] || phase || copy.recovery;
  }

  function snapshotKey(snapshot) {
    return JSON.stringify({
      phase: snapshot.phase,
      endpoint: snapshot.endpoint,
      appUrl: snapshot.appUrl,
      diagnostic: snapshot.diagnostic,
      canOpenBrowser: snapshot.canOpenBrowser,
      generation: snapshot.generation,
      inFlight: snapshot.inFlight,
      closed: snapshot.closed,
    });
  }

  function render(snapshot) {
    if (!snapshot || typeof snapshot !== "object") return;
    const nextKey = snapshotKey(snapshot);
    const changed = nextKey !== lastSnapshotKey;
    if (changed) {
      pollDelayIndex = 0;
      lastSnapshotKey = nextKey;
    }
    latest = { ...latest, ...snapshot };
    if (!changed) {
      if (latest.inFlight && !latest.closed) schedulePoll();
      return;
    }
    const phase = latest.phase || "recovery";
    const ready = phase === "ready";
    const recovery = phase === "recovery";
    elements.card.setAttribute("aria-busy", latest.inFlight ? "true" : "false");
    elements.dot.classList.toggle("ready", ready);
    elements.dot.classList.toggle("recovery", recovery);
    elements.status.textContent = ready ? copy.ready : recovery ? copy.recovery : copy.checking;
    elements.phase.textContent = phaseText(phase);
    elements.endpoint.textContent = latest.endpoint || "127.0.0.1:8721";
    elements.diagnostic.textContent = latest.diagnostic?.message || "—";
    elements.retry.disabled = Boolean(latest.inFlight || latest.closed);
    elements.start.disabled = Boolean(latest.inFlight || latest.closed);
    elements.open.disabled = !latest.canOpenBrowser;
    if (latest.inFlight && !latest.closed) schedulePoll();
  }

  function schedulePoll() {
    if (pollTimer !== null || latest.closed || !latest.inFlight) return;
    const delay = POLL_DELAYS_MS[pollDelayIndex];
    pollTimer = window.setTimeout(async () => {
      pollTimer = null;
      const previousKey = snapshotKey(latest);
      try {
        const snapshot = await invoke("bootstrap_snapshot");
        if (snapshotKey(snapshot) === previousKey) {
          pollDelayIndex = Math.min(pollDelayIndex + 1, POLL_DELAYS_MS.length - 1);
        } else {
          pollDelayIndex = 0;
        }
        render(snapshot);
      } catch (error) {
        elements.feedback.textContent = errorText(error);
        pollDelayIndex = Math.min(pollDelayIndex + 1, POLL_DELAYS_MS.length - 1);
      }
      if (latest.inFlight && !latest.closed) schedulePoll();
    }, delay);
  }

  async function runAction(command) {
    elements.feedback.textContent = "";
    try {
      render(await invoke(command));
    } catch (error) {
      elements.feedback.textContent = errorText(error);
    }
  }

  async function copyDiagnostics() {
    const lines = [
      `phase: ${latest.phase || "unknown"}`,
      `endpoint: ${latest.endpoint || "127.0.0.1:8721"}`,
      `appUrl: ${latest.appUrl || FIXED_APP_URL}`,
      `generation: ${latest.generation ?? "unknown"}`,
      `diagnostic: ${latest.diagnostic?.message || "none"}`,
    ];
    try {
      if (!navigator.clipboard || typeof navigator.clipboard.writeText !== "function") {
        throw new Error(copy.copyFailed);
      }
      await navigator.clipboard.writeText(lines.join("\n"));
      elements.feedback.textContent = copy.copied;
    } catch (error) {
      elements.feedback.textContent = errorText(error);
    }
  }

  elements.retry.addEventListener("click", () => runAction("retry_probe_existing"));
  elements.start.addEventListener("click", () => runAction("start_local_host"));
  elements.open.addEventListener("click", async () => {
    elements.feedback.textContent = "";
    try {
      await invoke("open_in_browser");
      elements.feedback.textContent = isChinese ? "已请求浏览器打开固定 URL。" : "Requested the browser open the fixed URL.";
    } catch (error) {
      elements.feedback.textContent = errorText(error);
    }
  });
  elements.copy.addEventListener("click", copyDiagnostics);

  render(latest);
  invoke("bootstrap_snapshot").then(render).catch((error) => {
    elements.feedback.textContent = errorText(error);
    elements.retry.disabled = true;
    elements.start.disabled = true;
    elements.open.disabled = true;
  });
})();
