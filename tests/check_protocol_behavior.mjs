import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const source = fs.readFileSync(new URL("../analysis/tauri-assets/assets/ProtocolWorkbenchViewV4.js", import.meta.url), "utf8");
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const tick = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
const text = node => node == null ? "" : Array.isArray(node) ? node.map(text).join(" ") : typeof node === "object" ? text(node.children) : String(node);
function find(node, predicate) {
  if (!node || typeof node !== "object") return null;
  if (Array.isArray(node)) return node.map(child => find(child, predicate)).find(Boolean);
  return predicate(node) ? node : find(node.children, predicate);
}

async function harness(overrides = {}) {
  const hooks = { mounted: [], unmounted: [] };
  const calls = [];
  const intervals = new Map();
  const listeners = new Map();
  const storage = new Map();
  let nextTimer = 1;
  let removed = 0;
  const defaults = {
    protocol_server_status: () => ({ servers: [] }),
    protocol_tcp_client_status: () => ({ connected: false }),
    serial_list_ports: () => [],
    protocol_server_get_memory: () => ({ values: [0, 0] }),
    protocol_payload_preview: () => ({ byte_count: 0, hex: "" }),
  };
  const context = vm.createContext({
    console, __hooks: hooks,
    localStorage: { getItem: key => storage.get(key) || null, setItem: (key, value) => storage.set(key, value) },
    setInterval: (callback, delay) => { const id = nextTimer++; intervals.set(id, { callback, delay }); return id; },
    clearInterval: id => intervals.delete(id), setTimeout: () => nextTimer++, clearTimeout: () => {},
    __invoke: async (command, args) => {
      calls.push({ command, args });
      const handler = overrides[command] || defaults[command];
      if (!handler) throw new Error(`Unexpected command: ${command}`);
      return handler(args);
    },
    __listen: async (name, callback) => {
      if (overrides.listen) return overrides.listen(name, callback);
      listeners.set(name, callback);
      return () => { removed++; listeners.delete(name); };
    },
  });
  const dependency = new vm.SourceTextModule(`
    export const a = value => value;
    export const r = value => ({ value });
    export const o = callback => __hooks.mounted.push(callback);
    export const b = callback => __hooks.unmounted.push(callback);
    export const e = (tag, props, children) => ({ tag, props, children: children && !Array.isArray(children) && typeof children === "object" ? null : children });
    export const t = (command, args) => __invoke(command, args);
    export const p = (name, callback) => __listen(name, callback);
  `, { context });
  await dependency.link(() => { throw new Error("Unexpected import"); });
  await dependency.evaluate();
  const module = new vm.SourceTextModule(source, { context });
  await module.link(() => dependency);
  await module.evaluate();
  const render = module.namespace.default.setup();
  const button = label => {
    const node = find(render(), node => node.tag === "button" && text(node) === label);
    assert.ok(node, `Missing button ${label}`);
    return node;
  };
  const field = label => {
    const node = find(render(), node => node.tag === "label" && text(node.children?.[0]) === label);
    assert.ok(node, `Missing field ${label}`);
    return find(node.children, child => ["input", "select"].includes(child.tag));
  };
  return {
    render, calls, intervals, listeners, storage, get removed() { return removed; }, button, field,
    mount: () => Promise.all(hooks.mounted.map(callback => callback())),
    unmount: () => hooks.unmounted.forEach(callback => callback()),
    selectSession: label => {
      const node = find(render(), node => node.tag === "button" && String(node.props?.class).split(" ").includes("pw4-session") && text(node).includes(label));
      assert.ok(node, `Missing session ${label}`);
      node.props.onClick();
    },
    set: (label, value) => {
      const node = field(label);
      (node.props.onInput || node.props.onChange)({ target: { value: String(value) } });
    },
    click: label => button(label).props.onClick(),
    allText: () => text(render()),
  };
}

{
  const app = await harness();
  app.selectSession("Modbus TCP Client");
  for (const [field, value, reset] of [["起始地址", "99999", "0"], ["数量", "126", "1"], ["端口", "1502abc", "1502"], ["Unit ID", "256", "1"]]) {
    app.set(field, value);
    await app.click("执行一次");
    assert.ok(!app.calls.some(call => call.command === "protocol_modbus_tcp"));
    app.set(field, reset);
  }
  app.set("功能码", 6);
  for (const value of ["12abc", "1.5", "1,2", "0x12oops", "65536", ""]) {
    app.set("写入值", value);
    await app.click("执行一次");
    assert.ok(!app.calls.some(call => call.command === "protocol_modbus_tcp"));
  }
  app.unmount();
}

{
  let request;
  const reply = deferred();
  const app = await harness({ protocol_modbus_tcp: ({ req }) => { request = req; return reply.promise; } });
  app.selectSession("Modbus TCP Client");
  app.set("显示格式", "hex16");
  assert.equal(JSON.parse(app.storage.get("network-toolbox.protocol-workbench.v5")).sessions["modbus-tcp-client"].format, "hex16");
  app.set("显示格式", "u16");
  app.set("起始地址", "10");
  app.set("数量", "3");
  const pending = app.click("执行一次");
  app.set("起始地址", "90");
  reply.resolve({ registers: [1, 2, 3], received_hex: "00", elapsed_ms: 1 });
  await pending;
  assert.equal(request.address, 10);
  const table = () => text(find(app.render(), node => String(node.props?.class).includes("pw4-table-wrap modbus")));
  assert.ok(table().includes("10 0x0001 1"), table());
  app.set("显示格式", "u32");
  assert.ok(table().includes("65538"), table());
  assert.ok(table().includes("数据不足 (1/2)"), table());
  app.set("字节 / 字序", "CDAB");
  assert.ok(table().includes("131073"), table());
  app.set("功能码", 5);
  app.set("写入值", "ON");
  await app.click("执行一次");
  assert.equal(request.values[0], 1);
  app.unmount();
}

{
  const app = await harness({ protocol_modbus_rtu: ({ req }) => {
    assert.equal(req.stop_bits, 2);
    assert.equal(req.unit_id, 3);
    return { registers: [42], received_hex: "backend-validated", elapsed_ms: 1 };
  } });
  app.selectSession("Modbus RTU Client");
  app.set("串口", "COM9");
  app.set("停止位", 2);
  app.set("Unit ID", 3);
  app.set("数量", 1);
  await app.click("执行一次");
  assert.ok(app.allText().includes("0x002A 42"));
  app.unmount();
}

{
  const pending = [];
  const app = await harness({ protocol_server_get_memory: args => {
    const result = deferred(); pending.push({ args, ...result }); return result.promise;
  } });
  app.selectSession("Modbus TCP Server");
  app.click("线圈");
  assert.equal(pending[0].args.area, "holding");
  assert.equal(pending[1].args.area, "coil");
  pending[1].resolve({ values: [1] });
  await tick();
  pending[0].resolve({ values: [123] });
  await tick();
  assert.ok(app.allText().includes("ON"));
  assert.ok(!app.allText().includes("123"));
  app.unmount();
}

{
  const app = await harness({ protocol_server_set_memory: () => { throw new Error("write failed"); } });
  app.selectSession("Modbus TCP Server");
  await tick();
  const input = find(app.render(), node => node.tag === "input" && node.props?.onChange && node.props?.max === "65535");
  assert.ok(input);
  assert.equal(input.props.onInput, undefined);
  const target = { value: "42" };
  await input.props.onChange({ target });
  assert.equal(target.value, 0);
  assert.ok(app.allText().includes("write failed"));
  app.unmount();
}

{
  const initial = deferred();
  const app = await harness({ protocol_server_status: () => initial.promise });
  const mount = app.mount();
  app.unmount();
  initial.resolve({ servers: [] });
  await mount;
  assert.equal(app.intervals.size, 0);
  assert.equal(app.listeners.size, 0);
}

{
  const registration = deferred();
  let removed = 0;
  const app = await harness({ listen: () => registration.promise });
  const mount = app.mount();
  await tick();
  app.unmount();
  registration.resolve(() => removed++);
  await mount;
  assert.equal(removed, 1);
  assert.equal(app.intervals.size, 0);
}

{
  const reply = deferred();
  const app = await harness({ protocol_modbus_tcp: () => reply.promise });
  app.selectSession("Modbus TCP Client");
  const polling = app.click("启动轮询");
  app.unmount();
  reply.resolve({ registers: [1] });
  await polling;
  assert.equal(app.intervals.size, 0);
}

{
  const status = deferred();
  const app = await harness({
    protocol_tcp_client_status: () => status.promise,
    protocol_tcp_client_connect: () => ({ connected: true, peer: "127.0.0.1:9000" }),
  });
  const mount = app.mount();
  await tick();
  await app.click("连接");
  status.resolve({ connected: false });
  await mount;
  assert.ok(app.button("断开"));
  app.unmount();
}

{
  let request;
  const app = await harness({ protocol_server_start: ({ req }) => { request = req; return true; } });
  app.selectSession("Modbus RTU Server");
  app.set("串口", "COM9");
  app.set("Unit ID", 7);
  app.set("停止位", 2);
  await app.click("启动 Server");
  assert.equal(request.unit_id, 7);
  assert.equal(request.stop_bits, 2);
  assert.equal(request.data_bits, 8);
  app.unmount();
}

{
  const app = await harness();
  await app.mount();
  assert.equal(app.intervals.size, 1);
  assert.equal(app.listeners.size, 5);
  app.unmount();
  assert.equal(app.intervals.size, 0);
  assert.equal(app.listeners.size, 0);
  assert.equal(app.removed, 5);
}

console.log("protocol behavior: PASS (validation, snapshots, RTU results, memory races, rollback, lifecycle)");
