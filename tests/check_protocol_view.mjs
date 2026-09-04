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
  export const e = (tag, props, children) => ({ tag, props, children });
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
  return { tree, source };
}

const { tree: v3Tree } = await renderComponent("ProtocolWorkbenchViewV3.js");
const v3ClassNames = String(v3Tree.props?.class ?? "").split(/\s+/);
if (!v3ClassNames.includes("protocol-workbench-v3")) throw new Error("V3 workbench root class missing");

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
