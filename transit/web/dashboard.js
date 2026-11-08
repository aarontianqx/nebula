import {
  t,
  locale,
  language,
  setLanguage,
  languages,
  applyTranslations,
} from "./i18n.js";
const $ = (id) => document.getElementById(id);
let number, compact, decimal, labels;
function configureFormatters() {
  number = new Intl.NumberFormat(locale());
  decimal = new Intl.NumberFormat(locale(), {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  compact = new Intl.NumberFormat(locale(), {
    notation: "compact",
    maximumFractionDigits: 1,
  });
  labels = Object.fromEntries(
    [
      "completed",
      "failed",
      "incomplete",
      "cancelled",
      "interrupted",
      "in_progress",
      "complete",
      "partial",
      "missing",
    ].map((key) => [key, t(`status.${key}`)]),
  );
}
const compactNumber = (value) => (value == null ? "—" : compact.format(value));
const n = (value) => (value == null ? "—" : number.format(value));
const date = (value, full = false) =>
  value == null
    ? "—"
    : new Date(value).toLocaleString(
        locale(),
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
const duration = (ms) =>
  ms == null
    ? "—"
    : ms < 1000
      ? `${number.format(Math.round(ms))} ms`
      : `${decimal.format(ms / 1000)} s`;
const cacheHitRate = (input, read) =>
  Number.isFinite(input) &&
  input > 0 &&
  Number.isFinite(read) &&
  read >= 0 &&
  read <= input
    ? read / input
    : null;
const percent = (ratio) =>
  ratio == null ? "—" : `${decimal.format(ratio * 100)}%`;
configureFormatters();
let range = "24",
  data = null,
  chartMetric = "requests",
  busy = false,
  nextCursor = null,
  cursors = [null],
  currentFilters = null,
  requestVersion = 0;
let detailVersion = 0,
  selectedDetail = null,
  detailError = null,
  dashboardError = null,
  connectionState = "connecting";
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
class UiError extends Error {
  constructor(key, values = {}) {
    super(key);
    this.key = key;
    this.values = values;
  }
}
function apiErrorKey(status) {
  return (
    {
      400: "error.badRequest",
      403: "error.forbidden",
      404: "error.notFound",
      503: "error.unavailable",
    }[status] || "error.http"
  );
}
function errorText(error) {
  if (error instanceof UiError) return t(error.key, error.values);
  if (error.name === "AbortError") return t("error.timeout");
  if (error instanceof TypeError) return t("error.network");
  return t("error.unexpected");
}
async function fetchJson(url) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 8000);
  try {
    const response = await fetch(url, {
      signal: controller.signal,
      cache: "no-store",
    });
    if (!response.ok)
      throw new UiError(apiErrorKey(response.status), {
        status: response.status,
      });
    return await response.json();
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
    throw new UiError("error.range");
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
    dashboardError = error;
    notify(errorText(error));
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
  connectionState = "loading";
  setText("connection", t("connection.loading"));
  try {
    const result = await fetchJson(`/api/stats?${query}`);
    if (version !== requestVersion) return;
    data = result;
    currentFilters = proposedFilters;
    cursors = proposedCursors;
    nextCursor = result.next_cursor;
    render();
    dashboardError = null;
    notify("");
    connectionState = "connected";
    setText("connection", t("connection.connected"));
    setText(
      "updated",
      t("connection.updated", { time: date(result.generated_at_ms) }),
    );
  } catch (error) {
    dashboardError = error;
    connectionState = "failed";
    notify(
      t("error.read", {
        message: errorText(error),
        retained: data ? t("error.retained") : "",
      }),
    );
    setText("connection", t("connection.failed"));
    if (!data) $("requests").replaceChildren(emptyRow(t("error.noRecords")));
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
function createFilter(id, placeholderKey, editable, initialOptions = []) {
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
  const placeholder = () => t(placeholderKey);
  source.before(wrapper);
  if (editable) wrapper.append(source);
  else {
    control.type = "button";
    control.id = `${id}-control`;
    wrapper.append(control);
  }
  label.htmlFor = control.id;
  const title = label.querySelector("[data-i18n]");
  title.classList.add("filter-label");
  title.id = `${id}-label`;
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
      t("filter.expand", { name: t(`filter.${id}`) }),
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
      const option = options.find((option) => option.value === source.value);
      text.textContent =
        (option?.labelKey ? t(option.labelKey) : option?.label) ||
        source.value ||
        placeholder();
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
    visible = [
      { value: "", label: placeholder() },
      ...options.map((option) => ({
        ...option,
        label: option.labelKey ? t(option.labelKey) : option.label,
      })),
    ].filter(
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
      const empty = element("div", "filter-no-options", t("filter.noMatches"));
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
    localize() {
      if (editable) source.placeholder = placeholder();
      if (toggle)
        toggle.setAttribute(
          "aria-label",
          t("filter.expand", { name: t(`filter.${id}`) }),
        );
      sync();
    },
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
    t("metrics.completion", {
      rate: ended ? decimal.format((s.completed / ended) * 100) + "%" : "—",
    }),
  );
  $("m-success").title = t("metrics.completionHint", {
    completed: n(s.completed),
    ended: n(ended),
  });
  const outcomes = [];
  if (s.cancelled)
    outcomes.push(t("metrics.cancelled", { count: n(s.cancelled) }));
  if (s.abnormal)
    outcomes.push(t("metrics.abnormal", { count: n(s.abnormal) }));
  if (s.pending) outcomes.push(t("metrics.pending", { count: n(s.pending) }));
  setText("m-outcomes", outcomes.length ? ` · ${outcomes.join(" · ")}` : "");
  setText("m-duration", duration(s.avg_duration_ms));
  setText(
    "m-ttfb",
    t("metrics.firstByte", { duration: duration(s.avg_first_byte_ms) }),
  );
  setText("m-tokens", compactNumber(s.total_tokens));
  $("m-tokens").title = n(s.total_tokens);
  setText(
    "m-cache",
    t("metrics.cache", {
      read: compactNumber(s.cache_read_tokens),
      input: compactNumber(s.cache_input_tokens),
    }),
  );
  $("m-cache").title = t("metrics.cacheHint", {
    read: n(s.cache_read_tokens),
    input: n(s.cache_input_tokens),
  });
  setText(
    "m-cache-rate",
    percent(cacheHitRate(s.cache_input_tokens, s.cache_read_tokens)),
  );
  setText(
    "m-token-split",
    t("metrics.tokenSplit", {
      input: compactNumber(s.input_tokens),
      output: compactNumber(s.output_tokens),
    }),
  );
  setText(
    "m-usage",
    t("diagnostics.usage", {
      completed: n(s.completed_generation_requests),
      issues: n(s.completed_usage_issues),
      partial: n(s.partial_usage),
      missing: n(s.missing_usage),
    }),
  );
  setText(
    "cache-diagnostics",
    t("diagnostics.cache", {
      known: n(s.cache_usage),
      unknown: n(s.complete_usage - s.cache_usage),
    }),
  );
  $("usage-notice").hidden = s.completed_usage_issues === 0;
  setText(
    "usage-notice",
    t("metrics.usageIssues", { count: n(s.completed_usage_issues) }),
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
  setText(
    "page-description",
    t("page.count", { count: n(data.requests.length) }),
  );
}
function renderChart() {
  const chart = $("chart");
  chart.replaceChildren();
  const width = Math.max(chart.clientWidth, 260),
    height = chart.clientHeight;
  const chartFrom = data.chart_from_ms,
    chartTo = data.chart_to_ms,
    span = chartTo - chartFrom,
    datedTicks = data.bucket_ms < 86400000 && span > 86400000;
  const left = 52,
    right = width - 10,
    top = 12,
    bottom = height - (datedTicks ? 48 : 36);
  const plotWidth = right - left,
    plotHeight = bottom - top;
  const bins = new Map(data.series.map((row) => [row.time_ms, row]));
  const max = Math.max(0, ...data.series.map((row) => row[chartMetric]));
  const rawStep = Math.max(max, 1) / 4;
  const power = 10 ** Math.floor(Math.log10(rawStep));
  let step = [1, 2, 2.5, 5, 10].find((v) => v * power >= rawStep) * power;
  if (chartMetric === "requests") step = Math.max(1, Math.ceil(step));
  const ceiling = Math.max(step, Math.ceil(max / step) * step);
  const count = Math.round((chartTo - chartFrom) / data.bucket_ms);
  const bucketMinutes = data.bucket_ms / 60000;
  const bucketLabel =
    bucketMinutes < 60
      ? t("chart.minutes", { count: n(bucketMinutes) })
      : bucketMinutes < 1440
        ? t("chart.hours", { count: n(bucketMinutes / 60) })
        : t("chart.days", { count: n(bucketMinutes / 1440) });
  setText("chart-bucket", t("chart.bucket", { duration: bucketLabel }));
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
        ? t("chart.requestsLabel")
        : t("chart.tokensLabel"),
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
    new Date(time).toLocaleString(locale(), {
      ...(withDate ? { month: "2-digit", day: "2-digit" } : {}),
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
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
    const day = new Date(time).toLocaleDateString(locale(), {
      month: "2-digit",
      day: "2-digit",
    });
    const text = data.bucket_ms >= 86400000 ? day : shortTime(time);
    const tick = svgElement(
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
    );
    if (datedTicks)
      tick.append(svgElement("tspan", { x: position, dy: 13 }, day));
    svg.append(tick);
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
        chartMetric === "requests" ? t("chart.noRequests") : t("chart.noUsage"),
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
        `${n(value)} ${chartMetric === "requests" ? t("chart.requestUnit") : t("chart.tokenUnit")}`,
      ),
    );
    const partial = [];
    if (start < data.from_ms)
      partial.push(
        t("chart.since", { time: shortTime(data.from_ms, span > 86400000) }),
      );
    if (end > data.to_ms)
      partial.push(
        t("chart.until", { time: shortTime(data.to_ms, span > 86400000) }),
      );
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
    target.append(element("p", "empty", t("model.empty")));
    return;
  }
  for (const group of groups) {
    const row = element("div", "model-row"),
      label = element("div", "model-label"),
      button = element(
        "button",
        "",
        `${group.provider || t("model.unknownProvider")}/${group.model || t("model.unknown")}`,
      );
    button.type = "button";
    button.disabled = group.model == null;
    button.title = t("model.filterHint", {
      model: `${group.provider || t("model.unknownProvider")}/${group.model || t("model.unknown")}`,
    });
    button.addEventListener("click", () => {
      if (busy) return;
      $("model").value = group.model;
      $("provider").value = group.provider || "";
      load();
    });
    label.append(
      button,
      element("span", "", t("model.count", { count: n(group.requests) })),
    );
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
  cell.colSpan = 7;
  row.append(cell);
  return row;
}
function renderRequests() {
  const target = $("requests");
  target.replaceChildren();
  if (!data.requests.length) {
    target.append(emptyRow(t("requests.empty")));
    return;
  }
  for (const request of data.requests) {
    const row = element("tr", "request-row"),
      identity = element("td", "mono", date(request.started_at_ms));
    row.tabIndex = 0;
    row.setAttribute(
      "aria-label",
      t("requests.open", { time: date(request.started_at_ms) }),
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
      t("model.unknown");
    const route = element("td", "mono", `${request.provider || "—"}/${model}`);
    route.title = route.textContent;
    const path = element("td", "mono", request.request_path || "—");
    path.title = path.textContent;
    const outcome = element("td"),
      result = badge(request.outcome);
    const status = request.http_status;
    if (request.outcome === "completed")
      result.textContent =
        status == null ? t("status.completed") : String(status);
    else if (request.outcome === "failed" && status >= 400)
      result.textContent = String(status);
    else if (request.outcome === "failed" && status != null)
      result.textContent = t("requests.failed", { status });
    else if (request.outcome === "incomplete")
      result.textContent = t("status.incompleteShort");
    else if (request.outcome === "in_progress")
      result.textContent = t("status.pendingShort");
    result.title = `${labels[request.outcome] || request.outcome} · HTTP ${status ?? "—"}${request.error_code ? " · " + request.error_code : ""}`;
    result.setAttribute("aria-label", result.title);
    outcome.append(result);
    const elapsed = element("td", "mono", duration(request.duration_ms));
    const tokens = element("td", "numeric", n(request.total_tokens));
    const cache = element(
      "td",
      "numeric",
      percent(cacheHitRate(request.input_tokens, request.cache_read_tokens)),
    );
    cache.title = t("metrics.cacheHint", {
      read: n(request.cache_read_tokens),
      input: n(request.input_tokens),
    });
    row.append(identity, route, path, outcome, elapsed, tokens, cache);
    target.append(row);
  }
}
function detailFields() {
  return {
    routing: [
      [t("field.provider"), "provider"],
      [t("field.requested_model"), "requested_model"],
      [t("field.resolved_model"), "resolved_model"],
      [t("field.response_model"), "response_model"],
      [t("field.method"), "method"],
      [t("field.request_path"), "request_path"],
      [t("field.upstream_path"), "upstream_path"],
      [t("field.client_protocol"), "client_protocol"],
      [t("field.upstream_protocol"), "upstream_protocol"],
      [t("field.rule_id"), "rule_id"],
      [t("field.config_generation"), "config_generation"],
      [t("field.streaming"), "streaming"],
    ],
    usage: [
      [t("field.input_tokens"), "input_tokens"],
      [t("field.output_tokens"), "output_tokens"],
      [t("field.total_tokens"), "total_tokens"],
      [t("field.cache_read_tokens"), "cache_read_tokens"],
      [t("field.cache_write_tokens"), "cache_write_tokens"],
      [t("field.reasoning_tokens"), "reasoning_tokens"],
      [t("field.usage_status"), "usage_status"],
    ],
    diagnostics: [
      [t("field.outcome"), "outcome"],
      [t("field.http_status"), "http_status"],
      [t("field.upstream_status"), "upstream_status"],
      [t("field.finish_reason"), "finish_reason"],
      [t("field.response_status"), "response_status"],
      [t("field.error_source"), "error_source"],
      [t("field.error_stage"), "error_stage"],
      [t("field.error_code"), "error_code"],
      [t("field.error_message"), "error_message"],
      [t("field.observer_status"), "observer_status"],
      [t("field.observer_error"), "observer_error"],
      [t("field.request_id"), "request_id"],
      [t("field.started_at_ms"), "started_at_ms"],
      [t("field.finished_at_ms"), "finished_at_ms"],
      [t("field.duration_ms"), "duration_ms"],
      [t("field.response_headers_ms"), "response_headers_ms"],
      [t("field.first_body_byte_ms"), "first_body_byte_ms"],
      [t("field.request_bytes"), "request_bytes"],
      [t("field.response_bytes"), "response_bytes"],
    ],
  };
}
function detailFacts(request, fields) {
  const list = element("dl", "detail-facts");
  for (const [title, key] of fields) {
    let value = request[key];
    if (key.endsWith("_at_ms")) value = date(value, true);
    else if (key.endsWith("_ms")) value = duration(value);
    else if (typeof value === "boolean")
      value = value ? t("common.yes") : t("common.no");
    else if (typeof value === "number") value = n(value);
    else if (key === "outcome" || key === "usage_status")
      value = labels[value] || value;
    else if (
      key === "observer_status" &&
      [
        "pending",
        "observing",
        "observed",
        "partial",
        "unsupported",
        "parse_error",
      ].includes(value)
    )
      value = t(`observer.${value}`);
    const item = element("div", "detail-fact");
    const label = element("dt", "", title);
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
  notice.append(
    element("strong", "", labels[request.outcome] || t("detail.abnormal")),
  );
  if (request.error_message)
    notice.append(element("p", "", request.error_message));
  return notice;
}
function detailJson(value) {
  if (value == null)
    return element("p", "detail-quiet", t("common.notProvided"));
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
  pane.setAttribute("aria-label", t("detail.process"));
  pane.append(
    element("h3", "", t("detail.process")),
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
      element("span", "", t("detail.waitTime", { duration: duration(first) })),
      element(
        "span",
        "",
        t("detail.transferTime", { duration: duration(total - first) }),
      ),
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
    t("detail.received"),
    0,
    `${request.method || "—"} ${request.request_path || "—"}`,
  );
  if (request.response_headers_ms != null)
    event(
      t("detail.headers"),
      request.response_headers_ms,
      t("detail.upstreamHttp", { status: request.upstream_status ?? "—" }),
    );
  if (first != null)
    event(t("detail.firstByte"), first, t("detail.firstByteNote"));
  const finished =
    request.finished_at_ms != null ||
    (request.outcome && request.outcome !== "in_progress");
  if (finished)
    event(
      labels[request.outcome] || t("detail.ended"),
      total,
      request.finish_reason || null,
      request.outcome,
    );
  else event(t("detail.pending"), null, t("detail.pendingNote"));
  const bytes = (value) =>
    value == null
      ? "—"
      : value < 1024
        ? `${n(value)} B`
        : `${decimal.format(value / 1024)} KiB`;
  const transfer = element("div", "detail-transfer");
  transfer.append(
    element(
      "span",
      "",
      t("detail.upload", { bytes: bytes(request.request_bytes) }),
    ),
    element(
      "span",
      "",
      t("detail.download", { bytes: bytes(request.response_bytes) }),
    ),
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
    [t("field.requested_model"), request.requested_model],
    [t("field.provider"), request.provider],
    [
      t("detail.actualModel"),
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
  result.append(detailSection(t("detail.routing"), flow, path));
  const metrics = element("div", "detail-token-metrics");
  for (const [label, key] of [
    [t("token.input"), "input_tokens"],
    [t("token.output"), "output_tokens"],
  ]) {
    const metric = element("div");
    metric.append(
      element("span", "", label),
      element("strong", "", n(request[key])),
    );
    metrics.append(metric);
  }
  const usage = detailSection(t("detail.usage"), metrics);
  const cache = element("div", "detail-cache-caption");
  const rate = cacheHitRate(request.input_tokens, request.cache_read_tokens);
  cache.append(
    element("span", "", t("cache.hit")),
    element("strong", "", percent(rate)),
  );
  usage.append(cache);
  if (rate != null) {
    const track = element("div", "detail-cache-track"),
      fill = element("span");
    fill.style.width = `${rate * 100}%`;
    track.append(fill);
    usage.append(track);
  }
  usage.append(
    detailFacts(request, [
      [t("field.total_tokens"), "total_tokens"],
      [t("field.cache_read_tokens"), "cache_read_tokens"],
      [t("field.cache_write_tokens"), "cache_write_tokens"],
      [t("field.reasoning_tokens"), "reasoning_tokens"],
    ]),
  );
  result.append(usage);
  const foot = element("div", "detail-summary-foot");
  foot.append(
    element(
      "span",
      "",
      t("detail.finish", { reason: request.finish_reason ?? "—" }),
    ),
    element(
      "span",
      "",
      t("detail.usageState", {
        status: labels[request.usage_status] || t("common.unknown"),
      }),
    ),
  );
  result.append(foot);
  return result;
}
function renderDetail(request) {
  selectedDetail = request;
  const status = badge(request.outcome);
  status.textContent +=
    request.http_status != null ? ` · HTTP ${request.http_status}` : "";
  $("detail-status").replaceChildren(status);
  const copy = element(
    "button",
    "detail-copy mono",
    String(request.request_id).slice(0, 12),
  );
  const copyIcon = element("span", "copy-icon");
  copyIcon.setAttribute("aria-hidden", "true");
  copy.append(copyIcon);
  copy.type = "button";
  copy.title = t("detail.copyHint", { id: request.request_id });
  copy.setAttribute("aria-label", t("detail.copy"));
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
      feedback.textContent = t("detail.copied");
    } catch {
      feedback.textContent = t("detail.copyFailed");
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
        ? t("detail.responseUnknown")
        : request.streaming
          ? t("field.streaming")
          : t("detail.nonStreaming"),
    ),
    copy,
  );
  const inspector = element("section", "detail-inspector"),
    nav = element("div", "detail-tabs"),
    panel = element("div", "detail-tab-panel");
  nav.setAttribute("role", "tablist");
  nav.setAttribute("aria-label", t("detail.categories"));
  panel.id = "detail-tab-panel";
  panel.setAttribute("role", "tabpanel");
  panel.tabIndex = 0;
  const tabs = [
    ["summary", t("tab.summary")],
    ["routing", t("tab.routing")],
    ["usage", t("tab.usage")],
    ["diagnostics", t("tab.diagnostics")],
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
      panel.append(detailSection(t("detail.raw"), detailJson(request)));
    else {
      const notice = key === "diagnostics" ? detailNotice(request) : null;
      if (notice) panel.append(notice);
      panel.append(
        detailSection(
          {
            routing: t("detail.routing"),
            usage: t("detail.tokenBreakdown"),
            diagnostics: t("detail.diagnostics"),
          }[key],
          detailFacts(request, detailFields()[key]),
        ),
      );
      if (key === "usage")
        panel.append(
          detailSection(t("detail.rawUsage"), detailJson(request.usage_json)),
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
  selectedDetail = null;
  detailError = null;
  target.replaceChildren(element("p", "empty", t("detail.loading")));
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
    if (version === detailVersion) detailError = error;
    if (version === detailVersion && $("detail").open)
      target.replaceChildren(
        element(
          "p",
          "empty",
          t("detail.readError", { message: errorText(error) }),
        ),
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
    const controls = $("range-controls");
    if (range === "custom" && controls.scrollWidth > controls.clientWidth)
      controls.scrollLeft +=
        button.getBoundingClientRect().left -
        controls.getBoundingClientRect().left;
    else controls.scrollLeft = 0;
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
createFilter("provider", "filter.allProvider", false);
createFilter("model", "filter.allModel", true);
createFilter("path", "filter.allPath", true);
createFilter(
  "outcome",
  "filter.allOutcome",
  false,
  [
    "completed",
    "failed",
    "incomplete",
    "cancelled",
    "interrupted",
    "in_progress",
  ].map((value) => ({ value, labelKey: `status.${value}` })),
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
  selectedDetail = null;
  detailError = null;
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

function renderLanguage() {
  configureFormatters();
  activeFilter?.close();
  applyTranslations();
  for (const button of $("language-switch").children)
    button.setAttribute(
      "aria-pressed",
      String(button.dataset.language === language()),
    );
  for (const widget of filterWidgets.values()) widget.localize();
  // Rerender existing results, preserving filters, range, pagination and requests.
  if (data) render();
  setText("connection", t(`connection.${connectionState}`));
  if (data)
    setText(
      "updated",
      t("connection.updated", { time: date(data.generated_at_ms) }),
    );
  $("updated").title = t("common.timezone", {
    zone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  });
  if (dashboardError) {
    notify(
      dashboardError instanceof UiError && dashboardError.key === "error.range"
        ? errorText(dashboardError)
        : t("error.read", {
            message: errorText(dashboardError),
            retained: data ? t("error.retained") : "",
          }),
    );
    if (!data) $("requests").replaceChildren(emptyRow(t("error.noRecords")));
  }
  if ($("detail").open) {
    if (selectedDetail) {
      const tab = document.querySelector(".detail-tabs [aria-selected=true]")
        ?.dataset.tab;
      const panelScroll =
        document.querySelector(".detail-tab-panel")?.scrollTop || 0;
      const processScroll =
        document.querySelector(".detail-process")?.scrollTop || 0;
      renderDetail(selectedDetail);
      if (tab) document.querySelector(`[data-tab="${tab}"]`)?.click();
      document.querySelector(".detail-tab-panel").scrollTop = panelScroll;
      document.querySelector(".detail-process").scrollTop = processScroll;
      $("detail-feedback").textContent = "";
    } else
      $("detail-content").replaceChildren(
        element(
          "p",
          "empty",
          detailError
            ? t("detail.readError", { message: errorText(detailError) })
            : t("detail.loading"),
        ),
      );
  }
}
for (const [code, definition] of Object.entries(languages)) {
  const button = element("button", "", definition.label);
  button.type = "button";
  button.lang = definition.locale;
  button.dataset.language = code;
  button.addEventListener("click", () => setLanguage(code));
  $("language-switch").append(button);
}
window.addEventListener("transit:languagechange", renderLanguage);
renderLanguage();
load();
