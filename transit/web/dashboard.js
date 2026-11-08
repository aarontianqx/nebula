"use strict";
const $ = (id) => document.getElementById(id);
const number = new Intl.NumberFormat("zh-CN");
const compact = new Intl.NumberFormat("zh-CN", {
  notation: "compact",
  maximumFractionDigits: 1,
});
const compactNumber = (value) => (value == null ? "—" : compact.format(value));
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
  const result = {
    from_ms: Math.floor(from),
    to_ms: Math.floor(to),
    utc_offset_minutes: -new Date(to).getTimezoneOffset(),
  };
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
  syncFilterControls();
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
const filterWidgets = new Map();
let activeFilter = null;
function createFilter(id, placeholder, editable, initialOptions = []) {
  const source = $(id),
    label = source.closest("label");
  const wrapper = element("span", "filter-combo");
  const control = editable ? source : element("button", "filter-select");
  const arrow = element("span", "filter-chevron");
  const toggle = editable ? element("button", "filter-toggle") : null;
  const popup = element("div", "filter-popup");
  let options = initialOptions,
    visible = [],
    active = -1,
    query = "",
    committed = source.value;
  source.before(wrapper);
  if (editable) wrapper.append(source);
  else {
    control.type = "button";
    control.id = `${id}-control`;
    wrapper.append(control);
  }
  label.htmlFor = control.id;
  const title = element(
    "span",
    "filter-label",
    label.firstChild.textContent.trim(),
  );
  title.id = `${id}-label`;
  label.firstChild.replaceWith(title);
  control.setAttribute("aria-labelledby", title.id);
  control.setAttribute("role", "combobox");
  control.setAttribute("aria-expanded", "false");
  control.setAttribute("aria-haspopup", "listbox");
  control.setAttribute("aria-controls", `${id}-options`);
  if (editable) {
    control.setAttribute("aria-autocomplete", "list");
    control.spellcheck = false;
    toggle.type = "button";
    toggle.tabIndex = -1;
    toggle.setAttribute(
      "aria-label",
      `展开${placeholder.replace("全部", "")}选项`,
    );
    toggle.setAttribute("aria-controls", `${id}-options`);
    toggle.append(arrow);
    wrapper.append(toggle);
    toggle.addEventListener("mousedown", (event) => event.preventDefault());
    toggle.addEventListener("click", () => {
      control.focus();
      if (activeFilter === widget) close();
      else open();
    });
  } else control.append(arrow);
  popup.id = `${id}-options`;
  popup.hidden = true;
  popup.setAttribute("role", "listbox");
  popup.setAttribute("aria-labelledby", title.id);
  document.body.append(popup);
  function sync() {
    committed = source.value;
    if (!editable) {
      let text = control.querySelector(".filter-value");
      if (!text) {
        text = element("span", "filter-value");
        control.prepend(text);
      }
      text.textContent =
        options.find((option) => option.value === source.value)?.label ||
        source.value ||
        placeholder;
      control.classList.toggle("is-placeholder", !source.value);
    }
  }
  function position() {
    if (popup.hidden) return;
    const rect = wrapper.getBoundingClientRect();
    if (rect.bottom < 0 || rect.top > innerHeight) {
      close();
      return;
    }
    const width = Math.min(
      Math.max(rect.width, editable ? 420 : 220),
      innerWidth - 24,
    );
    const below = innerHeight - rect.bottom - 16,
      above = rect.top - 16;
    const upwards = below < 180 && above > below;
    popup.style.width = `${width}px`;
    popup.style.left = `${Math.max(12, Math.min(rect.left, innerWidth - width - 12))}px`;
    popup.style.maxHeight = `${Math.max(60, Math.min(300, upwards ? above : below))}px`;
    popup.style.top = upwards ? "auto" : `${rect.bottom + 6}px`;
    popup.style.bottom = upwards ? `${innerHeight - rect.top + 6}px` : "auto";
  }
  function highlight(index, scroll = false) {
    active = index;
    for (const [i, option] of [
      ...popup.querySelectorAll("[role=option]"),
    ].entries())
      option.classList.toggle("active", i === active);
    if (index < 0) control.removeAttribute("aria-activedescendant");
    else {
      const option = popup.querySelectorAll("[role=option]")[index];
      control.setAttribute("aria-activedescendant", option.id);
      if (scroll) option.scrollIntoView({ block: "nearest" });
    }
  }
  function renderOptions() {
    const needle = query.toLocaleLowerCase();
    visible = [{ value: "", label: placeholder }, ...options].filter(
      (option) => !needle || option.label.toLocaleLowerCase().includes(needle),
    );
    popup.replaceChildren();
    for (const [index, option] of visible.entries()) {
      const item = element("div", "filter-option"),
        text = element("span", "", option.label);
      item.id = `${id}-option-${index}`;
      item.setAttribute("role", "option");
      item.setAttribute("aria-selected", String(option.value === source.value));
      item.title = option.label;
      item.append(
        text,
        element(
          "span",
          "filter-check",
          option.value === source.value ? "✓" : "",
        ),
      );
      item.addEventListener("mouseenter", () => highlight(index));
      item.addEventListener("click", () => commit(option.value));
      popup.append(item);
    }
    if (!visible.length) {
      const empty = element(
        "div",
        "filter-no-options",
        editable ? "无匹配项，按 Enter 应用输入值" : "暂无选项",
      );
      empty.setAttribute("role", "option");
      empty.setAttribute("aria-disabled", "true");
      popup.append(empty);
    }
    highlight(visible.findIndex((option) => option.value === source.value));
    position();
  }
  function open(search = "") {
    if (control.disabled) return;
    if (activeFilter && activeFilter !== widget) activeFilter.close();
    query = search;
    activeFilter = widget;
    popup.hidden = false;
    wrapper.classList.add("is-open");
    control.setAttribute("aria-expanded", "true");
    renderOptions();
  }
  function close() {
    popup.hidden = true;
    wrapper.classList.remove("is-open");
    control.setAttribute("aria-expanded", "false");
    control.removeAttribute("aria-activedescendant");
    if (activeFilter === widget) activeFilter = null;
  }
  function commit(value) {
    const changed = value !== committed;
    source.value = value;
    sync();
    close();
    if (changed) load();
  }
  popup.addEventListener("mousedown", (event) => event.preventDefault());
  control.addEventListener("click", () => {
    if (!editable && activeFilter === widget) close();
    else open();
  });
  if (editable) control.addEventListener("input", () => open(control.value));
  control.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if (
      ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) &&
      (activeFilter === widget || event.key.startsWith("Arrow"))
    ) {
      event.preventDefault();
      if (activeFilter !== widget) {
        open();
        highlight(event.key === "ArrowUp" ? visible.length - 1 : 0, true);
      } else {
        const index =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? visible.length - 1
              : (active +
                  (event.key === "ArrowDown" ? 1 : -1) +
                  visible.length) %
                visible.length;
        highlight(visible.length ? index : -1, true);
      }
    } else if (event.key === "Escape" && activeFilter === widget) {
      event.preventDefault();
      event.stopPropagation();
      if (editable) control.value = committed;
      close();
    } else if (event.key === "Enter" || (!editable && event.key === " ")) {
      event.preventDefault();
      if (activeFilter === widget && active >= 0) commit(visible[active].value);
      else if (editable) commit(control.value);
      else open();
    } else if (event.key === "Tab") close();
  });
  wrapper.addEventListener("focusout", (event) => {
    if (wrapper.contains(event.relatedTarget)) return;
    close();
    if (editable && !control.disabled) commit(control.value);
  });
  const widget = {
    close,
    position,
    wrapper,
    popup,
    sync,
    setOptions(values) {
      options = values;
      sync();
      if (activeFilter === widget) renderOptions();
    },
  };
  sync();
  filterWidgets.set(id, widget);
}
function syncFilterControls() {
  activeFilter?.close();
  for (const widget of filterWidgets.values()) widget.sync();
}
document.addEventListener("pointerdown", (event) => {
  if (
    activeFilter &&
    !activeFilter.wrapper.contains(event.target) &&
    !activeFilter.popup.contains(event.target)
  )
    activeFilter.close();
});
window.addEventListener("resize", () => activeFilter?.position());
window.addEventListener("scroll", () => activeFilter?.position(), true);
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
  setText("m-tokens", compactNumber(s.total_tokens));
  $("m-tokens").title = n(s.total_tokens);
  setText(
    "m-cache",
    `缓存输入 ${compactNumber(s.cache_read_tokens)} / ${compactNumber(s.cache_input_tokens)}`,
  );
  $("m-cache").title =
    `缓存读取 ${n(s.cache_read_tokens)} / 输入 ${n(s.cache_input_tokens)} token`;
  setText(
    "m-cache-rate",
    s.cache_input_tokens
      ? `${((s.cache_read_tokens / s.cache_input_tokens) * 100).toFixed(1)}%`
      : "—",
  );
  setText(
    "m-token-split",
    `输入 ${compactNumber(s.input_tokens)} / 输出 ${compactNumber(s.output_tokens)}`,
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
  for (const [id, values] of [
    ["provider", data.options.providers],
    ["model", data.options.models],
    ["path", data.options.paths],
  ]) {
    filterWidgets
      .get(id)
      .setOptions(values.map((value) => ({ value, label: value })));
  }
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
  const chartFrom = data.chart_from_ms,
    chartTo = data.chart_to_ms;
  const count = Math.round((chartTo - chartFrom) / data.bucket_ms);
  const bucketMinutes = data.bucket_ms / 60000;
  const bucketLabel =
    bucketMinutes < 60
      ? `${bucketMinutes} 分钟`
      : bucketMinutes < 1440
        ? `${bucketMinutes / 60} 小时`
        : `${bucketMinutes / 1440} 天`;
  setText("chart-bucket", `每柱 ${bucketLabel}`);
  const x = (time) =>
    left + ((time - chartFrom) / (chartTo - chartFrom)) * plotWidth;
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
  const span = chartTo - chartFrom;
  const maxTicks = width < 480 ? 3 : 5;
  const desiredTickStep = span / (maxTicks - 1);
  const tickSteps = [
    1, 2, 5, 10, 15, 30, 60, 120, 180, 360, 720, 1440, 2880, 7200, 14400, 21600,
    43200, 86400, 129600, 259200,
  ]
    .map((minutes) => minutes * 60000)
    .filter((step) => step >= data.bucket_ms && step % data.bucket_ms === 0);
  const tickStep = tickSteps.reduce((best, step) =>
    Math.abs(step - desiredTickStep) < Math.abs(best - desiredTickStep)
      ? step
      : best,
  );
  const offset = data.utc_offset_minutes * 60000;
  const firstTick =
    Math.ceil((chartFrom + offset) / tickStep) * tickStep - offset;
  const tickTimes = [];
  for (let time = firstTick; time <= chartTo; time += tickStep)
    tickTimes.push(time);
  if (tickTimes.length < 2)
    tickTimes.splice(0, tickTimes.length, chartFrom, chartTo);
  for (const time of tickTimes) {
    const position = x(time);
    svg.append(
      svgElement("line", {
        x1: position,
        x2: position,
        y1: bottom,
        y2: bottom + 5,
        class: "chart-grid",
      }),
    );
    const text =
      data.bucket_ms >= 86400000
        ? new Date(time).toLocaleDateString("zh-CN", {
            month: "2-digit",
            day: "2-digit",
          })
        : shortTime(time, span > 86400000);
    svg.append(
      svgElement(
        "text",
        {
          x: position,
          y: bottom + 21,
          "text-anchor":
            position - left < 55
              ? "start"
              : right - position < 55
                ? "end"
                : "middle",
          class: "chart-tick x-tick",
        },
        text,
      ),
    );
  }
  const bars = [];
  for (let time = chartFrom; time < chartTo; time += data.bucket_ms) {
    const value = bins.get(time)?.[chartMetric] || 0,
      end = time + data.bucket_ms;
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
    const time = chartFrom + ((px - left) / plotWidth) * span;
    const index = Math.min(
      bars.length - 1,
      Math.floor((time - chartFrom) / data.bucket_ms),
    );
    const start = chartFrom + index * data.bucket_ms,
      end = start + data.bucket_ms;
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
    const partial = [];
    if (start < data.from_ms)
      partial.push(`自 ${shortTime(data.from_ms, span > 86400000)} 起`);
    if (end > data.to_ms)
      partial.push(`截至 ${shortTime(data.to_ms, span > 86400000)}`);
    if (partial.length)
      tooltip.append(
        element("span", "chart-tooltip-partial", partial.join(" · ")),
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
const detailFields = {
  routing: [
    ["Provider", "provider"],
    ["请求模型", "requested_model"],
    ["解析后模型", "resolved_model"],
    ["响应模型", "response_model"],
    ["请求方法", "method"],
    ["请求路径", "request_path"],
    ["上游路径", "upstream_path"],
    ["客户端协议", "client_protocol"],
    ["上游协议", "upstream_protocol"],
    ["路由规则", "rule_id"],
    ["配置版本", "config_generation"],
    ["流式响应", "streaming"],
  ],
  usage: [
    ["输入 Token", "input_tokens"],
    ["输出 Token", "output_tokens"],
    ["总 Token", "total_tokens"],
    ["缓存读取", "cache_read_tokens"],
    ["缓存写入", "cache_write_tokens"],
    ["推理 Token", "reasoning_tokens"],
    ["用量状态", "usage_status"],
  ],
  diagnostics: [
    ["请求结果", "outcome"],
    ["HTTP 状态", "http_status"],
    ["上游状态", "upstream_status"],
    ["结束原因", "finish_reason"],
    ["响应状态", "response_status"],
    ["错误来源", "error_source"],
    ["错误阶段", "error_stage"],
    ["错误代码", "error_code"],
    ["错误信息", "error_message"],
    ["观察状态", "observer_status"],
    ["观察错误", "observer_error"],
    ["请求 ID", "request_id"],
    ["开始时间", "started_at_ms"],
    ["结束时间", "finished_at_ms"],
    ["总耗时", "duration_ms"],
    ["上游响应头", "response_headers_ms"],
    ["响应首字节", "first_body_byte_ms"],
    ["请求字节数", "request_bytes"],
    ["响应字节数", "response_bytes"],
  ],
};
function detailFacts(request, fields, technical = true) {
  const list = element("dl", "detail-facts");
  for (const [title, key] of fields) {
    let value = request[key];
    if (key.endsWith("_at_ms")) value = date(value, true);
    else if (key.endsWith("_ms")) value = duration(value);
    else if (typeof value === "boolean") value = value ? "是" : "否";
    else if (typeof value === "number") value = n(value);
    else value = labels[value] || value;
    const item = element("div", "detail-fact");
    const label = element("dt", "", title);
    if (technical) label.append(element("small", "", key));
    item.append(label, element("dd", "", value ?? "—"));
    list.append(item);
  }
  return list;
}
function detailSection(title, ...children) {
  const section = element("section", "detail-section");
  section.append(element("h3", "", title), ...children);
  return section;
}
function detailNotice(request) {
  if (request.outcome === "cancelled") return null;
  if (
    !request.error_message &&
    !["failed", "incomplete", "interrupted"].includes(request.outcome)
  )
    return null;
  const notice = element(
    "div",
    request.outcome === "failed" ? "detail-notice bad" : "detail-notice",
  );
  notice.append(element("strong", "", labels[request.outcome] || "请求异常"));
  if (request.error_message)
    notice.append(element("p", "", request.error_message));
  return notice;
}
function detailJson(value) {
  if (value == null) return element("p", "detail-quiet", "未提供");
  if (typeof value === "string") {
    try {
      value = JSON.parse(value);
    } catch {
      /* Keep legacy values as text. */
    }
  }
  return element(
    "pre",
    "",
    typeof value === "string" ? value : JSON.stringify(value, null, 2),
  );
}
function detailTimeline(request) {
  const pane = element("section", "detail-process");
  pane.setAttribute("aria-label", "请求过程");
  pane.append(
    element("h3", "", "请求过程"),
    element("p", "detail-duration", duration(request.duration_ms)),
  );
  const total = request.duration_ms,
    first = request.first_body_byte_ms;
  if (
    Number.isFinite(total) &&
    total > 0 &&
    Number.isFinite(first) &&
    first >= 0 &&
    first <= total
  ) {
    const strip = element("div", "detail-timing-strip");
    strip.setAttribute("aria-hidden", "true");
    const wait = element("span"),
      transfer = element("span");
    wait.style.width = `${(first / total) * 100}%`;
    transfer.style.flex = "1";
    strip.append(wait, transfer);
    const caption = element("div", "detail-timing-caption");
    caption.append(
      element("span", "", `首字节前 ${duration(first)}`),
      element("span", "", `后续传输 ${duration(total - first)}`),
    );
    pane.append(strip, caption);
  }
  const timeline = element("ol", "detail-timeline");
  const event = (title, elapsed, note, state = "") => {
    const item = element("li", `detail-event ${state}`),
      head = element("div", "detail-event-head");
    head.append(
      element("span", "", title),
      element("span", "mono", elapsed == null ? "—" : `+${duration(elapsed)}`),
    );
    item.append(head);
    if (note) item.append(element("p", "", note));
    timeline.append(item);
  };
  event(
    "接收请求",
    0,
    `${request.method || "—"} ${request.request_path || "—"}`,
  );
  if (request.response_headers_ms != null)
    event(
      "收到上游响应头",
      request.response_headers_ms,
      `上游 HTTP ${request.upstream_status ?? "—"}`,
    );
  if (first != null)
    event("收到响应首字节", first, "传输层首字节，并非首个生成 Token");
  const finished =
    request.finished_at_ms != null ||
    (request.outcome && request.outcome !== "in_progress");
  if (finished)
    event(
      labels[request.outcome] || "请求结束",
      total,
      request.finish_reason || null,
      request.outcome,
    );
  else event("等待结束记录", null, "请求进行中，或后台尚未更新");
  const bytes = (value) =>
    value == null
      ? "—"
      : value < 1024
        ? `${n(value)} B`
        : `${(value / 1024).toFixed(1)} KiB`;
  const transfer = element("div", "detail-transfer");
  transfer.append(
    element("span", "", `上传 ${bytes(request.request_bytes)}`),
    element("span", "", `下载 ${bytes(request.response_bytes)}`),
  );
  pane.append(timeline, transfer);
  return pane;
}
function detailSummary(request) {
  const result = element("div", "detail-summary");
  const notice = detailNotice(request);
  if (notice) result.append(notice);
  const flow = element("div", "detail-route");
  for (const [index, [label, value]] of [
    ["请求模型", request.requested_model],
    ["Provider", request.provider],
    [
      "实际模型",
      request.response_model ||
        request.resolved_model ||
        request.requested_model,
    ],
  ].entries()) {
    if (index) flow.append(element("span", "detail-arrow", "→"));
    const node = element("div", "detail-route-node");
    node.append(
      element("span", "", label),
      element("strong", "mono", value ?? "—"),
    );
    flow.append(node);
  }
  const path = element(
    "p",
    "detail-path mono",
    `${request.method || "—"} ${request.request_path || "—"}`,
  );
  result.append(detailSection("模型与路由", flow, path));
  const metrics = element("div", "detail-token-metrics");
  for (const [label, key] of [
    ["输入", "input_tokens"],
    ["输出", "output_tokens"],
  ]) {
    const metric = element("div");
    metric.append(
      element("span", "", label),
      element("strong", "", n(request[key])),
    );
    metrics.append(metric);
  }
  const usage = detailSection("Token 用量", metrics);
  const cache = element("div", "detail-cache-caption");
  const input = request.input_tokens,
    read = request.cache_read_tokens;
  const known =
    Number.isFinite(input) &&
    input > 0 &&
    Number.isFinite(read) &&
    read >= 0 &&
    read <= input;
  cache.append(
    element("span", "", "输入缓存命中"),
    element(
      "strong",
      "",
      known ? `${((read / input) * 100).toFixed(1)}%` : "—",
    ),
  );
  usage.append(cache);
  if (known) {
    const track = element("div", "detail-cache-track"),
      fill = element("span");
    fill.style.width = `${(read / input) * 100}%`;
    track.append(fill);
    usage.append(track);
  }
  usage.append(
    detailFacts(
      request,
      [
        ["总 Token", "total_tokens"],
        ["缓存读取", "cache_read_tokens"],
        ["缓存写入", "cache_write_tokens"],
        ["推理 Token", "reasoning_tokens"],
      ],
      false,
    ),
  );
  result.append(usage);
  const foot = element("div", "detail-summary-foot");
  foot.append(
    element("span", "", `结束原因 ${request.finish_reason ?? "—"}`),
    element("span", "", `用量${labels[request.usage_status] || "未知"}`),
  );
  result.append(foot);
  return result;
}
function renderDetail(request) {
  const status = badge(request.outcome);
  status.textContent +=
    request.http_status != null ? ` · HTTP ${request.http_status}` : "";
  $("detail-status").replaceChildren(status);
  const copy = element(
    "button",
    "detail-copy mono",
    `${String(request.request_id).slice(0, 12)} ⧉`,
  );
  copy.type = "button";
  copy.title = `复制请求 ID：${request.request_id}`;
  copy.setAttribute("aria-label", "复制请求 ID");
  copy.addEventListener("click", async () => {
    const feedback = $("detail-feedback");
    let input;
    try {
      let copied = false;
      if (navigator.clipboard && window.isSecureContext) {
        try {
          await navigator.clipboard.writeText(request.request_id);
          copied = true;
        } catch {
          // Browser permissions may still deny the modern clipboard API.
        }
      }
      if (!copied) {
        input = element("textarea", "copy-buffer");
        input.value = request.request_id;
        $("detail").append(input);
        input.select();
        if (!document.execCommand("copy")) throw new Error("copy unavailable");
      }
      feedback.textContent = "请求 ID 已复制";
    } catch {
      feedback.textContent = "复制不可用，可在 JSON 页手动复制";
    } finally {
      if (input) {
        input.remove();
        copy.focus();
      }
    }
  });
  $("detail-meta").replaceChildren(
    element("span", "", date(request.started_at_ms, true)),
    element(
      "span",
      "",
      request.streaming == null
        ? "响应类型未知"
        : request.streaming
          ? "流式响应"
          : "非流式响应",
    ),
    copy,
  );
  const inspector = element("section", "detail-inspector"),
    nav = element("div", "detail-tabs"),
    panel = element("div", "detail-tab-panel");
  nav.setAttribute("role", "tablist");
  nav.setAttribute("aria-label", "详情分类");
  panel.id = "detail-tab-panel";
  panel.setAttribute("role", "tabpanel");
  panel.tabIndex = 0;
  const tabs = [
    ["summary", "摘要"],
    ["routing", "路由"],
    ["usage", "用量"],
    ["diagnostics", "诊断"],
    ["json", "JSON"],
  ];
  function selectTab(key) {
    for (const button of nav.children) {
      const active = button.dataset.tab === key;
      button.setAttribute("aria-selected", String(active));
      button.tabIndex = active ? 0 : -1;
    }
    panel.setAttribute("aria-labelledby", `detail-tab-${key}`);
    panel.replaceChildren();
    if (key === "summary") panel.append(detailSummary(request));
    else if (key === "json")
      panel.append(detailSection("完整记录", detailJson(request)));
    else {
      const notice = key === "diagnostics" ? detailNotice(request) : null;
      if (notice) panel.append(notice);
      panel.append(
        detailSection(
          {
            routing: "模型与路由",
            usage: "Token 明细",
            diagnostics: "诊断与关联",
          }[key],
          detailFacts(request, detailFields[key]),
        ),
      );
      if (key === "usage")
        panel.append(
          detailSection("上游原始用量", detailJson(request.usage_json)),
        );
    }
    panel.scrollTop = 0;
  }
  for (const [key, title] of tabs) {
    const button = element("button", "", title);
    button.type = "button";
    button.dataset.tab = key;
    button.id = `detail-tab-${key}`;
    button.setAttribute("role", "tab");
    button.setAttribute("aria-controls", panel.id);
    button.addEventListener("click", () => selectTab(key));
    button.addEventListener("keydown", (event) => {
      const index = tabs.findIndex(([name]) => name === key);
      const next = {
        ArrowRight: (index + 1) % tabs.length,
        ArrowLeft: (index + tabs.length - 1) % tabs.length,
        Home: 0,
        End: tabs.length - 1,
      }[event.key];
      if (next == null) return;
      event.preventDefault();
      selectTab(tabs[next][0]);
      nav.children[next].focus();
    });
    nav.append(button);
  }
  inspector.append(nav, panel);
  $("detail-content").replaceChildren(detailTimeline(request), inspector);
  selectTab("summary");
}
async function showDetail(id) {
  const version = ++detailVersion,
    target = $("detail-content");
  target.replaceChildren(element("p", "empty", "正在读取记录…"));
  $("detail-status").replaceChildren();
  $("detail-meta").replaceChildren();
  $("detail-feedback").textContent = "";
  target.scrollTop = 0;
  if (!$("detail").open) $("detail").showModal();
  try {
    const request = await fetchJson(`/api/requests/${encodeURIComponent(id)}`);
    if (version !== detailVersion || !$("detail").open) return;
    renderDetail(request);
  } catch (error) {
    if (version === detailVersion && $("detail").open)
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
createFilter("provider", "全部 provider", false);
createFilter("model", "全部模型", true);
createFilter("path", "全部路径", true);
createFilter(
  "outcome",
  "全部结果",
  false,
  [
    ["completed", "已完成"],
    ["failed", "失败"],
    ["incomplete", "未完整结束"],
    ["cancelled", "已取消"],
    ["interrupted", "已中断"],
    ["in_progress", "等待结束 / 更新"],
  ].map(([value, label]) => ({ value, label })),
);
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
  if (
    $("auto").checked &&
    !document.hidden &&
    !$("detail").open &&
    !activeFilter &&
    !busy
  )
    load();
}, 15000);
load();
