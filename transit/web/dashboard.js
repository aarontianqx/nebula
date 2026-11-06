"use strict";
const $ = (id) => document.getElementById(id);
const number = new Intl.NumberFormat("zh-CN");
const compact = new Intl.NumberFormat("zh-CN", {
  notation: "compact",
  maximumFractionDigits: 1,
});
const date = (value, full = false) =>
  value == null
    ? "—"
    : new Date(value).toLocaleString(
        "zh-CN",
        full
          ? { hour12: false }
          : {
              month: "2-digit",
              day: "2-digit",
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
              hour12: false,
            },
      );
const n = (value) => (value == null ? "—" : number.format(value));
const duration = (ms) =>
  ms == null
    ? "—"
    : ms < 1000
      ? `${Math.round(ms)} ms`
      : `${(ms / 1000).toFixed(1)} s`;
const labels = {
  completed: "已完成",
  failed: "失败",
  incomplete: "未完整结束",
  cancelled: "已取消",
  interrupted: "已中断",
  in_progress: "等待更新",
  complete: "完整",
  partial: "部分",
  missing: "缺失",
};
let range = "24",
  data = null,
  chartMetric = "requests",
  busy = false,
  nextCursor = null,
  cursors = [null],
  currentFilters = null,
  requestVersion = 0;
let detailVersion = 0;
function element(tag, className, text) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (text != null) el.textContent = text;
  return el;
}
function badge(value) {
  const type =
    value === "completed"
      ? "good"
      : ["incomplete", "cancelled", "interrupted"].includes(value)
        ? "warn"
        : value === "failed"
          ? "bad"
          : "";
  return element("span", `badge ${type}`, labels[value] || value || "—");
}
function setText(id, value) {
  $(id).textContent = value;
}
function notify(message) {
  $("error").hidden = !message;
  $("error").textContent = message || "";
}
async function fetchJson(url) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 8000);
  try {
    const response = await fetch(url, {
      signal: controller.signal,
      cache: "no-store",
    });
    const value = await response.json();
    if (!response.ok)
      throw new Error(value.error || `请求失败（HTTP ${response.status}）`);
    return value;
  } finally {
    clearTimeout(timer);
  }
}
function filters() {
  const to =
    range === "custom" ? new Date($("to").value).getTime() : Date.now();
  const from =
    range === "custom"
      ? new Date($("from").value).getTime()
      : to - Number(range) * 3600000;
  if (
    !Number.isFinite(from) ||
    !Number.isFinite(to) ||
    to <= from ||
    to - from > 366 * 86400000
  )
    throw new Error("请选择有效的开始、结束时间，最长支持 366 天。");
  const result = { from_ms: Math.floor(from), to_ms: Math.floor(to) };
  for (const key of ["provider", "model", "path", "outcome"]) {
    const value = $(key).value.trim();
    if (value) result[key] = value;
  }
  return result;
}
async function load(reset = true, pageMove = 0) {
  if (busy) return;
  let proposedFilters;
  try {
    proposedFilters = reset ? filters() : currentFilters;
  } catch (error) {
    notify(error.message);
    return;
  }
  if (!proposedFilters) return;
  const proposedCursors = reset
    ? [null]
    : pageMove > 0
      ? [...cursors, nextCursor]
      : pageMove < 0
        ? cursors.slice(0, -1)
        : [...cursors];
  const query = new URLSearchParams({
    ...proposedFilters,
    ...(proposedCursors.at(-1) || {}),
  });
  busy = true;
  const version = ++requestVersion;
  for (const control of document.querySelectorAll(
    "#filters input:not(#auto), #filters select, #filters button",
  ))
    control.disabled = true;
  $("refresh").disabled = true;
  $("previous").disabled = true;
  $("next").disabled = true;
  $("metrics").setAttribute("aria-busy", "true");
  setText("connection", "正在读取…");
  try {
    const result = await fetchJson(`/api/stats?${query}`);
    if (version !== requestVersion) return;
    data = result;
    currentFilters = proposedFilters;
    cursors = proposedCursors;
    nextCursor = result.next_cursor;
    render();
    notify("");
    setText("connection", "● 已连接");
    setText("updated", `更新于 ${date(result.generated_at_ms)}`);
  } catch (error) {
    notify(
      `读取失败：${error.name === "AbortError" ? "查询超时，请稍后重试" : error.message}。${data ? "当前保留上次数据。" : ""}`,
    );
    setText("connection", "连接异常");
    if (!data)
      $("requests").replaceChildren(emptyRow("暂时无法读取记录，请重试。"));
  } finally {
    busy = false;
    $("refresh").disabled = false;
    $("metrics").setAttribute("aria-busy", "false");
    for (const control of document.querySelectorAll(
      "#filters input:not(#auto), #filters select, #filters button",
    ))
      control.disabled = false;
    $("from").disabled = range !== "custom";
    $("to").disabled = range !== "custom";
    $("previous").disabled = cursors.length <= 1;
    $("next").disabled = !nextCursor;
  }
}
function fillOptions(id, values, select = false) {
  const target = $(id),
    current = target.value;
  target.replaceChildren();
  if (select) {
    const option = element("option", "", "全部 provider");
    option.value = "";
    target.append(option);
  }
  for (const value of values) {
    const option = element("option", "", value);
    option.value = value;
    target.append(option);
  }
  if (select) {
    if (current && !values.includes(current)) {
      const option = element("option", "", current);
      option.value = current;
      target.append(option);
    }
    target.value = current;
  }
}
function render() {
  const s = data.summary,
    ended = s.requests - s.pending;
  setText("m-requests", n(s.requests));
  setText("m-completed", n(s.completed));
  setText(
    "m-success",
    `完成率 ${ended ? ((s.completed / ended) * 100).toFixed(1) + "%" : "—"}`,
  );
  $("m-success").title =
    `已完成 ${n(s.completed)} / 已结束 ${n(ended)}（不含进行中请求）`;
  const outcomes = [];
  if (s.cancelled) outcomes.push(`取消 ${n(s.cancelled)}`);
  if (s.abnormal) outcomes.push(`异常 ${n(s.abnormal)}`);
  if (s.pending) outcomes.push(`进行中 ${n(s.pending)}`);
  setText("m-outcomes", outcomes.length ? ` · ${outcomes.join(" · ")}` : "");
  setText("m-duration", duration(s.avg_duration_ms));
  setText("m-ttfb", `平均首字节 ${duration(s.avg_first_byte_ms)}`);
  setText("m-tokens", compact.format(s.total_tokens));
  $("m-tokens").title = n(s.total_tokens);
  setText(
    "m-cache",
    `缓存输入 ${compact.format(s.cached_input_tokens)} / ${compact.format(s.cache_input_tokens)}`,
  );
  $("m-cache").title =
    `缓存读取 ${n(s.cached_input_tokens)} / 输入 ${n(s.cache_input_tokens)} token`;
  setText(
    "m-cache-rate",
    s.cache_input_tokens
      ? `${((s.cached_input_tokens / s.cache_input_tokens) * 100).toFixed(1)}%`
      : "—",
  );
  setText(
    "m-token-split",
    `输入 ${compact.format(s.input_tokens)} / 输出 ${compact.format(s.output_tokens)}`,
  );
  setText(
    "m-usage",
    `正常完成的生成请求 ${n(s.completed_generation_requests)} 条，其中 ${n(s.completed_usage_issues)} 条用量不完整。全部记录中：用量部分 ${n(s.partial_usage)} 条、缺失 ${n(s.missing_usage)} 条，包含取消或中断。`,
  );
  setText(
    "cache-diagnostics",
    `完整用量记录中：缓存字段已知 ${n(s.cache_usage)} 条 · 未知 / 无效 ${n(s.complete_usage - s.cache_usage)} 条。缓存比例按 token 加权，输入为零时不计算比例。`,
  );
  $("usage-notice").hidden = s.completed_usage_issues === 0;
  setText(
    "usage-notice",
    `${n(s.completed_usage_issues)} 条已完成请求用量不完整 · 查看诊断 ↗`,
  );
  fillOptions("provider", data.options.providers, true);
  fillOptions("models", data.options.models);
  fillOptions("paths", data.options.paths);
  renderChart();
  renderDistribution();
  renderRequests();
  setText("request-count", n(s.requests));
  setText("page-number", String(cursors.length).padStart(2, "0"));
  setText("page-description", `本页 ${data.requests.length} 条`);
}
function renderChart() {
  const chart = $("chart");
  chart.replaceChildren();
  const width = Math.max(chart.clientWidth, 260),
    height = chart.clientHeight;
  const left = 52,
    right = width - 10,
    top = 12,
    bottom = height - 36;
  const plotWidth = right - left,
    plotHeight = bottom - top;
  const bins = new Map(data.series.map((row) => [row.time_ms, row]));
  const max = Math.max(0, ...data.series.map((row) => row[chartMetric]));
  const rawStep = Math.max(max, 1) / 4;
  const power = 10 ** Math.floor(Math.log10(rawStep));
  let step = [1, 2, 2.5, 5, 10].find((v) => v * power >= rawStep) * power;
  if (chartMetric === "requests") step = Math.max(1, Math.ceil(step));
  const ceiling = Math.max(step, Math.ceil(max / step) * step);
  const x = (time) =>
    left + ((time - data.from_ms) / (data.to_ms - data.from_ms)) * plotWidth;
  const y = (value) => bottom - (value / ceiling) * plotHeight;
  const svgElement = (tag, attrs = {}, text) => {
    const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attrs))
      node.setAttribute(key, String(value));
    if (text != null) node.textContent = text;
    return node;
  };
  const svg = svgElement("svg", {
    viewBox: `0 0 ${width} ${height}`,
    role: "img",
    "aria-label":
      chartMetric === "requests"
        ? "请求数量趋势，横轴为时间，纵轴为请求数"
        : "Token 用量趋势，横轴为时间，纵轴为 Token 数",
  });
  for (let value = 0; value <= ceiling + step / 2; value += step) {
    const position = y(value);
    svg.append(
      svgElement("line", {
        x1: left,
        x2: right,
        y1: position,
        y2: position,
        class: "chart-grid",
      }),
    );
    svg.append(
      svgElement(
        "text",
        {
          x: left - 9,
          y: position,
          "text-anchor": "end",
          "dominant-baseline": "middle",
          class: "chart-tick y-tick",
        },
        compact.format(value),
      ),
    );
  }
  const shortTime = (time, withDate = false) =>
    new Date(time).toLocaleString("zh-CN", {
      ...(withDate ? { month: "2-digit", day: "2-digit" } : {}),
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
  const span = data.to_ms - data.from_ms;
  const ticks = width < 480 ? 3 : 5;
  for (let index = 0; index < ticks; index++) {
    const time = data.from_ms + (span * index) / (ticks - 1),
      position = x(time);
    svg.append(
      svgElement("line", {
        x1: position,
        x2: position,
        y1: bottom,
        y2: bottom + 5,
        class: "chart-grid",
      }),
    );
    svg.append(
      svgElement(
        "text",
        {
          x: position,
          y: bottom + 21,
          "text-anchor":
            index === 0 ? "start" : index === ticks - 1 ? "end" : "middle",
          class: "chart-tick x-tick",
        },
        shortTime(time, span > 86400000),
      ),
    );
  }
  const bars = [];
  for (let time = data.from_ms; time < data.to_ms; time += data.bucket_ms) {
    const value = bins.get(time)?.[chartMetric] || 0,
      end = Math.min(time + data.bucket_ms, data.to_ms);
    const gap = Math.min(4, (x(end) - x(time)) * 0.2);
    const bar = svgElement("rect", {
      x: x(time) + gap / 2,
      y: y(value),
      width: Math.max(0, x(end) - x(time) - gap),
      height: bottom - y(value),
      rx: 2,
      class: "chart-bar",
    });
    svg.append(bar);
    bars.push(bar);
  }
  if (!max)
    svg.append(
      svgElement(
        "text",
        {
          x: (left + right) / 2,
          y: (top + bottom) / 2,
          "text-anchor": "middle",
          class: "chart-empty",
        },
        chartMetric === "requests"
          ? "该时间范围没有请求"
          : "该时间范围暂无用量",
      ),
    );
  const guide = svgElement("g", { class: "chart-guide", visibility: "hidden" });
  const horizontal = svgElement("line", {
    x1: left,
    x2: right,
    class: "chart-crosshair",
  });
  const vertical = svgElement("line", {
    y1: top,
    y2: bottom,
    class: "chart-crosshair vertical",
  });
  const labelBackground = svgElement("rect", {
    x: 0,
    width: left - 4,
    height: 20,
    rx: 4,
    class: "chart-readout-bg",
  });
  const readout = svgElement("text", {
    x: left - 9,
    "text-anchor": "end",
    "dominant-baseline": "middle",
    class: "chart-readout",
  });
  guide.append(horizontal, vertical, labelBackground, readout);
  svg.append(guide);
  const tooltip = element("div", "chart-tooltip");
  tooltip.hidden = true;
  chart.append(svg, tooltip);
  let active = null;
  const clear = () => {
    guide.setAttribute("visibility", "hidden");
    tooltip.hidden = true;
    active?.classList.remove("active");
    active = null;
  };
  svg.addEventListener("pointermove", (event) => {
    const bounds = svg.getBoundingClientRect(),
      px = ((event.clientX - bounds.left) * width) / bounds.width,
      py = ((event.clientY - bounds.top) * height) / bounds.height;
    if (px < left || px > right || py < top || py > bottom) {
      clear();
      return;
    }
    const time = data.from_ms + ((px - left) / plotWidth) * span;
    const index = Math.min(
      bars.length - 1,
      Math.floor((time - data.from_ms) / data.bucket_ms),
    );
    const start = data.from_ms + index * data.bucket_ms,
      end = Math.min(start + data.bucket_ms, data.to_ms);
    const value = bins.get(start)?.[chartMetric] || 0;
    active?.classList.remove("active");
    active = bars[index];
    active?.classList.add("active");
    horizontal.setAttribute("y1", py);
    horizontal.setAttribute("y2", py);
    vertical.setAttribute("x1", px);
    vertical.setAttribute("x2", px);
    labelBackground.setAttribute("y", py - 10);
    readout.setAttribute("y", py);
    readout.textContent = compact.format(
      Math.round(((bottom - py) / plotHeight) * ceiling),
    );
    guide.setAttribute("visibility", "visible");
    tooltip.replaceChildren(
      element(
        "span",
        "chart-tooltip-time",
        `${shortTime(start, span > 86400000)}–${shortTime(end, span > 86400000)}`,
      ),
      element(
        "strong",
        "",
        `${n(value)} ${chartMetric === "requests" ? "次请求" : "token"}`,
      ),
    );
    tooltip.hidden = false;
    tooltip.style.left = `${Math.max(0, Math.min(width - tooltip.offsetWidth, px + 12))}px`;
    tooltip.style.top = `${py > height / 2 ? py - tooltip.offsetHeight - 12 : py + 12}px`;
  });
  svg.addEventListener("pointerleave", clear);
}
let chartWidth = 0;
new ResizeObserver(([entry]) => {
  const width = Math.round(entry.contentRect.width);
  if (width !== chartWidth) {
    chartWidth = width;
    if (data) renderChart();
  }
}).observe($("chart"));
function renderDistribution() {
  const target = $("distribution");
  target.replaceChildren();
  const groups = data.groups.slice(0, 5),
    max = groups[0]?.requests || 1;
  if (!groups.length) {
    target.append(element("p", "empty", "当前筛选没有模型记录"));
    return;
  }
  for (const group of groups) {
    const row = element("div", "model-row"),
      label = element("div", "model-label"),
      button = element(
        "button",
        "",
        `${group.provider || "未知 provider"}/${group.model || "未知模型"}`,
      );
    button.type = "button";
    button.disabled = group.model == null;
    button.title = `${group.provider || "未知 provider"}/${group.model || "未知模型"} · 点击筛选此 provider 和模型`;
    button.addEventListener("click", () => {
      if (busy) return;
      $("model").value = group.model;
      $("provider").value = group.provider || "";
      load();
    });
    label.append(button, element("span", "", `${n(group.requests)} 次`));
    const track = element("div", "model-track"),
      fill = element("div", "model-fill");
    fill.style.width = `${(group.requests / max) * 100}%`;
    track.append(fill);
    row.append(label, track);
    target.append(row);
  }
}
function emptyRow(text) {
  const row = element("tr"),
    cell = element("td", "empty", text);
  cell.colSpan = 6;
  row.append(cell);
  return row;
}
function renderRequests() {
  const target = $("requests");
  target.replaceChildren();
  if (!data.requests.length) {
    target.append(
      emptyRow("该筛选条件下没有请求。试试扩大时间范围或重置筛选。"),
    );
    return;
  }
  for (const request of data.requests) {
    const row = element("tr", "request-row"),
      identity = element("td", "mono", date(request.started_at_ms));
    row.tabIndex = 0;
    row.setAttribute(
      "aria-label",
      `查看 ${date(request.started_at_ms)} 的请求详情`,
    );
    row.setAttribute("aria-haspopup", "dialog");
    const open = () => {
      row.focus({ preventScroll: true });
      showDetail(request.request_id);
    };
    row.addEventListener("click", () => {
      const selection = window.getSelection();
      if (selection?.toString() && row.contains(selection.anchorNode)) return;
      open();
    });
    row.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        open();
      }
    });
    const model =
      request.response_model ||
      request.resolved_model ||
      request.requested_model ||
      "未知模型";
    const route = element("td", "mono", `${request.provider || "—"}/${model}`);
    route.title = route.textContent;
    const path = element("td", "mono", request.request_path || "—");
    path.title = path.textContent;
    const outcome = element("td"),
      result = badge(request.outcome);
    const status = request.http_status;
    if (request.outcome === "completed")
      result.textContent = status == null ? "已完成" : String(status);
    else if (request.outcome === "failed" && status >= 400)
      result.textContent = String(status);
    else if (request.outcome === "failed" && status != null)
      result.textContent = `失败 · ${status}`;
    else if (request.outcome === "incomplete") result.textContent = "未完整";
    else if (request.outcome === "in_progress") result.textContent = "待结束";
    result.title = `${labels[request.outcome] || request.outcome} · HTTP ${status ?? "—"}${request.error_code ? " · " + request.error_code : ""}`;
    result.setAttribute("aria-label", result.title);
    outcome.append(result);
    const elapsed = element("td", "mono", duration(request.duration_ms));
    const tokens = element(
      "td",
      "numeric",
      `${n(request.input_tokens)} / ${n(request.output_tokens)} / ${n(request.total_tokens)}`,
    );
    tokens.title = `输入 ${n(request.input_tokens)} / 输出 ${n(request.output_tokens)} / 总计 ${n(request.total_tokens)} · 用量${labels[request.usage_status] || request.usage_status || "未知"}`;
    tokens.setAttribute("aria-label", tokens.title);
    row.append(identity, route, path, outcome, elapsed, tokens);
    target.append(row);
  }
}
const sections = [
  [
    "关联与时间",
    [
      "request_id",
      "started_at_ms",
      "finished_at_ms",
      "config_generation",
      "rule_id",
    ],
  ],
  [
    "路由与模型",
    [
      "method",
      "request_path",
      "upstream_path",
      "provider",
      "client_protocol",
      "upstream_protocol",
      "requested_model",
      "resolved_model",
      "response_model",
      "streaming",
    ],
  ],
  [
    "结果与时延",
    [
      "outcome",
      "http_status",
      "upstream_status",
      "duration_ms",
      "response_headers_ms",
      "first_body_byte_ms",
      "request_bytes",
      "response_bytes",
      "finish_reason",
      "response_status",
    ],
  ],
  [
    "用量与观察",
    [
      "usage_status",
      "observer_status",
      "observer_error",
      "input_tokens",
      "output_tokens",
      "total_tokens",
      "cached_input_tokens",
      "cache_write_input_tokens",
      "reasoning_tokens",
    ],
  ],
  ["错误", ["error_source", "error_stage", "error_code", "error_message"]],
];
async function showDetail(id) {
  const version = ++detailVersion,
    target = $("detail-content");
  target.replaceChildren(element("p", "empty", "正在读取记录…"));
  target.scrollTop = 0;
  if (!$("detail").open) $("detail").showModal();
  try {
    const request = await fetchJson(`/api/requests/${encodeURIComponent(id)}`);
    if (version !== detailVersion || !$("detail").open) return;
    target.replaceChildren();
    for (const [title, fields] of sections) {
      const section = element("section", "detail-section"),
        list = element("dl", "detail-grid");
      section.append(element("h3", "", title));
      for (const field of fields) {
        const item = element("div", "detail-field");
        let value = request[field];
        if (field.endsWith("_at_ms")) value = date(value, true);
        else if (field === "streaming")
          value = value == null ? "—" : value ? "true" : "false";
        item.append(
          element("dt", "", field),
          element("dd", "", value == null ? "—" : String(value)),
        );
        list.append(item);
      }
      section.append(list);
      target.append(section);
    }
    if (request.usage_json) {
      let usage = request.usage_json;
      try {
        usage = JSON.stringify(JSON.parse(usage), null, 2);
      } catch {
        /* Preserve a legacy non-JSON value as text. */
      }
      target.append(element("h3", "", "usage_json"), element("pre", "", usage));
    }
  } catch (error) {
    if (version === detailVersion)
      target.replaceChildren(
        element("p", "empty", `详情读取失败：${error.message}`),
      );
  }
}
$("filters").addEventListener("submit", (event) => {
  event.preventDefault();
  load();
});
for (const button of document.querySelectorAll("[data-range]")) {
  button.setAttribute("aria-pressed", String(button.dataset.range === range));
  button.addEventListener("click", () => {
    if (busy) return;
    range = button.dataset.range;
    for (const item of document.querySelectorAll("[data-range]")) {
      item.classList.toggle("selected", item === button);
      item.setAttribute("aria-pressed", String(item === button));
    }
    $("custom-range").hidden = range !== "custom";
    $("from").disabled = range !== "custom";
    $("to").disabled = range !== "custom";
    if (range !== "custom") load();
  });
}
for (const button of document.querySelectorAll("[data-chart]")) {
  button.setAttribute(
    "aria-pressed",
    String(button.dataset.chart === chartMetric),
  );
  button.addEventListener("click", () => {
    chartMetric = button.dataset.chart;
    for (const item of document.querySelectorAll("[data-chart]")) {
      item.classList.toggle("selected", item === button);
      item.setAttribute("aria-pressed", String(item === button));
    }
    if (data) renderChart();
  });
}
for (const id of ["provider", "outcome"])
  $(id).addEventListener("change", () => load());
for (const id of ["model", "path"])
  $(id).addEventListener("change", () => load());
$("reset").addEventListener("click", () => {
  if (busy) return;
  for (const id of ["provider", "model", "path", "outcome"]) $(id).value = "";
  load();
});
$("previous").addEventListener("click", () => {
  if (cursors.length > 1) load(false, -1);
});
$("next").addEventListener("click", () => {
  if (nextCursor) load(false, 1);
});
$("close-detail").addEventListener("click", () => $("detail").close());
$("usage-notice").addEventListener("click", () => {
  $("diagnostics").open = true;
  $("diagnostics").scrollIntoView({ block: "center" });
  $("diagnostics").querySelector("summary").focus();
});
$("detail").addEventListener("close", () => {
  detailVersion++;
});
$("detail").addEventListener("click", (event) => {
  if (event.target === $("detail")) {
    const r = $("detail").getBoundingClientRect();
    if (
      event.clientX < r.left ||
      event.clientX > r.right ||
      event.clientY < r.top ||
      event.clientY > r.bottom
    )
      $("detail").close();
  }
});
const localInput = (ms) => {
  const d = new Date(ms);
  return new Date(ms - d.getTimezoneOffset() * 60000)
    .toISOString()
    .slice(0, 16);
};
$("from").value = localInput(Date.now() - 86400000);
$("to").value = localInput(Date.now());
$("updated").title =
  `显示时区：${Intl.DateTimeFormat().resolvedOptions().timeZone}`;
setInterval(() => {
  if ($("auto").checked && !document.hidden && !$("detail").open && !busy)
    load();
}, 15000);
load();
