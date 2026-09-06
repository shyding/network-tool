"""Exercise the shipped WiFi view using synthetic scan results and bounded UI waits."""
from pathlib import Path
from urllib.parse import unquote, urlsplit

from playwright.sync_api import sync_playwright


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "analysis" / "tauri-assets"
MOCK = r"""
let callbackId = 1;
window.__wifiCalls = [];
window.__TAURI_INTERNALS__ = {
  metadata: {currentWindow: {label: 'main'}, currentWebview: {label: 'main'}},
  transformCallback: () => callbackId++, unregisterCallback: () => {},
  invoke: async (command, args = {}) => {
    if (command === 'list_wifi_interfaces') return [{name: 'Wi-Fi', description: 'Test adapter'}];
    if (command === 'scan_wifi_networks') {
      window.__wifiCalls.push(args);
      const aps = Array.from({length: 48}, (_, i) => ({
        ssid: 'Network-' + i, bssid: '00:11:22:33:44:' + i.toString(16).padStart(2, '0'),
        signal_percent: 100 - i, signal_dbm: -30 - i, quality: '良好',
        channel: i % 13 + 1, band: '2.4G', auth: 'WPA2-Personal',
      }));
      return {aps, channel_stats: [{channel: 1, weight: 40, band: '2.4G'}], recommend_24g: [6, 11], message: 'Scan complete'};
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
        page.set_default_timeout(4000)
        page.route("http://wifi.test/**", serve)
        page.add_init_script(MOCK)
        errors = []
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.goto("http://wifi.test/#/wifianalyze", wait_until="networkidle")
        print("WiFi page opened", flush=True)
        page.get_by_role("button", name="开始扫描", exact=True).click()
        page.locator(".wifi tbody tr").nth(47).wait_for()
        print("48 scan results rendered", flush=True)
        for width, height in [(1280, 820), (1050, 700)]:
            page.set_viewport_size({"width": width, "height": height})
            page.wait_for_timeout(1200)
            sizes = page.locator(".wifi canvas").evaluate_all("canvases => canvases.map(c => [c.width, c.height])")
            assert len(sizes) == 3, sizes
            assert all(20 < w < 4000 and 20 < h < 2000 for w, h in sizes), sizes
            page.get_by_role("button", name="刷新网卡", exact=True).click()
            print(f"{width}x{height} charts and clicks responsive: {sizes}", flush=True)
        assert not errors, errors
        page.get_by_text("Ping 测试", exact=True).first.click()
        page.wait_for_url("**/#/ping")
        assert page.locator(".wifi").count() == 0
        print("WiFi browser: PASS (chart bounds, scan, refresh, navigation)", flush=True)
    finally:
        browser.close()
