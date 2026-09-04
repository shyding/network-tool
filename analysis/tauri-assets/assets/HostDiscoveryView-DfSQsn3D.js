import {
  a as O,
  B as us,
  o as rs,
  t as E,
  p as D,
  b as cs,
  c as d,
  e as s,
  w as V,
  x as ss,
  y as N,
  v as ds,
  f as a,
  h as F,
  j as U,
  u as K,
  N as vs,
  n as W,
  F as L,
  i as B,
  k as ps,
  s as I,
  r as m,
  l as C,
  m as c,
  g as es,
  A as ms,
  _ as H,
  q,
} from "./index-DNL4tSY9.js";
import { P as ys } from "./PageTabs-S7SJq5-f.js";
import {
  u as _s,
  a as bs,
  s as Z,
  i as hs,
} from "./useThemeColors-DEU6HLyc.js";
import { i as gs } from "./install-6XjsQTvv.js";
import { i as fs } from "./install-BpiDeX-u.js";
import { i as ks } from "./install-B1KIHPBh.js";
import { i as ws } from "./install-CIgk3cSO.js";
import "./LegendVisualProvider-Bdgkckq-.js";
import "./createSeriesDataSimply-Cz60qhMH.js";
import "./sectorHelper-BvK_qgfw.js";
import "./dataSample-Cqp3lA6C.js";
import "./sausage-CoObHdFv.js";
const xs = { class: "scan" },
  $s = { class: "panel ctrl" },
  Cs = { class: "row" },
  Ms = ["disabled"],
  Ss = ["disabled"],
  Ps = ["disabled"],
  As = ["disabled"],
  Vs = { class: "actions" },
  Is = ["disabled"],
  Ls = { key: 0, class: "err" },
  Us = { class: "prog" },
  Bs = { class: "metrics" },
  Ts = { class: "metric" },
  Ns = { class: "metric" },
  zs = { class: "ok" },
  Ds = { class: "metric" },
  Es = { class: "metric" },
  Ws = { class: "filters" },
  Fs = ["disabled", "onClick"],
  Os = { class: "body" },
  Hs = { class: "panel card wall" },
  Rs = { key: 0, class: "empty" },
  js = { key: 1, class: "cards" },
  Ks = ["onClick"],
  qs = { class: "h-top" },
  Gs = { class: "ip" },
  Js = { class: "role-tag" },
  Qs = { class: "h-name" },
  Xs = { class: "h-meta" },
  Ys = { class: "h-mac" },
  Zs = { class: "mid" },
  se = { class: "panel card" },
  ee = { class: "panel card" },
  te = { key: 0, class: "empty" },
  le = { class: "panel card detail" },
  oe = { class: "d-name" },
  ae = { class: "d-tags" },
  ne = { key: 0, class: "tag warn" },
  ie = { class: "tag mute" },
  ue = { class: "tag mute" },
  re = { class: "d-block" },
  ce = { class: "v" },
  de = { class: "d-block" },
  ve = { class: "v" },
  pe = { key: 1, class: "empty" },
  me = O({
    __name: "ScanPanel",
    props: { hosts: {}, summary: {} },
    emits: ["update:hosts", "update:summary", "scanned"],
    setup(x, { emit: f }) {
      _s([gs, fs, ks, ws, hs]);
      const y = x,
        p = f,
        _ = m("192.168.1.0/24"),
        w = m("tcp"),
        $ = m(50),
        b = m(2e3),
        l = m(!1),
        fullScanActive = m(!1),
        u = m(null),
        o = m([]),
        r = m(""),
        i = m(null),
        k = m("all"),
        S = m(null),
        { colors: A } = bs(),
        h = {
          gateway: { label: "网关", color: "#fbbf24" },
          nas: { label: "存储", color: "#38bdf8" },
          printer: { label: "打印机", color: "#a78bfa" },
          camera: { label: "监控", color: "#fb7185" },
          network: { label: "网络设备", color: "#2dd4bf" },
          host: { label: "主机", color: "#94a3b8" },
        };
      let P = [],
        g = m([]);
      us(
        () => y.hosts,
        (t) => {
          g.value = t;
        },
        { immediate: !0 },
      );
      const T = C(() => {
          var t;
          return (
            ((t = u.value) == null ? void 0 : t.percent) ??
            (y.summary ? 100 : 0)
          );
        }),
        G = C(() => {
          var t, e;
          return (
            ((t = u.value) == null ? void 0 : t.alive_count) ??
            ((e = y.summary) == null ? void 0 : e.alive_count) ??
            g.value.length
          );
        }),
        ts = C(() => {
          var t, e;
          return (
            ((t = u.value) == null ? void 0 : t.scanned) ??
            ((e = y.summary) == null ? void 0 : e.scanned) ??
            0
          );
        }),
        ls = C(() => {
          var t, e;
          return (
            ((t = u.value) == null ? void 0 : t.total) ??
            ((e = y.summary) == null ? void 0 : e.total) ??
            0
          );
        }),
        z = C(() =>
          k.value === "all"
            ? g.value
            : g.value.filter((t) => t.role === k.value),
        ),
        J = C(() =>
          Object.keys(h).map((t) => ({
            key: t,
            ...h[t],
            count: g.value.filter((e) => e.role === t).length,
          })),
        ),
        os = C(() => {
          const t = A.value,
            e = J.value
              .filter((v) => v.count > 0)
              .map((v) => ({
                name: v.label,
                value: v.count,
                itemStyle: { color: v.color },
              }));
          return (
            e.length ||
              e.push({
                name: "暂无",
                value: 1,
                itemStyle: { color: "rgba(148,163,184,0.25)" },
              }),
            {
              backgroundColor: "transparent",
              tooltip: { trigger: "item" },
              series: [
                {
                  type: "pie",
                  radius: ["40%", "68%"],
                  center: ["50%", "52%"],
                  label: { color: t.text, fontSize: 11 },
                  data: e,
                },
              ],
            }
          );
        }),
        as = C(() => {
          const t = A.value,
            e = [...z.value]
              .filter((v) => v.response_time_ms != null)
              .sort(
                (v, M) => (M.response_time_ms ?? 0) - (v.response_time_ms ?? 0),
              )
              .slice(0, 14);
          return {
            backgroundColor: "transparent",
            tooltip: {
              trigger: "axis",
              backgroundColor: t.tooltipBg,
              borderColor: t.tooltipBorder,
              textStyle: { color: t.text, fontSize: 12 },
            },
            grid: { left: 8, right: 12, top: 8, bottom: 28, containLabel: !0 },
            xAxis: {
              type: "category",
              data: e.map((v) => v.ip.split(".").pop()),
              axisLabel: { color: t.muted, fontSize: 10 },
              axisLine: { lineStyle: { color: t.axis } },
            },
            yAxis: {
              type: "value",
              axisLabel: { color: t.muted, fontSize: 10 },
              splitLine: { lineStyle: { color: t.split } },
            },
            series: [
              {
                type: "bar",
                data: e.map((v) => {
                  var M;
                  return {
                    value: v.response_time_ms,
                    itemStyle: {
                      color:
                        ((M = h[v.role]) == null ? void 0 : M.color) ||
                        "#94a3b8",
                      borderRadius: [3, 3, 0, 0],
                    },
                  };
                }),
                barMaxWidth: 18,
              },
            ],
          };
        });
      function Q(t) {
        var e;
        return ((e = h[t]) == null ? void 0 : e.label) || t;
      }
      function R(t) {
        var e;
        return ((e = h[t]) == null ? void 0 : e.color) || "#94a3b8";
      }
      async function X() {
        if (l.value) return;
        const t = _.value.trim();
        if (!t) {
          r.value = "请输入网络范围";
          return;
        }
        if (t.includes("/") && !/^\d/.test(t)) {
          r.value = "CIDR 格式无效";
          return;
        }
        ((r.value = ""),
          (l.value = !0),
          (fullScanActive.value = !1),
          (g.value = []),
          p("update:hosts", []),
          p("update:summary", null),
          (u.value = null),
          (i.value = null),
          (o.value = []));
        try {
          const e = await E("start_host_discovery", {
            network: t,
            scanMethod: w.value,
            threads: $.value,
            timeoutMs: b.value,
          });
          (p("update:summary", e),
            p("update:hosts", e.hosts),
            (g.value = e.hosts),
            (fullScanActive.value = !!e.port_scan_running),
            p("scanned"));
        } catch (e) {
          r.value = String(e);
        } finally {
          l.value = !1;
        }
      }
      async function Y() {
        try {
          await E("stop_host_discovery");
        } catch {}
        fullScanActive.value = !1;
      }
      function ns() {
        l.value ||
          ((g.value = []),
          p("update:hosts", []),
          p("update:summary", null),
          (u.value = null),
          (i.value = null),
          (o.value = []),
          (r.value = ""));
      }
      async function is() {
        (await ms(), S.value && (S.value.scrollTop = S.value.scrollHeight));
      }
      return (
        rs(async () => {
          try {
            const t = await E("list_segment_ifaces"),
              e =
                t.find((v) => v.is_lan_primary) ||
                t.find((v) => v.kind === "ethernet" || v.kind === "wifi") ||
                t[0];
            e != null && e.network && (_.value = e.network);
          } catch {}
          (P.push(
            await D("hostdisc:line", (t) => {
              (o.value.push(t.payload),
                o.value.length > 400 && o.value.shift(),
                is());
            }),
          ),
            P.push(
              await D("hostdisc:host", (t) => {
                const e = t.payload,
                  v = g.value.findIndex((M) => M.ip === e.ip);
                (v >= 0
                  ? (g.value[v] = {
                      ...g.value[v],
                      ...e,
                      hostname: e.hostname ?? g.value[v].hostname,
                      mac: e.mac ?? g.value[v].mac,
                    })
                  : (g.value.push(e),
                    g.value.sort((M, n) =>
                      M.ip.localeCompare(n.ip, void 0, { numeric: !0 }),
                    )),
                  i.value &&
                    i.value.ip === e.ip &&
                    (i.value = g.value.find((M) => M.ip === e.ip) || e),
                  (fullScanActive.value = g.value.some((M) =>
                    ["queued", "scanning"].includes(M.port_scan?.state),
                  )),
                  p("update:hosts", [...g.value]),
                  i.value || (i.value = e));
              }),
            ),
            P.push(
              await D("hostdisc:progress", (t) => {
                u.value = t.payload;
              }),
            ),
            P.push(
              await D("hostdisc:complete", (t) => {
                (p("update:summary", t.payload),
                  p("update:hosts", t.payload.hosts),
                  (g.value = t.payload.hosts),
                  (fullScanActive.value = !!t.payload.port_scan_running),
                  p("scanned"));
              }),
            ));
        }),
        cs(() => {
          (P.forEach((t) => t()), (l.value || fullScanActive.value) && Y());
        }),
        (t, e) => {
          var v, M;
          return (
            c(),
            d("div", xs, [
              s("section", $s, [
                s("div", Cs, [
                  e[6] || (e[6] = s("label", null, "网段", -1)),
                  V(
                    s(
                      "input",
                      {
                        "onUpdate:modelValue":
                          e[0] || (e[0] = (n) => (_.value = n)),
                        class: "inp grow",
                        disabled: l.value,
                        onKeyup: ss(X, ["enter"]),
                      },
                      null,
                      40,
                      Ms,
                    ),
                    [[N, _.value]],
                  ),
                  e[7] || (e[7] = s("label", null, "方式", -1)),
                  V(
                    s(
                      "select",
                      {
                        "onUpdate:modelValue":
                          e[1] || (e[1] = (n) => (w.value = n)),
                        class: "inp sel",
                        disabled: l.value,
                      },
                      [
                        ...(e[5] ||
                          (e[5] = [
                            s("option", { value: "tcp" }, "TCP（推荐）", -1),
                            s("option", { value: "ping" }, "Ping", -1),
                            s("option", { value: "hybrid" }, "混合", -1),
                          ])),
                      ],
                      8,
                      Ss,
                    ),
                    [[ds, w.value]],
                  ),
                  e[8] || (e[8] = s("label", null, "线程", -1)),
                  V(
                    s(
                      "input",
                      {
                        "onUpdate:modelValue":
                          e[2] || (e[2] = (n) => ($.value = n)),
                        class: "inp num",
                        type: "number",
                        disabled: l.value,
                      },
                      null,
                      8,
                      Ps,
                    ),
                    [[N, $.value, void 0, { number: !0 }]],
                  ),
                  e[9] || (e[9] = s("label", null, "超时", -1)),
                  V(
                    s(
                      "input",
                      {
                        "onUpdate:modelValue":
                          e[3] || (e[3] = (n) => (b.value = n)),
                        class: "inp num",
                        type: "number",
                        disabled: l.value,
                      },
                      null,
                      8,
                      As,
                    ),
                    [[N, b.value, void 0, { number: !0 }]],
                  ),
                  e[10] || (e[10] = s("span", { class: "unit" }, "ms", -1)),
                  s("div", Vs, [
                    s(
                      "button",
                      {
                        type: "button",
                        class: "btn-ghost",
                        disabled: l.value,
                        onClick: ns,
                      },
                      "清空",
                      8,
                      Is,
                    ),
                    l.value || fullScanActive.value
                      ? (c(),
                        d(
                          "button",
                          {
                            key: 0,
                            type: "button",
                            class: "btn-ghost",
                            onClick: Y,
                          },
                          l.value ? "停止主机扫描" : "停止端口扫描",
                          1,
                        ))
                      : (c(),
                        d(
                          "button",
                          {
                            key: 1,
                            type: "button",
                            class: "btn-primary",
                            onClick: X,
                          },
                          "开始扫描",
                        )),
                  ]),
                ]),
                r.value ? (c(), d("p", Ls, a(r.value), 1)) : F("", !0),
              ]),
              s("section", Us, [
                U(
                  K(vs),
                  {
                    type: "line",
                    percentage: Math.min(100, Math.round(T.value)),
                    height: 8,
                    "border-radius": 4,
                    processing: l.value,
                    "show-indicator": !1,
                  },
                  null,
                  8,
                  ["percentage", "processing"],
                ),
              ]),
              s("section", Bs, [
                s("div", Ts, [
                  e[11] || (e[11] = s("span", null, "进度", -1)),
                  s("b", null, a(Math.round(T.value)) + "%", 1),
                  s("small", null, a(ts.value) + "/" + a(ls.value || "—"), 1),
                ]),
                s("div", Ns, [
                  e[12] || (e[12] = s("span", null, "存活", -1)),
                  s("b", zs, a(G.value), 1),
                ]),
                s("div", Ds, [
                  e[13] || (e[13] = s("span", null, "网关", -1)),
                  s(
                    "b",
                    null,
                    a(
                      ((M = (v = x.summary) == null ? void 0 : v.gateway) ==
                      null
                        ? void 0
                        : M.ip) || "—",
                    ),
                    1,
                  ),
                ]),
                s("div", Es, [
                  e[14] || (e[14] = s("span", null, "耗时", -1)),
                  s(
                    "b",
                    null,
                    a(
                      x.summary
                        ? `${(x.summary.elapsed_ms / 1e3).toFixed(1)}s`
                        : "—",
                    ),
                    1,
                  ),
                ]),
              ]),
              s("div", Ws, [
                s(
                  "button",
                  {
                    type: "button",
                    class: W(["fchip", { on: k.value === "all" }]),
                    onClick: e[4] || (e[4] = (n) => (k.value = "all")),
                  },
                  " 全部 " + a(G.value),
                  3,
                ),
                (c(!0),
                d(
                  L,
                  null,
                  B(
                    J.value,
                    (n) => (
                      c(),
                      d(
                        "button",
                        {
                          key: n.key,
                          type: "button",
                          class: W([
                            "fchip",
                            { on: k.value === n.key, dim: n.count === 0 },
                          ]),
                          style: I({ "--c": n.color }),
                          disabled: n.count === 0 && k.value !== n.key,
                          onClick: (j) => (k.value = n.key),
                        },
                        [
                          s(
                            "i",
                            { style: I({ background: n.color }) },
                            null,
                            4,
                          ),
                          es(" " + a(n.label) + " " + a(n.count), 1),
                        ],
                        14,
                        Fs,
                      )
                    ),
                  ),
                  128,
                )),
              ]),
              s("div", Os, [
                s("section", Hs, [
                  e[15] ||
                    (e[15] = s(
                      "div",
                      { class: "sec" },
                      "存活主机 · 点击查看详情",
                      -1,
                    )),
                  z.value.length
                    ? (c(),
                      d("div", js, [
                        (c(!0),
                        d(
                          L,
                          null,
                          B(z.value, (n) => {
                            var j;
                            return (
                              c(),
                              d(
                                "button",
                                {
                                  key: n.ip,
                                  type: "button",
                                  class: W([
                                    "host",
                                    {
                                      on:
                                        ((j = i.value) == null
                                          ? void 0
                                          : j.ip) === n.ip,
                                      gw: n.is_gateway,
                                    },
                                  ]),
                                  style: I({ "--accent": R(n.role) }),
                                  onClick: (Oe) => (i.value = n),
                                },
                                [
                                  s("div", qs, [
                                    s("span", Gs, a(n.ip), 1),
                                    s("span", Js, a(Q(n.role)), 1),
                                  ]),
                                  s(
                                    "div",
                                    Qs,
                                    a(n.hostname || "未知主机名"),
                                    1,
                                  ),
                                  s("div", Xs, [
                                    s(
                                      "span",
                                      null,
                                      a(
                                        n.response_time_ms != null
                                          ? `${n.response_time_ms}ms`
                                          : "—",
                                      ),
                                      1,
                                    ),
                                    s("span", null, a(n.method_hit || "—"), 1),
                                  ]),
                                  s(
                                    "div",
                                    Ys,
                                    a(n.mac || "无 MAC") +
                                      " · " +
                                      a(n.vendor || "未知厂商"),
                                    1,
                                  ),
                                ],
                                14,
                                Ks,
                              )
                            );
                          }),
                          128,
                        )),
                      ]))
                    : (c(),
                      d(
                        "div",
                        Rs,
                        a(
                          l.value
                            ? "扫描中，发现存活主机后实时展示…"
                            : "开始扫描后显示主机卡片",
                        ),
                        1,
                      )),
                ]),
                s("div", Zs, [
                  s("section", se, [
                    e[16] ||
                      (e[16] = s("div", { class: "sec" }, "角色分布", -1)),
                    U(
                      K(Z),
                      { class: "ech", option: os.value, autoresize: "" },
                      null,
                      8,
                      ["option"],
                    ),
                  ]),
                  s("section", ee, [
                    e[17] ||
                      (e[17] = s(
                        "div",
                        { class: "sec" },
                        "时延（当前筛选）",
                        -1,
                      )),
                    z.value.length
                      ? (c(),
                        ps(
                          K(Z),
                          {
                            key: 1,
                            class: "ech",
                            option: as.value,
                            autoresize: "",
                          },
                          null,
                          8,
                          ["option"],
                        ))
                      : (c(), d("div", te, "—")),
                  ]),
                ]),
                s("section", le, [
                  e[20] || (e[20] = s("div", { class: "sec" }, "主机详情", -1)),
                  i.value
                    ? (c(),
                      d(
                        L,
                        { key: 0 },
                        [
                          s(
                            "div",
                            {
                              class: "d-ip",
                              style: I({ color: R(i.value.role) }),
                            },
                            a(i.value.ip),
                            5,
                          ),
                          s("div", oe, a(i.value.hostname || "未知主机名"), 1),
                          s("div", ae, [
                            s(
                              "span",
                              {
                                class: "tag",
                                style: I({ background: R(i.value.role) }),
                              },
                              a(Q(i.value.role)),
                              5,
                            ),
                            i.value.is_gateway
                              ? (c(), d("span", ne, "默认网关"))
                              : F("", !0),
                            s("span", ie, a(i.value.method_hit || "—"), 1),
                            s(
                              "span",
                              ue,
                              a(i.value.response_time_ms ?? "—") + " ms",
                              1,
                            ),
                          ]),
                          s("div", re, [
                            e[18] ||
                              (e[18] = s("div", { class: "k" }, "MAC", -1)),
                            s("div", ce, a(i.value.mac || "—"), 1),
                          ]),
                          s("div", de, [
                            e[19] ||
                              (e[19] = s("div", { class: "k" }, "厂商", -1)),
                            s("div", ve, a(i.value.vendor || "未知"), 1),
                          ]),
                          s("div", { class: "port-block" }, [
                            s("div", { class: "port-head" }, [
                              s("span", null, "全量端口 1–65535"),
                              s(
                                "b",
                                null,
                                a(
                                  i.value.port_scan?.state === "complete"
                                    ? "扫描完成"
                                    : i.value.port_scan?.state === "cancelled"
                                      ? "已停止"
                                      : i.value.port_scan?.state === "queued"
                                        ? "等待扫描"
                                        : "后台扫描中",
                                ),
                                1,
                              ),
                            ]),
                            s("div", { class: "port-progress" }, [
                              s(
                                "i",
                                {
                                  style: I({
                                    width: `${Math.min(100, i.value.port_scan?.percent || 0)}%`,
                                  }),
                                },
                                null,
                                4,
                              ),
                            ]),
                            s(
                              "div",
                              { class: "port-summary" },
                              `已扫 ${(i.value.port_scan?.scanned || 0).toLocaleString()} / 65,535 · 开放 ${i.value.open_ports?.length || 0} 个`,
                              1,
                            ),
                            s("div", { class: "port-list" }, [
                              Array.isArray(i.value.open_ports) &&
                              i.value.open_ports.length
                                ? (c(!0),
                                  d(
                                    L,
                                    null,
                                    B(i.value.open_ports, (n) =>
                                      (c(),
                                      d(
                                        n.url ? "a" : "div",
                                        {
                                          key: n.port ?? n,
                                          class: "port-row",
                                          href: n.url || void 0,
                                          title: n.url
                                            ? `已确认 HTTP 服务，点击用浏览器打开 ${n.url}`
                                            : n.evidence || "端口开放，协议未确认",
                                          target: n.url ? "_blank" : void 0,
                                          rel: n.url ? "noreferrer" : void 0,
                                          onClick: n.url
                                            ? (t) => {
                                                (t.preventDefault(),
                                                  E("plugin:opener|open_url", {
                                                    url: n.url,
                                                  }).catch((t) => {
                                                    (o.value.push(
                                                      `浏览器打开失败：${String(t)}`,
                                                    ),
                                                      is());
                                                  }));
                                              }
                                            : void 0,
                                        },
                                        [
                                          s("strong", null, a(n.port ?? n), 1),
                                          s(
                                            "span",
                                            null,
                                            a(n.service || "开放端口"),
                                            1,
                                          ),
                                          s(
                                            "small",
                                            null,
                                            a(
                                              `${n.category || "其他"} · ${n.verified ? "已确认" : "端口推断"}`,
                                            ),
                                            1,
                                          ),
                                        ],
                                        8,
                                        [
                                          "href",
                                          "title",
                                          "target",
                                          "rel",
                                          "onClick",
                                        ],
                                      )),
                                    ),
                                    128,
                                  ))
                                : s(
                                    "div",
                                    { class: "port-empty" },
                                    i.value.port_scan?.state === "complete"
                                      ? "未发现开放 TCP 端口"
                                      : "优先检测 HTTP / MySQL / FTP / SSH…",
                                  ),
                            ]),
                          ]),
                        ],
                        64,
                      ))
                    : (c(), d("div", pe, "选择左侧主机卡片")),
                  e[21] ||
                    (e[21] = s(
                      "div",
                      { class: "sec log-sec" },
                      "实时日志",
                      -1,
                    )),
                  s(
                    "pre",
                    { ref_key: "logEl", ref: S, class: "log-body" },
                    a(
                      o.value.slice(-40).join(`
`) || "等待…",
                    ),
                    513,
                  ),
                ]),
              ]),
            ])
          );
        }
      );
    },
  }),
  ye = H(me, [["__scopeId", "data-v-3b6b7d21"]]),
  _e = { class: "topo" },
  be = { class: "summary" },
  he = { key: 0, class: "empty" },
  ge = { key: 1, class: "stage" },
  fe = { class: "edges", viewBox: "0 0 100 100", preserveAspectRatio: "none" },
  ke = ["x1", "y1", "x2", "y2"],
  we = { class: "label" },
  xe = { class: "legend" },
  $e = O({
    __name: "TopologyPanel",
    props: { hosts: {}, summary: {} },
    setup(x) {
      const f = x,
        y = {
          gateway: { label: "网关", color: "#fbbf24" },
          nas: { label: "存储", color: "#38bdf8" },
          printer: { label: "打印机", color: "#a78bfa" },
          camera: { label: "监控", color: "#fb7185" },
          network: { label: "网络设备", color: "#2dd4bf" },
          host: { label: "主机", color: "#94a3b8" },
        },
        p = C(() => {
          var o, r, i, k, S, A;
          const l = [...f.hosts],
            u =
              (r = (o = f.summary) == null ? void 0 : o.gateway) == null
                ? void 0
                : r.ip;
          return (
            u &&
              !l.some((h) => h.ip === u) &&
              l.unshift({
                ip: u,
                alive: !0,
                response_time_ms: null,
                hostname: "默认网关",
                mac:
                  ((k = (i = f.summary) == null ? void 0 : i.gateway) == null
                    ? void 0
                    : k.mac) ?? null,
                vendor:
                  ((A = (S = f.summary) == null ? void 0 : S.gateway) == null
                    ? void 0
                    : A.vendor) ?? null,
                role: "gateway",
                is_gateway: !0,
                method_hit: null,
              }),
            l.sort(
              (h, P) =>
                Number(P.is_gateway) - Number(h.is_gateway) ||
                h.ip.localeCompare(P.ip, void 0, { numeric: !0 }),
            ),
            l
          );
        }),
        _ = C(() => {
          const l = p.value;
          if (!l.length) return [];
          const u = l.findIndex((h) => h.is_gateway || h.role === "gateway"),
            o = u >= 0 ? l[u] : l[0],
            r = l.filter((h) => h.ip !== o.ip),
            i = 50,
            k = 48,
            S = [{ h: o, x: i, y: k, gw: !0 }],
            A = r.length || 1;
          return (
            r.forEach((h, P) => {
              const g = (Math.PI * 2 * P) / A - Math.PI / 2,
                T = A <= 6 ? 34 : A <= 14 ? 38 : 42;
              S.push({
                h,
                x: i + Math.cos(g) * T,
                y: k + Math.sin(g) * T * 0.92,
                gw: !1,
              });
            }),
            S
          );
        }),
        w = C(() => {
          const l = _.value.find((u) => u.gw);
          return l
            ? _.value
                .filter((u) => !u.gw)
                .map((u) => ({ x1: l.x, y1: l.y, x2: u.x, y2: u.y }))
            : [];
        });
      function $(l) {
        var u;
        return ((u = y[l]) == null ? void 0 : u.label) || l;
      }
      function b(l) {
        var u;
        return ((u = y[l]) == null ? void 0 : u.color) || "#94a3b8";
      }
      return (l, u) => (
        c(),
        d("div", _e, [
          s(
            "div",
            be,
            a(
              x.summary
                ? `${x.summary.network} · 存活 ${x.hosts.length} 台`
                : "请先在「扫描结果」完成主机发现",
            ),
            1,
          ),
          p.value.length
            ? (c(),
              d("div", ge, [
                (c(),
                d("svg", fe, [
                  (c(!0),
                  d(
                    L,
                    null,
                    B(
                      w.value,
                      (o, r) => (
                        c(),
                        d(
                          "line",
                          {
                            key: r,
                            x1: o.x1,
                            y1: o.y1,
                            x2: o.x2,
                            y2: o.y2,
                            class: "link",
                          },
                          null,
                          8,
                          ke,
                        )
                      ),
                    ),
                    128,
                  )),
                ])),
                (c(!0),
                d(
                  L,
                  null,
                  B(
                    _.value,
                    (o) => (
                      c(),
                      d(
                        "div",
                        {
                          key: o.h.ip,
                          class: W(["node", { gw: o.gw }]),
                          style: I({
                            left: `${o.x}%`,
                            top: `${o.y}%`,
                            "--c": b(o.h.role),
                          }),
                        },
                        [
                          u[0] || (u[0] = s("div", { class: "dot" }, null, -1)),
                          s("div", we, [
                            s("b", null, a(o.h.ip), 1),
                            s("span", null, a(o.h.hostname || $(o.h.role)), 1),
                            s("em", null, a($(o.h.role)), 1),
                          ]),
                        ],
                        6,
                      )
                    ),
                  ),
                  128,
                )),
              ]))
            : (c(), d("div", he, "完成扫描后自动生成局域网拓扑")),
          s("div", xe, [
            (c(),
            d(
              L,
              null,
              B(y, (o, r) =>
                s("span", { key: r }, [
                  s("i", { style: I({ background: o.color }) }, null, 4),
                  es(a(o.label), 1),
                ]),
              ),
              64,
            )),
          ]),
        ])
      );
    },
  }),
  Ce = H($e, [["__scopeId", "data-v-f04c291b"]]),
  Me = { class: "wol" },
  Se = { class: "panel form" },
  Pe = { class: "row" },
  Ae = ["disabled"],
  Ve = { key: 0, class: "err" },
  Ie = { key: 1, class: "ok" },
  Le = { class: "panel list" },
  Ue = { key: 0, class: "empty" },
  Be = { key: 1, class: "rows" },
  Te = ["onClick"],
  Ne = { class: "l" },
  ze = { class: "r" },
  De = O({
    __name: "WolPanel",
    props: { hosts: {} },
    setup(x) {
      const f = x,
        y = m(""),
        p = m("255.255.255.255"),
        _ = m(""),
        w = m(""),
        $ = m(!1),
        b = C(() => f.hosts.filter((o) => o.mac && o.mac.length >= 17));
      function l(o) {
        o.mac && (y.value = o.mac);
      }
      async function u() {
        if (((w.value = ""), (_.value = ""), !y.value.trim())) {
          w.value = "请填写 MAC 地址";
          return;
        }
        $.value = !0;
        try {
          (await E("send_wake_on_lan", {
            mac: y.value.trim(),
            broadcast: p.value.trim() || "255.255.255.255",
            port: 9,
          }),
            (_.value = `已发送魔术包 → ${y.value.trim()} @ ${p.value || "255.255.255.255"}:9/7`));
        } catch (o) {
          w.value = String(o);
        } finally {
          $.value = !1;
        }
      }
      return (o, r) => (
        c(),
        d("div", Me, [
          s("section", Se, [
            r[4] ||
              (r[4] = s(
                "div",
                { class: "sec" },
                "远程唤醒（Wake-on-LAN）",
                -1,
              )),
            s("div", Pe, [
              r[2] || (r[2] = s("label", null, "MAC", -1)),
              V(
                s(
                  "input",
                  {
                    "onUpdate:modelValue":
                      r[0] || (r[0] = (i) => (y.value = i)),
                    class: "inp grow",
                    placeholder: "AA-BB-CC-DD-EE-FF",
                    onKeyup: ss(u, ["enter"]),
                  },
                  null,
                  544,
                ),
                [[N, y.value]],
              ),
              r[3] || (r[3] = s("label", null, "广播", -1)),
              V(
                s(
                  "input",
                  {
                    "onUpdate:modelValue":
                      r[1] || (r[1] = (i) => (p.value = i)),
                    class: "inp mid",
                  },
                  null,
                  512,
                ),
                [[N, p.value]],
              ),
              s(
                "button",
                {
                  type: "button",
                  class: "btn-primary",
                  disabled: $.value,
                  onClick: u,
                },
                a($.value ? "发送中…" : "发送唤醒"),
                9,
                Ae,
              ),
            ]),
            w.value ? (c(), d("p", Ve, a(w.value), 1)) : F("", !0),
            _.value ? (c(), d("p", Ie, a(_.value), 1)) : F("", !0),
            r[5] ||
              (r[5] = s(
                "p",
                { class: "hint" },
                "向广播地址发送 UDP 魔术包（端口 9 / 7）。目标需支持 WOL 且与本机同二层网络。",
                -1,
              )),
          ]),
          s("section", Le, [
            r[6] ||
              (r[6] = s("div", { class: "sec" }, "来自最近扫描（含 MAC）", -1)),
            b.value.length
              ? (c(),
                d("div", Be, [
                  (c(!0),
                  d(
                    L,
                    null,
                    B(
                      b.value,
                      (i) => (
                        c(),
                        d(
                          "button",
                          {
                            key: i.ip,
                            type: "button",
                            class: "row-item",
                            onClick: (k) => l(i),
                          },
                          [
                            s("div", Ne, [
                              s("b", null, a(i.ip), 1),
                              s("span", null, a(i.hostname || "未知"), 1),
                            ]),
                            s("div", ze, [
                              s("code", null, a(i.mac), 1),
                              s("small", null, a(i.vendor || "未知厂商"), 1),
                            ]),
                          ],
                          8,
                          Te,
                        )
                      ),
                    ),
                    128,
                  )),
                ]))
              : (c(), d("div", Ue, "先扫描网段，带 MAC 的主机会出现在这里")),
          ]),
        ])
      );
    },
  }),
  Ee = H(De, [["__scopeId", "data-v-337170b5"]]),
  We = { class: "page hd" },
  Fe = O({
    __name: "HostDiscoveryView",
    setup(x) {
      const f = m("scan"),
        y = [
          { key: "scan", label: "扫描结果" },
          { key: "topo", label: "局域网拓扑" },
          { key: "wol", label: "远程唤醒" },
        ],
        p = m([]),
        _ = m(null);
      function w() {}
      return ($, b) => (
        c(),
        d("div", We, [
          b[3] ||
            (b[3] = s(
              "header",
              { class: "head" },
              [
                s("div", null, [
                  s("div", { class: "page-title" }, "主机发现"),
                  s(
                    "div",
                    { class: "page-sub" },
                    "Ping/TCP/混合探测 · MAC 与厂商识别 · 拓扑视图 · 网络唤醒(WOL)",
                  ),
                ]),
              ],
              -1,
            )),
          U(
            ys,
            {
              modelValue: f.value,
              "onUpdate:modelValue": b[0] || (b[0] = (l) => (f.value = l)),
              tabs: y,
            },
            null,
            8,
            ["modelValue"],
          ),
          V(
            U(
              ye,
              {
                hosts: p.value,
                "onUpdate:hosts": b[1] || (b[1] = (l) => (p.value = l)),
                summary: _.value,
                "onUpdate:summary": b[2] || (b[2] = (l) => (_.value = l)),
                onScanned: w,
              },
              null,
              8,
              ["hosts", "summary"],
            ),
            [[q, f.value === "scan"]],
          ),
          V(
            U(Ce, { hosts: p.value, summary: _.value }, null, 8, [
              "hosts",
              "summary",
            ]),
            [[q, f.value === "topo"]],
          ),
          V(U(Ee, { hosts: p.value }, null, 8, ["hosts"]), [
            [q, f.value === "wol"],
          ]),
        ])
      );
    },
  }),
  et = H(Fe, [["__scopeId", "data-v-de3c7077"]]);
export { et as default };
