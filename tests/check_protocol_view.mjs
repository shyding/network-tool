import fs from "node:fs";
import vm from "node:vm";

const context = vm.createContext({
  console,
  Date,
  Math,
  Number,
  String,
  Promise,
  setInterval: () => 1,
  clearInterval: () => {},
});

const hooks = { mounted: [], unmounted: [] };
const dependency = new vm.SourceTextModule(
  `
  export const a = value => value;
  export const r = value => ({ value });
  export const o = callback => globalThis.__hooks.mounted.push(callback);
  export const b = callback => globalThis.__hooks.unmounted.push(callback);
  export const e = (tag, props, children) => ({
    tag,
    props,
    children: children && !Array.isArray(children) && typeof children === "object" ? null : children,
  });
  export const t = async () => ({});
  export const p = async () => () => {};
  `,
  { context },
);
context.__hooks = hooks;

await dependency.link(async specifier => {
  throw new Error(`Unexpected dependency module import: ${specifier}`);
});
await dependency.evaluate();

async function renderComponent(filename) {
  const source = fs.readFileSync(
    new URL(`../analysis/tauri-assets/assets/${filename}`, import.meta.url),
    "utf8",
  );
  const module = new vm.SourceTextModule(source, { context, identifier: filename });
  await module.link(async specifier => {
    if (specifier === "./index-DNL4tSY9.js") return dependency;
    throw new Error(`Unexpected dependency in ${filename}: ${specifier}`);
  });
  await module.evaluate();
  const component = module.namespace.default;
  if (!component || typeof component.setup !== "function") {
    throw new Error(`${filename} default export is not a Vue setup component`);
  }
  const render = component.setup();
  if (typeof render !== "function") throw new Error(`${filename} setup() did not return render()`);
  const tree = render();
  if (!tree || tree.tag !== "div") throw new Error(`${filename} first render did not produce a root div`);
  return { tree, source, render };
}

function nodeText(node) {
  if (node == null) return "";
  if (Array.isArray(node)) return node.map(nodeText).join(" ");
  if (typeof node === "object") return nodeText(node.children);
  return String(node);
}

function hasClass(node, className) {
  return String(node?.props?.class ?? "").split(/\s+/).includes(className);
}

function findNode(node, predicate) {
  if (node == null) return null;
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findNode(child, predicate);
      if (match) return match;
    }
    return null;
  }
  if (typeof node !== "object") return null;
  if (predicate(node)) return node;
  return findNode(node.children, predicate);
}

const { tree: v3Tree } = await renderComponent("ProtocolWorkbenchViewV3.js");
const v3ClassNames = String(v3Tree.props?.class ?? "").split(/\s+/);
if (!v3ClassNames.includes("protocol-workbench-v3")) throw new Error("V3 workbench root class missing");

const { tree: v4Tree, source: v4Source, render: v4Render } = await renderComponent("ProtocolWorkbenchViewV4.js");
const v4ClassNames = String(v4Tree.props?.class ?? "").split(/\s+/);
if (!v4ClassNames.includes("protocol-workbench-v4")) throw new Error("V4 workbench root class missing");
const v4Text = JSON.stringify(v4Tree);
for (const required of ["TCP Client", "TCP Server", "UDP Client", "UDP Server", "通信时间线", "连接参数", "发送报文", "暂无通信记录"]) {
  if (!v4Text.includes(required)) throw new Error(`missing V4 protocol section: ${required}`);
}
for (const required of ["Modbus TCP Client", "Modbus RTU Client", "Modbus TCP Server", "Modbus RTU Server"]) {
  if (!v4Source.includes(required)) throw new Error(`missing V4 protocol session: ${required}`);
}
for (const session of ["TCP Client", "TCP Server", "UDP Client", "UDP Server", "Modbus TCP Client", "Modbus TCP Server", "Modbus RTU Client", "Modbus RTU Server"]) {
  const button = findNode(v4Tree, node => node.tag === "button" && hasClass(node, "pw4-session") && nodeText(node).includes(session));
  if (!button) throw new Error(`missing V4 session navigation button: ${session}`);
  button.props.onClick();
  const workspace = findNode(v4Render(), node => hasClass(node, "pw4-workspace"));
  if (!workspace || !nodeText(workspace).includes(session)) {
    throw new Error(`V4 workspace did not render session panel: ${session}`);
  }
}
const v4Styles = fs.readFileSync(new URL("../analysis/tauri-assets/assets/protocol-workbench-v4.css", import.meta.url), "utf8");
if (!v4Styles.includes(".protocol-workbench-v4")) throw new Error("V4 stylesheet root selector missing");

const { tree: v2Tree, source: v2Source } = await renderComponent("ProtocolWorkbenchViewV2.js");
const v2ClassNames = String(v2Tree.props?.class ?? "").split(/\s+/);
if (!v2ClassNames.includes("modbus-poll-workbench")) throw new Error("V2 route workbench root class missing");
const routeText = JSON.stringify(v2Tree);
for (const required of ["TCP Client", "UDP Client", "一键 TCP 闭环"]) {
  if (!routeText.includes(required)) throw new Error(`missing protocol route section: ${required}`);
}
for (const required of ["TCP Server", "UDP Server"]) {
  if (!v2Source.includes(required)) throw new Error(`missing protocol server mode: ${required}`);
}
console.log("protocol view modules/setup/render: PASS");
