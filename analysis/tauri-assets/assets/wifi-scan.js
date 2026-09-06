import { r as ref, b as onBeforeUnmount, t as invoke } from "./index-DNL4tSY9.js";

export function useWifiScan() {
  const interfaces = ref([]);
  const selected = ref("");
  const result = ref(null);
  const busy = ref(false);
  const refreshing = ref(false);
  const error = ref("");
  const pending = new Set();
  let disposed = false;

  function query(command, args) {
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (callback, value) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        pending.delete(cancel);
        callback(value);
      };
      const cancel = () => finish(reject, new Error("WiFi 页面已关闭"));
      const timer = setTimeout(() => finish(reject, new Error("WiFi 查询超时，请检查无线网卡后重试")), 10000);
      pending.add(cancel);
      Promise.resolve().then(() => invoke(command, args)).then(
        value => finish(resolve, value),
        cause => finish(reject, cause),
      );
    });
  }

  async function refreshInterfaces() {
    if (disposed || refreshing.value || busy.value) return;
    refreshing.value = true;
    error.value = "";
    try {
      const items = await query("list_wifi_interfaces");
      if (disposed) return;
      interfaces.value = Array.isArray(items) ? items : [];
      if (selected.value && !interfaces.value.some(item => item.name === selected.value)) selected.value = "";
    } catch (cause) {
      if (!disposed) error.value = String(cause);
    } finally {
      refreshing.value = false;
    }
  }

  async function scan() {
    if (disposed || busy.value || refreshing.value) return;
    busy.value = true;
    error.value = "";
    const name = selected.value;
    try {
      const data = await query("scan_wifi_networks", { interface: name || null });
      if (disposed) return;
      result.value = data;
      if (Array.isArray(data?.interfaces) && data.interfaces.length && !name) interfaces.value = data.interfaces;
    } catch (cause) {
      if (!disposed) error.value = String(cause);
    } finally {
      busy.value = false;
    }
  }

  onBeforeUnmount(() => {
    disposed = true;
    pending.forEach(cancel => cancel());
  });

  return { interfaces, selected, result, busy, refreshing, error, refreshInterfaces, scan };
}
