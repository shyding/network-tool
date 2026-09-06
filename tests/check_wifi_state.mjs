import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const source = fs.readFileSync(new URL("../analysis/tauri-assets/assets/wifi-scan.js", import.meta.url), "utf8");
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const tick = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };

async function setup(invoke) {
  const hooks = [];
  const timers = new Map();
  let next = 1;
  const context = vm.createContext({
    __invoke: invoke, __hooks: hooks,
    setTimeout: callback => { const id = next++; timers.set(id, callback); return id; },
    clearTimeout: id => timers.delete(id),
  });
  const dependency = new vm.SourceTextModule(`
    export const r = value => ({value});
    export const b = callback => __hooks.push(callback);
    export const t = (command, args) => __invoke(command, args);
  `, { context });
  await dependency.link(() => { throw new Error("Unexpected dependency"); });
  await dependency.evaluate();
  const module = new vm.SourceTextModule(source, { context });
  await module.link(() => dependency);
  await module.evaluate();
  return {
    state: module.namespace.useWifiScan(), timers,
    unmount: () => hooks.forEach(callback => callback()),
    timeout: () => [...timers.values()].forEach(callback => callback()),
  };
}

{
  const pending = deferred();
  let requests = 0;
  const app = await setup(() => { requests++; return pending.promise; });
  const first = app.state.scan();
  await app.state.scan();
  await tick();
  assert.equal(requests, 1);
  assert.equal(app.state.busy.value, true);
  app.timeout();
  await first;
  assert.equal(app.state.busy.value, false);
  assert.match(app.state.error.value, /超时/);
  assert.equal(app.timers.size, 0);
  pending.resolve({ aps: [{ ssid: "late" }] });
  await tick();
  assert.equal(app.state.result.value, null);
  await app.state.scan();
  assert.equal(app.state.result.value.aps[0].ssid, "late");
  app.unmount();
}

{
  const pending = deferred();
  const app = await setup(() => pending.promise);
  const scan = app.state.scan();
  await tick();
  app.unmount();
  await scan;
  assert.equal(app.state.busy.value, false);
  assert.equal(app.timers.size, 0);
  pending.resolve({ aps: [{ ssid: "stale" }] });
  await tick();
  assert.equal(app.state.result.value, null);
  assert.equal(app.state.error.value, "");
}

{
  const pending = deferred();
  let requests = 0;
  const app = await setup(() => { requests++; return pending.promise; });
  const refresh = app.state.refreshInterfaces();
  await app.state.refreshInterfaces();
  await app.state.scan();
  assert.equal(requests, 1);
  app.timeout();
  await refresh;
  assert.equal(app.state.refreshing.value, false);
  assert.match(app.state.error.value, /超时/);
  app.unmount();
}

{
  const app = await setup(async (command, args) => {
    if (command === "list_wifi_interfaces") return [{ name: "Wi-Fi A" }, { name: "Wi-Fi B" }];
    assert.equal(args.interface, "Wi-Fi B");
    return { aps: [], interfaces: [{ name: "Wi-Fi B" }] };
  });
  await app.state.refreshInterfaces();
  assert.equal(app.state.selected.value, "");
  app.state.selected.value = "Wi-Fi B";
  await app.state.scan();
  assert.equal(app.state.interfaces.value.length, 2);
  app.unmount();
}

console.log("WiFi state: PASS (timeout, retry, deduplication, interface selection, unmount cleanup)");
