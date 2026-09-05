"""Render the shipped Vue bundle with mocked IPC; no device I/O or server required."""
from pathlib import Path
from urllib.parse import unquote, urlsplit

from playwright.sync_api import sync_playwright


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "analysis" / "tauri-assets"
OUTPUT = ROOT / ".build-cache"
MOCK = r"""
localStorage.setItem('ntk-theme', 'light');
let callbackId = 1;
window.__TAURI_INTERNALS__ = {
  metadata: {currentWindow: {label: 'main'}, currentWebview: {label: 'main'}},
  transformCallback: () => callbackId++, unregisterCallback: () => {},
  invoke: async (command, args = {}) => {
    if (command === 'protocol_server_status') return {servers: []};
    if (command === 'protocol_tcp_client_status') return {connected: false};
    if (command === 'protocol_server_get_memory') return {values: [0, 1, 42]};
    if (command === 'serial_list_ports') return [{name: 'COM9', description: 'Test port'}];
    if (command === 'protocol_payload_preview') return {byte_count: 0, hex: ''};
    if (command === 'protocol_modbus_tcp' || command === 'protocol_modbus_rtu') {
      window.__lastRequest = args.req;
      return {registers: [1, 2, 3], received_hex: '01 03 06 00 01 00 02 00 03', elapsed_ms: 2};
    }
    if (command === 'plugin:event|listen') return callbackId++;
    if (command.includes('is_')) return false;
    if (command.includes('version')) return '9.0.0';
    if (command.includes('list') || command.includes('get_all')) return [];
    return {};
  }
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
"""


def serve(route):
    path = unquote(urlsplit(route.request.url).path).lstrip("/") or "index.html"
    file = (ASSETS / path).resolve()
    if not file.is_relative_to(ASSETS.resolve()) or not file.is_file():
        route.fulfill(status=404, body="Not found")
        return
    route.fulfill(path=str(file))


with sync_playwright() as playwright:
    browser = playwright.chromium.launch(headless=True)
    try:
        page = browser.new_page(viewport={"width": 1280, "height": 820})
        page.set_default_timeout(5000)
        page.route("http://protocol.test/**", serve)
        page.add_init_script(MOCK)
        errors = []
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.goto("http://protocol.test/#/protocol-workbench", wait_until="networkidle")
        page.locator(".protocol-workbench-v4").wait_for()
        OUTPUT.mkdir(exist_ok=True)
        for width, height in [(1280, 820), (1050, 700)]:
            page.set_viewport_size({"width": width, "height": height})
            for role in ["Client", "Server"]:
                page.locator(".pw4-session").filter(has_text=f"Modbus RTU {role}").click()
                panel = page.locator(".pw4-panel")
                panel.get_by_text("停止位", exact=True).wait_for()
                bad = page.locator(".pw4-form-grid").evaluate_all("""grids => grids.flatMap(grid => {
                  const parent = grid.getBoundingClientRect();
                  return [...grid.children].filter(child => {
                    const box = child.getBoundingClientRect();
                    return box.left < parent.left - 1 || box.right > parent.right + 1;
                  }).map(child => child.textContent);
                })""")
                assert not bad, (width, role, bad)
                page.screenshot(path=str(OUTPUT / f"protocol-rtu-{role.lower()}-{width}.png"))
        page.locator(".pw4-session").filter(has_text="Modbus TCP Client").click()
        field = lambda label: page.locator(".pw4-field").filter(has=page.get_by_text(label, exact=True))
        field("起始地址").locator("input").fill("99999")
        page.get_by_role("button", name="执行一次", exact=True).click()
        page.get_by_text("地址必须是 0-65535 范围内的整数", exact=False).first.wait_for()
        assert page.evaluate("window.__lastRequest === undefined")
        field("起始地址").locator("input").fill("10")
        field("数量").locator("input").fill("3")
        page.get_by_role("button", name="执行一次", exact=True).click()
        page.locator(".pw4-table-wrap.modbus").get_by_text("0x0001", exact=True).wait_for()
        field("显示格式").locator("select").select_option("u32")
        page.locator(".pw4-table-wrap.modbus").get_by_text("65538", exact=True).wait_for()
        field("字节 / 字序").locator("select").select_option("CDAB")
        page.locator(".pw4-table-wrap.modbus").get_by_text("131073", exact=True).wait_for()
        assert not errors, errors
        print("protocol browser: PASS (1280x820, 1050x700, RTU fields, validation, live formatting; mocked IPC)")
    finally:
        browser.close()
