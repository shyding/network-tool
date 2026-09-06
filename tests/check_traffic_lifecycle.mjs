import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const source = fs.readFileSync(new URL("../analysis/tauri-assets/assets/TrafficMonitorView-5gSYrBJv.js", import.meta.url), "utf8");
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};
const tick = async () => { for (let i = 0; i < 16; i++) await Promise.resolve(); };

async function setup(invoke) {
  const hooks = { mounted: [], unmounted: [] };
  const timers = new Map();
  let id = 0;
  const context = vm.createContext({
    __hooks: hooks, __invoke: invoke,
    setInterval: callback => { timers.set(++id, callback); return id; },
    clearInterval: key => timers.delete(key),
  });
  const main = new vm.SourceTextModule(`
    export const a = value => value, B = () => {}, o = cb => __hooks.mounted.push(cb), b = cb => __hooks.unmounted.push(cb);
    export const r = value => ({value}), t = (name, args) => __invoke(name, args), l = cb => ({get value() {return cb();}}), _ = value => value;
    export const c = () => {}, e = c, n = c, f = c, w = c, v = c, F = c, i = c, h = c, j = c, u = c, m = c;
  `, { context });
  const theme = new vm.SourceTextModule("export const u=()=>{}, a=()=>({colors:{value:{}}}), s={}, i={};", { context });
  const plugin = new vm.SourceTextModule("export const i={};", { context });
  for (const module of [main, theme, plugin]) { await module.link(() => {}); await module.evaluate(); }
  const module = new vm.SourceTextModule(source, { context });
  await module.link(name => name.includes("index-DNL4") ? main : name.includes("useThemeColors") ? theme : plugin);
  await module.evaluate();
  module.namespace.default.setup();
  return { timers, mount: () => hooks.mounted[0](), unmount: () => hooks.unmounted[0]() };
}

{
  const query = deferred();
  const calls = [];
  const app = await setup(name => { calls.push(name); return query.promise; });
  const mounting = app.mount();
  app.unmount();
  query.resolve([]);
  await mounting;
  assert.deepEqual(calls, ["list_traffic_ifaces"]);
  assert.equal(app.timers.size, 0);
}

{
  const query = deferred();
  const app = await setup(name => name === "list_traffic_ifaces" ? [] : name === "sample_traffic" ? query.promise : true);
  const mounting = app.mount();
  await tick();
  app.unmount();
  query.resolve({ ifaces: [], sample: null });
  await mounting;
  assert.equal(app.timers.size, 0);
}

{
  const query = deferred();
  let samples = 0;
  const app = await setup(name => {
    if (name === "list_traffic_ifaces") return [];
    if (name === "sample_traffic") return ++samples === 1 ? { ifaces: [], sample: null } : query.promise;
    return true;
  });
  await app.mount();
  assert.equal(app.timers.size, 1);
  const poll = [...app.timers.values()][0];
  poll(); poll(); poll();
  await tick();
  assert.equal(samples, 2);
  app.unmount();
  query.resolve({ ifaces: [], sample: null });
  await tick();
  assert.equal(app.timers.size, 0);
}

{
  const app = await setup(name => {
    if (name === "reset_traffic_baseline") throw new Error("query failed");
    return [];
  });
  await app.mount();
  assert.equal(app.timers.size, 0);
  app.unmount();
}

console.log("Traffic lifecycle: PASS (late initialization, unmount, overlapping polls, failed start)");
