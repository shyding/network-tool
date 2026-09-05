import {
  a as defineComponent,
  r as ref,
  o as onMounted,
  b as onBeforeUnmount,
  e as h,
  t as invoke,
  p as listen,
} from "./index-DNL4tSY9.js";

const SESSION_GROUPS = [
  {
    label: "原始通信",
    items: [
      { id: "tcp-client", code: "TCP", role: "Client", label: "TCP Client" },
      { id: "tcp-server", code: "TCP", role: "Server", label: "TCP Server" },
      { id: "udp-client", code: "UDP", role: "Client", label: "UDP Client" },
      { id: "udp-server", code: "UDP", role: "Server", label: "UDP Server" },
    ],
  },
  {
    label: "Modbus",
    items: [
      { id: "modbus-tcp-client", code: "MB", role: "Client", label: "Modbus TCP Client" },
      { id: "modbus-tcp-server", code: "MB", role: "Server", label: "Modbus TCP Server" },
      { id: "modbus-rtu-client", code: "RTU", role: "Client", label: "Modbus RTU Client" },
      { id: "modbus-rtu-server", code: "RTU", role: "Server", label: "Modbus RTU Server" },
    ],
  },
];

const SESSION_INDEX = Object.fromEntries(
  SESSION_GROUPS.flatMap((group) => group.items.map((item) => [item.id, item])),
);
const SERVER_SESSION = {
  tcp: "tcp-server",
  udp: "udp-server",
  modbus_tcp: "modbus-tcp-server",
  modbus_rtu: "modbus-rtu-server",
};
const INPUT_MODES = [["text", "文本"], ["hex", "HEX"], ["base64", "Base64"]];
const TEXT_ENCODINGS = [
  ["utf-8", "UTF-8"],
  ["ascii", "ASCII"],
  ["gb18030", "GBK（GB18030）"],
  ["utf-16le", "UTF-16 LE"],
  ["utf-16be", "UTF-16 BE"],
  ["iso-8859-1", "ISO-8859-1"],
];
const LINE_ENDINGS = [["none", "无"], ["cr", "CR"], ["lf", "LF"], ["crlf", "CRLF"], ["custom", "自定义 HEX"]];
const FRAMING_MODES = [["raw", "原始流"], ["delimiter", "分隔符"], ["fixed", "固定长度"]];
const RESPONSE_MODES = [["echo", "Echo"], ["fixed", "固定应答"], ["none", "不应答"]];
const MODBUS_FUNCTIONS = [
  [1, "FC01 读线圈"],
  [2, "FC02 读离散输入"],
  [3, "FC03 读保持寄存器"],
  [4, "FC04 读输入寄存器"],
  [5, "FC05 写单线圈"],
  [6, "FC06 写单寄存器"],
  [15, "FC0F 写多线圈"],
  [16, "FC10 写多寄存器"],
];
const DATA_FORMATS = [
  ["u16", "U16"],
  ["s16", "S16"],
  ["hex16", "HEX16"],
  ["bin16", "BIN16"],
  ["u32", "U32"],
  ["s32", "S32"],
  ["float32", "Float32"],
  ["s64", "S64"],
  ["double64", "Double64"],
  ["ascii", "ASCII"],
];
const WORD_ORDERS = [["ABCD", "ABCD"], ["CDAB", "CDAB"], ["BADC", "BADC"], ["DCBA", "DCBA"]];
const MEMORY_AREAS = [
  ["holding", "保持寄存器"],
  ["input", "输入寄存器"],
  ["coil", "线圈"],
  ["discrete", "离散输入"],
];

const asNumber = (value, fallback = 0) => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
};
const clampInteger = (value, min, max, fallback = min) => {
  const parsed = Number.parseInt(String(value), 10);
  return Number.isFinite(parsed) ? Math.max(min, Math.min(max, parsed)) : fallback;
};
const requireInteger = (value, min, max, label) => {
  const text = String(value).trim();
  const parsed = Number(text);
  if (!/^\d+$/.test(text) || !Number.isSafeInteger(parsed) || parsed < min || parsed > max) {
    throw new Error(`${label}必须是 ${min}-${max} 范围内的整数`);
  }
  return parsed;
};
const isReadFunction = (value) => Number(value) >= 1 && Number(value) <= 4;
const functionArea = (value) => {
  const fn = Number(value);
  if (fn === 1 || fn === 5 || fn === 15) return "coil";
  if (fn === 2) return "discrete";
  if (fn === 4) return "input";
  return "holding";
};
const areaPrefix = (area) => area === "coil" ? 0 : area === "discrete" ? 1 : area === "input" ? 3 : 4;

export default defineComponent({
  name: "ProtocolWorkbenchViewV4",
  setup() {
    const activeSession = ref("tcp-client");
    const servers = ref([]);
    const serialPorts = ref([]);
    const timeline = ref([]);
    const timelineFilter = ref("all");
    const timelineSearch = ref("");
    const selectedEvent = ref("");
    const displayMode = ref("dual");
    const globalError = ref("");
    const copyState = ref("");
    let localSequence = 0;
    let statusTimer = null;
    let disposed = false;
    let statusPending = false;
    let clientStatusVersion = 0;
    let memoryToken = 0;
    const unlisteners = [];
    const previewStates = [];
    const saveKey = "network-toolbox.protocol-workbench.v5";

    const makePayload = (initial = "") => {
      const state = {
        inputMode: ref("text"),
        textEncoding: ref("utf-8"),
        parseEscapes: ref(false),
        lineEnding: ref("none"),
        customSuffixHex: ref(""),
        payload: ref(initial),
        preview: ref(null),
        previewError: ref(""),
        previewing: ref(false),
        timer: null,
        token: 0,
      };
      previewStates.push(state);
      return state;
    };

    const makeRawClient = (kind, port) => ({
      kind,
      host: ref("127.0.0.1"),
      port: ref(String(port)),
      timeoutMs: ref("2000"),
      framingMode: ref("raw"),
      delimiterHex: ref("0D 0A"),
      fixedLength: ref("8"),
      connected: ref(false),
      peer: ref(""),
      busy: ref(false),
      error: ref(""),
      latest: ref(null),
      composer: makePayload(""),
    });
    const makeRawServer = (kind, port) => ({
      kind,
      host: ref("0.0.0.0"),
      port: ref(String(port)),
      framingMode: ref("raw"),
      delimiterHex: ref("0D 0A"),
      fixedLength: ref("8"),
      responseMode: ref("echo"),
      peer: ref("*"),
      busy: ref(false),
      error: ref(""),
      latest: ref(null),
      composer: makePayload(""),
    });
    const makeModbusClient = (transport) => ({
      transport,
      host: ref("127.0.0.1"),
      port: ref("1502"),
      serialPort: ref(""),
      baudRate: ref("9600"),
      parity: ref("none"),
      stopBits: ref("1"),
      timeoutMs: ref("2000"),
      unitId: ref("1"),
      fn: ref("3"),
      address: ref("0"),
      addressMode: ref("zero"),
      quantity: ref("10"),
      values: ref("0"),
      format: ref("u16"),
      wordOrder: ref("ABCD"),
      pollMs: ref("500"),
      polling: ref(false),
      pollTimer: null,
      busy: ref(false),
      error: ref(""),
      rows: ref([]),
      latest: ref(null),
      lastRequest: null,
    });
    const makeModbusServer = (transport) => ({
      transport,
      kind: transport === "tcp" ? "modbus_tcp" : "modbus_rtu",
      host: ref("0.0.0.0"),
      port: ref("1502"),
      serialPort: ref(""),
      baudRate: ref("9600"),
      parity: ref("none"),
      stopBits: ref("1"),
      unitId: ref("1"),
      busy: ref(false),
      error: ref(""),
      latest: ref(null),
    });

    const tcpClient = makeRawClient("tcp", 9000);
    const udpClient = makeRawClient("udp", 9000);
    const tcpServer = makeRawServer("tcp", 9000);
    const udpServer = makeRawServer("udp", 9000);
    const modbusTcpClient = makeModbusClient("tcp");
    const modbusRtuClient = makeModbusClient("rtu");
    const modbusTcpServer = makeModbusServer("tcp");
    const modbusRtuServer = makeModbusServer("rtu");
    const memoryArea = ref("holding");
    const memoryStart = ref("0");
    const memoryCount = ref("20");
    const memoryRows = ref([]);
    const memoryBusy = ref(false);
    const memoryError = ref("");

    const configs = {
      "tcp-client": tcpClient,
      "udp-client": udpClient,
      "tcp-server": tcpServer,
      "udp-server": udpServer,
      "modbus-tcp-client": modbusTcpClient,
      "modbus-rtu-client": modbusRtuClient,
      "modbus-tcp-server": modbusTcpServer,
      "modbus-rtu-server": modbusRtuServer,
    };

    const persistedFields = {
      "tcp-client": ["host", "port", "timeoutMs", "framingMode", "delimiterHex", "fixedLength"],
      "udp-client": ["host", "port", "timeoutMs"],
      "tcp-server": ["host", "port", "framingMode", "delimiterHex", "fixedLength", "responseMode"],
      "udp-server": ["host", "port", "responseMode"],
      "modbus-tcp-client": ["host", "port", "timeoutMs", "unitId", "fn", "address", "addressMode", "quantity", "values", "format", "wordOrder", "pollMs"],
      "modbus-rtu-client": ["serialPort", "baudRate", "parity", "stopBits", "timeoutMs", "unitId", "fn", "address", "addressMode", "quantity", "values", "format", "wordOrder", "pollMs"],
      "modbus-tcp-server": ["host", "port"],
      "modbus-rtu-server": ["serialPort", "baudRate", "parity", "stopBits", "unitId"],
    };

    function payloadSnapshot(state) {
      return {
        inputMode: state.inputMode.value,
        textEncoding: state.textEncoding.value,
        parseEscapes: state.parseEscapes.value,
        lineEnding: state.lineEnding.value,
        customSuffixHex: state.customSuffixHex.value,
        payload: state.payload.value,
      };
    }

    function persist() {
      if (typeof localStorage === "undefined") return;
      const data = { activeSession: activeSession.value, sessions: {}, memory: {
        area: memoryArea.value,
        start: memoryStart.value,
        count: memoryCount.value,
      }};
      Object.entries(configs).forEach(([id, config]) => {
        const values = {};
        (persistedFields[id] || []).forEach((key) => { values[key] = config[key].value; });
        if (config.composer) values.composer = payloadSnapshot(config.composer);
        data.sessions[id] = values;
      });
      try { localStorage.setItem(saveKey, JSON.stringify(data)); } catch (_) {}
    }

    function restore() {
      if (typeof localStorage === "undefined") return;
      try {
        const data = JSON.parse(localStorage.getItem(saveKey) || "{}");
        if (SESSION_INDEX[data.activeSession]) activeSession.value = data.activeSession;
        Object.entries(data.sessions || {}).forEach(([id, values]) => {
          const config = configs[id];
          if (!config) return;
          (persistedFields[id] || []).forEach((key) => {
            if (Object.prototype.hasOwnProperty.call(values, key)) config[key].value = values[key];
          });
          if (config.composer && values.composer) {
            Object.entries(values.composer).forEach(([key, value]) => {
              if (config.composer[key]) config.composer[key].value = value;
            });
          }
        });
        if (data.memory) {
          memoryArea.value = data.memory.area || memoryArea.value;
          memoryStart.value = data.memory.start || memoryStart.value;
          memoryCount.value = data.memory.count || memoryCount.value;
        }
      } catch (_) {}
    }

    function timestamp(value) {
      const date = new Date(value || Date.now());
      const milliseconds = String(date.getMilliseconds()).padStart(3, "0");
      return date.toLocaleTimeString("zh-CN", { hour12: false }) + "." + milliseconds;
    }

    function addTimeline(event) {
      if (disposed) return;
      localSequence += 1;
      const item = {
        id: "event-" + localSequence,
        localSequence,
        wireSequence: event.sequence || null,
        timestampMs: event.timestamp_ms || Date.now(),
        time: timestamp(event.timestamp_ms),
        sessionId: event.sessionId,
        direction: event.direction || "sys",
        endpoint: event.endpoint || "",
        byteLength: Number(event.byteLength || 0),
        hex: event.hex || "",
        text: event.text || "",
        summary: event.summary || "",
        detail: event.detail || "",
      };
      timeline.value = [...timeline.value, item].slice(-2000);
      return item;
    }

    function addExchange(sessionId, result, summary) {
      const endpoint = result.peer || "";
      if (Number(result.sent_bytes || 0) > 0) {
        addTimeline({
          sequence: result.sequence,
          timestamp_ms: result.timestamp_ms,
          sessionId,
          direction: "tx",
          endpoint,
          byteLength: result.sent_bytes,
          hex: result.sent_hex,
          text: result.sent_text,
          summary: summary || "发送完成",
        });
      }
      if (Number(result.received_bytes || 0) > 0) {
        addTimeline({
          sequence: result.sequence,
          timestamp_ms: result.timestamp_ms,
          sessionId,
          direction: "rx",
          endpoint,
          byteLength: result.received_bytes,
          hex: result.received_hex,
          text: result.received_text,
          summary: "收到响应",
        });
      }
    }

    function reportError(sessionId, cause, summary) {
      const message = String(cause);
      addTimeline({ sessionId, direction: "err", summary: summary || "操作失败", detail: message });
      return message;
    }

    function payloadRequest(state) {
      return {
        payload: state.payload.value,
        input_mode: state.inputMode.value,
        text_encoding: state.textEncoding.value,
        parse_escapes: state.parseEscapes.value,
        line_ending: state.lineEnding.value,
        custom_suffix_hex: state.customSuffixHex.value,
      };
    }

    async function refreshPreview(state) {
      const token = ++state.token;
      state.previewing.value = true;
      state.previewError.value = "";
      try {
        const result = await invoke("protocol_payload_preview", { req: payloadRequest(state) });
        if (token === state.token) state.preview.value = result;
        return result;
      } catch (cause) {
        if (token === state.token) {
          state.preview.value = null;
          state.previewError.value = String(cause);
        }
        return null;
      } finally {
        if (token === state.token) state.previewing.value = false;
      }
    }

    function schedulePreview(state) {
      if (state.timer) clearTimeout(state.timer);
      state.timer = setTimeout(() => refreshPreview(state), 180);
      persist();
    }

    function serverFor(kind) {
      return servers.value.find((item) => item.kind === kind && item.running);
    }

    async function refreshStatus() {
      if (disposed || statusPending) return;
      statusPending = true;
      const version = clientStatusVersion;
      try {
        const result = await invoke("protocol_server_status");
        const client = await invoke("protocol_tcp_client_status");
        if (disposed) return;
        servers.value = result.servers || [];
        if (tcpClient.busy.value || version !== clientStatusVersion) return;
        tcpClient.connected.value = Boolean(client.connected);
        tcpClient.peer.value = client.peer || "";
      } catch (cause) {
        if (!disposed) globalError.value = String(cause);
      } finally {
        statusPending = false;
      }
    }

    function rawExchangeRequest(config) {
      return {
        host: config.host.value.trim(),
        port: clampInteger(config.port.value, 1, 65535, 9000),
        timeout_ms: clampInteger(config.timeoutMs.value, 100, 30000, 2000),
        ...payloadRequest(config.composer),
      };
    }

    function framingRequest(config) {
      return {
        framing_mode: config.framingMode.value,
        frame_delimiter_hex: config.framingMode.value === "delimiter" ? config.delimiterHex.value : "",
        fixed_frame_length: config.framingMode.value === "fixed"
          ? clampInteger(config.fixedLength.value, 1, 65535, 8)
          : null,
      };
    }

    function parseModbusAddress(config) {
      const text = String(config.address.value).trim();
      if (!/^\d+$/.test(text)) throw new Error("地址必须是十进制整数");
      if (config.addressMode.value === "zero") return requireInteger(text, 0, 65535, "地址");
      const area = functionArea(config.fn.value);
      if (text.length >= 5) {
        if (Number(text[0]) !== areaPrefix(area)) throw new Error("PLC 地址前缀与功能码的数据区不一致");
        const offset = Number.parseInt(text.slice(1), 10);
        if (offset < 1 || offset > 65536) throw new Error("PLC 地址超出范围");
        return offset - 1;
      }
      const offset = Number.parseInt(text, 10);
      if (offset < 1 || offset > 65536) throw new Error("PLC 地址应从 1 开始");
      return offset - 1;
    }

    function formatModbusAddress(address, area, mode = "zero") {
      const normalized = clampInteger(address, 0, 65535, 0);
      if (mode === "zero") return String(normalized);
      const offset = normalized + 1;
      return `${areaPrefix(area)}${String(offset).padStart(offset <= 9999 ? 4 : 5, "0")}`;
    }

    function parseModbusValues(text, bitValues = false) {
      const tokens = String(text).split(/[\s,;]+/).filter(Boolean);
      if (!tokens.length) throw new Error("写入值不能为空");
      return tokens.map((token) => {
        if (bitValues) {
          if (/^(1|true|on|yes|ff00)$/i.test(token)) return 1;
          if (/^(0|false|off|no|0000)$/i.test(token)) return 0;
          throw new Error(`无法识别线圈值：${token}`);
        }
        const value = /^(?:0x[0-9a-f]+|\d+)$/i.test(token) ? Number(token) : NaN;
        if (!Number.isSafeInteger(value) || value < 0 || value > 65535) {
          throw new Error(`寄存器值超出 0-65535：${token}`);
        }
        return value;
      });
    }

    function registersPerValue(format) {
      if (["u32", "s32", "float32"].includes(format)) return 2;
      if (["s64", "double64"].includes(format)) return 4;
      return 1;
    }

    function orderedBytes(registers, order, words) {
      const reverseWords = order === "CDAB" || order === "DCBA";
      const swapBytes = order === "BADC" || order === "DCBA";
      const bytes = [];
      for (let index = 0; index < words; index += 1) {
        const word = Number(registers[reverseWords ? words - 1 - index : index] || 0) & 0xffff;
        const high = (word >> 8) & 0xff;
        const low = word & 0xff;
        bytes.push(...(swapBytes ? [low, high] : [high, low]));
      }
      return bytes;
    }

    function formatRegisters(registers, format, order) {
      if (!registers.length) return "";
      const value = Number(registers[0]) & 0xffff;
      if (format === "u16") return String(value);
      if (format === "s16") return String(value > 0x7fff ? value - 0x10000 : value);
      if (format === "hex16") return `0x${value.toString(16).toUpperCase().padStart(4, "0")}`;
      if (format === "bin16") return value.toString(2).padStart(16, "0");
      if (format === "ascii") {
        return registers.map((item) => {
          const word = Number(item) & 0xffff;
          return String.fromCharCode((word >> 8) & 0xff, word & 0xff).replace(/[^\x20-\x7e]/g, ".");
        }).join("");
      }
      const words = registersPerValue(format);
      const view = new DataView(new Uint8Array(orderedBytes(registers, order, words)).buffer);
      if (format === "u32") return String(view.getUint32(0, false));
      if (format === "s32") return String(view.getInt32(0, false));
      if (format === "float32") return String(view.getFloat32(0, false));
      if (format === "s64") return String(view.getBigInt64(0, false));
      if (format === "double64") return String(view.getFloat64(0, false));
      return String(value);
    }

    function bytesFromHex(value) {
      const compact = String(value || "").replace(/[^0-9a-f]/gi, "");
      if (!compact || compact.length % 2 !== 0) return [];
      return compact.match(/.{2}/g).map((item) => Number.parseInt(item, 16));
    }

    function buildModbusRows(config, result, address, quantity) {
      const fn = Number(config.fn.value);
      const area = functionArea(fn);
      const mode = config.addressMode.value;
      if (Array.isArray(result.bits)) {
        return result.bits.slice(0, quantity).map((value, index) => ({
          address: address + index,
          label: formatModbusAddress(address + index, area, mode),
          raw: value ? "1" : "0",
          value: value ? "ON" : "OFF",
        }));
      }
      if (Array.isArray(result.registers)) {
        const perValue = registersPerValue(config.format.value);
        const rows = [];
        for (let offset = 0; offset < result.registers.length; offset += perValue) {
          const chunk = result.registers.slice(offset, offset + perValue);
          rows.push({
            address: address + offset,
            label: formatModbusAddress(address + offset, area, mode),
            raw: chunk.map((value) => `0x${Number(value).toString(16).toUpperCase().padStart(4, "0")}`).join(" "),
            value: chunk.length < perValue ? `数据不足 (${chunk.length}/${perValue})` : formatRegisters(chunk, config.format.value, config.wordOrder.value),
          });
        }
        return rows;
      }
      return [];
    }

    async function connectTcpClient() {
      clientStatusVersion += 1;
      const config = tcpClient;
      config.busy.value = true;
      config.error.value = "";
      try {
        const result = await invoke("protocol_tcp_client_connect", { req: {
          host: config.host.value.trim(),
          port: clampInteger(config.port.value, 1, 65535, 9000),
          timeout_ms: clampInteger(config.timeoutMs.value, 100, 30000, 2000),
          text_encoding: config.composer.textEncoding.value,
          ...framingRequest(config),
        }});
        config.connected.value = Boolean(result.connected);
        config.peer.value = result.peer || `${config.host.value}:${config.port.value}`;
        addTimeline({ sessionId: "tcp-client", direction: "sys", endpoint: config.peer.value, summary: `TCP 已连接 · ${result.elapsed_ms || 0} ms` });
        persist();
      } catch (cause) {
        config.error.value = reportError("tcp-client", cause, "TCP 连接失败");
      } finally {
        config.busy.value = false;
      }
    }

    async function disconnectTcpClient() {
      clientStatusVersion += 1;
      tcpClient.busy.value = true;
      tcpClient.error.value = "";
      try {
        await invoke("protocol_tcp_client_disconnect");
        tcpClient.connected.value = false;
        tcpClient.peer.value = "";
        addTimeline({ sessionId: "tcp-client", direction: "sys", summary: "TCP 已断开" });
      } catch (cause) {
        tcpClient.error.value = reportError("tcp-client", cause, "TCP 断开失败");
      } finally {
        tcpClient.busy.value = false;
      }
    }

    async function sendRawClient(config, sessionId) {
      config.busy.value = true;
      config.error.value = "";
      try {
        const command = config.kind === "tcp" ? "protocol_tcp_client_send" : "protocol_udp_exchange";
        const request = config.kind === "tcp"
          ? { ...payloadRequest(config.composer), timeout_ms: clampInteger(config.timeoutMs.value, 100, 30000, 2000) }
          : rawExchangeRequest(config);
        const result = await invoke(command, { req: request });
        config.latest.value = result;
        addExchange(sessionId, result, config.kind === "tcp" ? "TCP 发送完成" : "UDP 数据报已发送");
      } catch (cause) {
        config.error.value = reportError(sessionId, cause, `${config.kind.toUpperCase()} 发送失败`);
      } finally {
        config.busy.value = false;
      }
    }

    async function runTcpExchangeOnce() {
      tcpClient.busy.value = true;
      tcpClient.error.value = "";
      try {
        const result = await invoke("protocol_tcp_exchange", { req: rawExchangeRequest(tcpClient) });
        tcpClient.latest.value = result;
        addExchange("tcp-client", result, "TCP 一次收发完成");
      } catch (cause) {
        tcpClient.error.value = reportError("tcp-client", cause, "TCP 一次收发失败");
      } finally {
        tcpClient.busy.value = false;
      }
    }

    async function startServer(config, sessionId) {
      config.busy.value = true;
      config.error.value = "";
      try {
        const isModbus = config.kind.startsWith("modbus_");
        const isRtu = config.kind === "modbus_rtu";
        const composer = config.composer;
        await invoke("protocol_server_start", { req: {
          kind: config.kind,
          host: isRtu ? "0.0.0.0" : config.host.value.trim(),
          port: isRtu ? 0 : requireInteger(config.port.value, 1, 65535, "端口"),
          response: composer && config.responseMode.value === "fixed" ? composer.payload.value : "",
          response_mode: isModbus ? "none" : config.responseMode.value,
          response_input_mode: composer ? composer.inputMode.value : "hex",
          encoding: composer ? composer.inputMode.value : "hex",
          text_encoding: composer ? composer.textEncoding.value : "utf-8",
          parse_escapes: composer ? composer.parseEscapes.value : false,
          line_ending: composer ? composer.lineEnding.value : "none",
          custom_suffix_hex: composer ? composer.customSuffixHex.value : "",
          framing_mode: isModbus ? (isRtu ? "rtu" : "modbus_tcp") : config.kind === "tcp" ? config.framingMode.value : "raw",
          frame_delimiter_hex: !isModbus && config.kind === "tcp" && config.framingMode.value === "delimiter" ? config.delimiterHex.value : "",
          fixed_frame_length: !isModbus && config.kind === "tcp" && config.framingMode.value === "fixed"
            ? clampInteger(config.fixedLength.value, 1, 65535, 8)
            : null,
          serial_port: isRtu ? config.serialPort.value : null,
          baud_rate: isRtu ? requireInteger(config.baudRate.value, 300, 4000000, "波特率") : null,
          parity: isRtu ? config.parity.value : null,
          unit_id: isRtu ? requireInteger(config.unitId.value, 1, 247, "Unit ID") : null,
          data_bits: isRtu ? 8 : null,
          stop_bits: isRtu ? requireInteger(config.stopBits.value, 1, 2, "停止位") : null,
        }});
        await refreshStatus();
        const endpoint = isRtu ? config.serialPort.value : `${config.host.value}:${config.port.value}`;
        addTimeline({ sessionId, direction: "sys", endpoint, summary: "Server 已启动" });
        if (isModbus) await refreshMemory();
        persist();
      } catch (cause) {
        config.error.value = reportError(sessionId, cause, "Server 启动失败");
      } finally {
        config.busy.value = false;
      }
    }

    async function stopServer(config, sessionId) {
      config.busy.value = true;
      config.error.value = "";
      try {
        await invoke("protocol_server_stop", { kind: config.kind });
        await refreshStatus();
        addTimeline({ sessionId, direction: "sys", summary: "Server 已停止" });
      } catch (cause) {
        config.error.value = reportError(sessionId, cause, "Server 停止失败");
      } finally {
        config.busy.value = false;
      }
    }

    async function sendFromTcpServer(config, sessionId) {
      config.busy.value = true;
      config.error.value = "";
      try {
        const result = await invoke("protocol_server_send", { req: {
          kind: config.kind,
          peer: config.peer.value,
          ...payloadRequest(config.composer),
        }});
        config.latest.value = result;
        addTimeline({
          sequence: result.sequence,
          timestamp_ms: result.timestamp_ms,
          sessionId,
          direction: "tx",
          endpoint: (result.sent_to || []).join(", "),
          byteLength: result.sent_bytes,
          hex: result.sent_hex,
          text: result.sent_text,
          summary: `Server 主动发送 · ${(result.sent_to || []).length} 个客户端`,
        });
        await refreshStatus();
      } catch (cause) {
        config.error.value = reportError(sessionId, cause, "Server 主动发送失败");
      } finally {
        config.busy.value = false;
      }
    }

    function stopPolling(config) {
      if (config.pollTimer) clearInterval(config.pollTimer);
      config.pollTimer = null;
      config.polling.value = false;
    }

    async function runModbus(config, sessionId) {
      if (disposed || config.busy.value) return;
      config.busy.value = true;
      config.error.value = "";
      try {
        const fn = requireInteger(config.fn.value, 1, 16, "功能码");
        if (!MODBUS_FUNCTIONS.some(([value]) => value === fn)) throw new Error("不支持的功能码");
        const address = parseModbusAddress(config);
        const read = isReadFunction(fn);
        let values = [];
        let quantity;
        if (read) {
          const limit = fn <= 2 ? 2000 : 125;
          quantity = requireInteger(config.quantity.value, 1, limit, "数量");
        } else {
          values = parseModbusValues(config.values.value, fn === 5 || fn === 15);
          if ((fn === 5 || fn === 6) && values.length !== 1) throw new Error("单点写入必须且只能提供一个值");
          quantity = fn === 5 || fn === 6 ? 1 : values.length;
          if (fn === 15 && quantity > 1968) throw new Error("FC0F 最多写入 1968 个线圈");
          if (fn === 16 && quantity > 123) throw new Error("FC10 最多写入 123 个寄存器");
        }
        if (address + quantity > 65536) throw new Error("请求地址范围超出 65535");
        const snapshot = { fn: { value: fn }, addressMode: { value: config.addressMode.value }, address, quantity };
        const base = {
          unit_id: requireInteger(config.unitId.value, config.transport === "tcp" ? 0 : 1, config.transport === "tcp" ? 255 : 247, "Unit ID"),
          function: fn,
          address,
          quantity,
          values,
          timeout_ms: requireInteger(config.timeoutMs.value, 100, 30000, "超时"),
        };
        const command = config.transport === "tcp" ? "protocol_modbus_tcp" : "protocol_modbus_rtu";
        const req = config.transport === "tcp"
          ? { ...base, host: config.host.value.trim(), port: requireInteger(config.port.value, 1, 65535, "端口") }
          : {
              ...base,
              port_name: config.serialPort.value,
              baud_rate: requireInteger(config.baudRate.value, 300, 4000000, "波特率"),
              data_bits: 8,
              stop_bits: requireInteger(config.stopBits.value, 1, 2, "停止位"),
              parity: config.parity.value,
            };
        if (config.transport === "rtu" && !req.port_name) throw new Error("请选择 Modbus RTU 串口");
        if (config.transport === "tcp" && !req.host) throw new Error("目标主机不能为空");
        const result = await invoke(command, { req });
        if (disposed) return;
        if (Number(config.fn.value) === fn) {
          config.latest.value = result;
          config.lastRequest = snapshot;
          refreshModbusRows(config);
        }
        const functionText = `FC${fn.toString(16).toUpperCase().padStart(2, "0")}`;
        addExchange(sessionId, result, `${functionText} ${read ? "读取" : "写入"}完成`);
        if (!read) addTimeline({ sessionId, direction: "sys", summary: `${functionText} 写入确认 · 地址 ${formatModbusAddress(address, functionArea(fn), snapshot.addressMode.value)} · ${quantity} 点` });
      } catch (cause) {
        stopPolling(config);
        config.error.value = reportError(sessionId, cause, "Modbus 请求失败");
      } finally {
        config.busy.value = false;
      }
    }

    function refreshModbusRows(config) {
      persist();
      if (!config.lastRequest || !config.latest.value) return;
      const snapshot = { ...config.lastRequest, format: config.format, wordOrder: config.wordOrder };
      config.rows.value = buildModbusRows(snapshot, config.latest.value, snapshot.address, snapshot.quantity);
    }

    async function togglePolling(config, sessionId) {
      if (disposed) return;
      if (config.polling.value) {
        stopPolling(config);
        addTimeline({ sessionId, direction: "sys", summary: "轮询已停止" });
        return;
      }
      if (!isReadFunction(config.fn.value)) {
        config.error.value = "写功能码不能启动周期轮询";
        return;
      }
      let interval;
      try { interval = requireInteger(config.pollMs.value, 100, 60000, "轮询周期"); }
      catch (cause) { config.error.value = String(cause); return; }
      config.polling.value = true;
      addTimeline({ sessionId, direction: "sys", summary: `轮询已启动 · ${interval} ms` });
      await runModbus(config, sessionId);
      if (disposed || !config.polling.value) return;
      config.pollTimer = setInterval(
        () => {
          if (disposed || !isReadFunction(config.fn.value)) { stopPolling(config); return; }
          runModbus(config, sessionId);
        },
        interval,
      );
    }

    async function refreshMemory() {
      if (disposed) return;
      const token = ++memoryToken;
      const area = memoryArea.value;
      const startText = memoryStart.value;
      const countText = memoryCount.value;
      const isCurrent = () => !disposed && token === memoryToken && area === memoryArea.value && startText === memoryStart.value && countText === memoryCount.value;
      if (memoryRows.value.some((row) => row.area !== area)) memoryRows.value = [];
      memoryBusy.value = true;
      memoryError.value = "";
      try {
        const start = requireInteger(startText, 0, 65535, "起始地址");
        const count = requireInteger(countText, 1, 256, "数量");
        if (start + count > 65536) throw new Error("数据区地址范围超出 65535");
        const result = await invoke("protocol_server_get_memory", {
          area,
          address: start,
          quantity: count,
        });
        if (!isCurrent()) return;
        memoryRows.value = (result.values || []).map((value, index) => ({
          area,
          address: start + index,
          label: formatModbusAddress(start + index, area, "zero"),
          value: Number(value) || 0,
          writing: false,
        }));
      } catch (cause) {
        if (isCurrent()) memoryError.value = String(cause);
      } finally {
        if (token === memoryToken) memoryBusy.value = false;
      }
    }

    async function writeMemory(row, value) {
      if (disposed || row.writing || row.area !== memoryArea.value) return;
      memoryError.value = "";
      const area = row.area;
      const sessionId = activeSession.value;
      const isBit = area === "coil" || area === "discrete";
      row.writing = true;
      try {
        const normalized = isBit ? (value ? 1 : 0) : requireInteger(value, 0, 65535, "寄存器值");
        await invoke("protocol_server_set_memory", { req: {
          area,
          address: row.address,
          values: [normalized],
        }});
        row.value = normalized;
        addTimeline({ sessionId, direction: "sys", summary: `数据区 ${area} · ${row.address} = ${normalized}` });
        if (!disposed && area === memoryArea.value) await refreshMemory();
      } catch (cause) {
        const message = reportError(sessionId, cause, "数据区写入失败");
        if (!disposed && area === memoryArea.value) memoryError.value = message;
      } finally {
        row.writing = false;
      }
    }

    async function refreshSerialPorts() {
      try {
        const result = await invoke("serial_list_ports");
        serialPorts.value = Array.isArray(result) ? result : [];
        const first = serialPorts.value[0]?.name || "";
        for (const config of [modbusRtuClient, modbusRtuServer]) {
          if (!config.serialPort.value && first) config.serialPort.value = first;
        }
      } catch (cause) {
        globalError.value = String(cause);
      }
    }

    function activateSession(id) {
      if (!SESSION_INDEX[id]) return;
      activeSession.value = id;
      selectedEvent.value = "";
      if (id.startsWith("modbus-") && id.endsWith("server")) refreshMemory();
      persist();
    }

    function visibleTimeline() {
      const query = timelineSearch.value.trim().toLowerCase();
      return timeline.value.filter((item) => {
        if (timelineFilter.value === "current" && item.sessionId !== activeSession.value) return false;
        if (["tx", "rx", "err", "sys"].includes(timelineFilter.value) && item.direction !== timelineFilter.value) return false;
        if (!query) return true;
        const session = SESSION_INDEX[item.sessionId]?.label || item.sessionId;
        return [session, item.endpoint, item.hex, item.text, item.summary, item.detail]
          .some((value) => String(value || "").toLowerCase().includes(query));
      }).slice().reverse();
    }

    function selectedTimelineEvent() {
      return timeline.value.find((item) => item.id === selectedEvent.value) || null;
    }

    async function copyText(value, label = "已复制") {
      const text = String(value || "");
      try {
        if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
          await navigator.clipboard.writeText(text);
        } else if (typeof document !== "undefined") {
          const textarea = document.createElement("textarea");
          textarea.value = text;
          textarea.style.position = "fixed";
          textarea.style.opacity = "0";
          document.body.appendChild(textarea);
          textarea.select();
          document.execCommand("copy");
          textarea.remove();
        }
        copyState.value = label;
        setTimeout(() => { copyState.value = ""; }, 1400);
      } catch (cause) {
        globalError.value = `复制失败：${cause}`;
      }
    }

    function exportTimeline() {
      const records = timeline.value.map((item) => ({
        timestamp: new Date(item.timestampMs).toISOString(),
        session: item.sessionId,
        direction: item.direction,
        endpoint: item.endpoint,
        byteLength: item.byteLength,
        summary: item.summary,
        hex: item.hex,
        text: item.text,
        detail: item.detail,
      }));
      const blob = new Blob([records.map((item) => JSON.stringify(item)).join("\n")], { type: "application/x-ndjson" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `protocol-workbench-${Date.now()}.jsonl`;
      link.click();
      URL.revokeObjectURL(url);
    }

    function clearTimeline() {
      timeline.value = [];
      selectedEvent.value = "";
    }

    async function bindListeners() {
      const bind = async (name, callback) => {
        if (disposed) return;
        const unlisten = await listen(name, (event) => { if (!disposed) callback(event); });
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      };
      await bind("protocol-client:event", (event) => {
        const item = event.payload || {};
        tcpClient.latest.value = { ...tcpClient.latest.value, ...item, elapsed_ms: null };
        addTimeline({
          sequence: item.sequence,
          timestamp_ms: item.timestamp_ms,
          sessionId: "tcp-client",
          direction: "rx",
          endpoint: item.peer,
          byteLength: item.received_bytes,
          hex: item.received_hex,
          text: item.received_text,
          summary: `TCP 收到 ${item.received_bytes || 0} B · ${item.kind || "raw"}`,
        });
      });
      await bind("protocol-client:status", refreshStatus);
      await bind("protocol-server:event", (event) => {
        const item = event.payload || {};
        const sessionId = SERVER_SESSION[item.kind];
        if (!sessionId) return;
        const config = configs[sessionId];
        config.latest.value = item;
        if (Number(item.received_bytes || 0) > 0) {
          addTimeline({
            sequence: item.sequence,
            timestamp_ms: item.timestamp_ms,
            sessionId,
            direction: "rx",
            endpoint: item.peer,
            byteLength: item.received_bytes,
            hex: item.received_hex,
            text: item.received_text,
            summary: `Server 收到请求 · ${item.frame_kind || "raw"}`,
          });
        }
        if (Number(item.sent_bytes || 0) > 0) {
          addTimeline({
            sequence: item.sequence,
            timestamp_ms: item.timestamp_ms,
            sessionId,
            direction: "tx",
            endpoint: item.peer,
            byteLength: item.sent_bytes,
            hex: item.sent_hex,
            text: item.sent_text,
            summary: "Server 已应答",
          });
        }
        refreshStatus();
        if (item.kind.startsWith("modbus_")) refreshMemory();
      });
      await bind("protocol-server:status", refreshStatus);
      await bind("protocol-server:client", refreshStatus);
    }

    onMounted(async () => {
      restore();
      await Promise.all([refreshStatus(), refreshSerialPorts(), refreshMemory()]);
      if (disposed) return;
      await Promise.all(previewStates.map((state) => refreshPreview(state)));
      if (disposed) return;
      try { await bindListeners(); } catch (cause) { globalError.value = String(cause); }
      if (!disposed) statusTimer = setInterval(refreshStatus, 1200);
    });

    onBeforeUnmount(() => {
      disposed = true;
      memoryToken += 1;
      persist();
      if (statusTimer) clearInterval(statusTimer);
      previewStates.forEach((state) => { state.token += 1; if (state.timer) clearTimeout(state.timer); });
      [modbusTcpClient, modbusRtuClient].forEach(stopPolling);
      unlisteners.splice(0).forEach((unlisten) => { if (typeof unlisten === "function") unlisten(); });
    });

    function textInput(state, attrs = {}) {
      const { onInput, ...rest } = attrs;
      return h("input", {
        ...rest,
        value: state.value,
        onInput: (event) => {
          state.value = event.target.value;
          if (onInput) onInput(event);
        },
      });
    }

    function selectInput(state, options, attrs = {}) {
      const { onChange, ...rest } = attrs;
      return h("select", {
        ...rest,
        value: state.value,
        onChange: (event) => {
          state.value = event.target.value;
          if (onChange) onChange(event.target.value, event);
        },
      }, options.map(([value, label]) => h("option", { value }, label)));
    }

    function field(label, control, className = "") {
      return h("label", { class: `pw4-field ${className}`.trim() }, [
        h("span", { class: "pw4-field-label" }, label),
        control,
      ]);
    }

    function commandButton(label, action, options = {}) {
      const classes = ["pw4-button", options.kind || "", options.compact ? "compact" : ""].filter(Boolean).join(" ");
      return h("button", {
        type: "button",
        class: classes,
        disabled: Boolean(options.disabled),
        title: options.title || "",
        onClick: action,
      }, label);
    }

    function toggleControl(label, checked, onChange, disabled = false) {
      return h("label", { class: `pw4-toggle${disabled ? " disabled" : ""}` }, [
        h("input", { type: "checkbox", checked, disabled, onChange: (event) => onChange(event.target.checked) }),
        h("span", { class: "pw4-toggle-track", "aria-hidden": "true" }, [h("i")]),
        h("span", null, label),
      ]);
    }

    function segmented(state, options, onChange, className = "") {
      return h("div", { class: `pw4-segmented ${className}`.trim() }, options.map(([value, label]) => h("button", {
        type: "button",
        class: state.value === value ? "active" : "",
        onClick: () => {
          state.value = value;
          if (onChange) onChange(value);
        },
      }, label)));
    }

    function statusBadge(state, label) {
      return h("span", { class: `pw4-status ${state}` }, [h("i"), label]);
    }

    function panelHeader(title, subtitle, status) {
      return h("header", { class: "pw4-panel-head" }, [
        h("div", null, [h("h2", null, title), subtitle ? h("p", null, subtitle) : null]),
        status,
      ]);
    }

    function errorBanner(message, dismiss) {
      if (!message) return null;
      return h("div", { class: "pw4-alert", role: "alert" }, [
        h("strong", null, "操作失败"),
        h("span", null, message),
        dismiss ? h("button", { type: "button", title: "关闭", "aria-label": "关闭", onClick: dismiss }, "×") : null,
      ]);
    }

    function endpointFields(config) {
      const disabled = config.busy.value || Boolean(config.kind && serverFor(config.kind));
      if (config.transport === "rtu" || config.kind === "modbus_rtu") {
        const options = serialPorts.value.length
          ? serialPorts.value.map((item) => [item.name, item.description ? `${item.name} · ${item.description}` : item.name])
          : [["", "未发现串口"]];
        return [
          field("串口", selectInput(config.serialPort, options, { disabled, onChange: persist }), "span-2"),
          field("波特率", textInput(config.baudRate, { type: "number", min: "300", max: "4000000", disabled, onInput: persist })),
          field("校验", selectInput(config.parity, [["none", "None"], ["even", "Even"], ["odd", "Odd"]], { disabled, onChange: persist })),
          field("数据位", selectInput({ value: "8" }, [["8", "8"]], { disabled: true })),
          field("停止位", selectInput(config.stopBits, [["1", "1"], ["2", "2"]], { disabled, onChange: persist })),
          ...(config.kind === "modbus_rtu" ? [field("Unit ID", textInput(config.unitId, { type: "number", min: "1", max: "247", disabled, onInput: persist }))] : []),
          commandButton("刷新串口", refreshSerialPorts, { compact: true, disabled }),
        ];
      }
      return [
        field(config.kind === "tcp" || config.kind === "udp" ? "主机" : "目标主机", textInput(config.host, { spellcheck: false, disabled, onInput: persist }), "span-2"),
        field("端口", textInput(config.port, { type: "number", min: "1", max: "65535", disabled, onInput: persist })),
      ];
    }

    function framingFields(config) {
      if (config.kind !== "tcp") return [];
      return [
        field("接收分帧", selectInput(config.framingMode, FRAMING_MODES, { onChange: persist }), "span-2"),
        config.framingMode.value === "delimiter"
          ? field("分隔符 HEX", textInput(config.delimiterHex, { spellcheck: false, onInput: persist }), "span-2")
          : null,
        config.framingMode.value === "fixed"
          ? field("固定字节数", textInput(config.fixedLength, { type: "number", min: "1", max: "65535", onInput: persist }), "span-2")
          : null,
      ].filter(Boolean);
    }

    function composerPanel(state, title, actions = []) {
      const preview = state.preview.value;
      const placeholder = state.inputMode.value === "hex"
        ? "01 03 00 00 00 02"
        : state.inputMode.value === "base64" ? "AQMAAAAAAg==" : "输入报文";
      return h("section", { class: "pw4-composer" }, [
        h("header", { class: "pw4-section-head" }, [
          h("div", null, [
            h("h3", null, title),
            h("span", null, state.previewError.value
              ? "格式错误"
              : preview ? `${preview.byte_count || 0} B · ${preview.character_count || 0} 字符` : "0 B"),
          ]),
          segmented(state.inputMode, INPUT_MODES, () => schedulePreview(state), "small"),
        ]),
        h("div", { class: "pw4-composer-options" }, [
          state.inputMode.value === "text"
            ? field("编码", selectInput(state.textEncoding, TEXT_ENCODINGS, { onChange: () => schedulePreview(state) }))
            : null,
          field("结尾", selectInput(state.lineEnding, LINE_ENDINGS, { onChange: () => schedulePreview(state) })),
          state.inputMode.value === "text"
            ? toggleControl("解析转义", state.parseEscapes.value, (value) => { state.parseEscapes.value = value; schedulePreview(state); })
            : null,
          state.lineEnding.value === "custom"
            ? field("自定义 HEX", textInput(state.customSuffixHex, { spellcheck: false, onInput: () => schedulePreview(state) }))
            : null,
        ].filter(Boolean)),
        h("textarea", {
          class: state.previewError.value ? "invalid" : "",
          value: state.payload.value,
          placeholder,
          spellcheck: false,
          onInput: (event) => { state.payload.value = event.target.value; schedulePreview(state); },
        }),
        state.previewError.value
          ? h("p", { class: "pw4-inline-error" }, state.previewError.value)
          : h("div", { class: "pw4-byte-preview" }, [
              h("span", null, state.previewing.value ? "解析中" : "HEX"),
              h("code", null, preview?.hex || "—"),
              preview?.truncated ? h("em", null, `仅预览前 ${preview.preview_bytes} B`) : null,
            ]),
        actions.length ? h("footer", { class: "pw4-composer-actions" }, actions) : null,
      ]);
    }

    function latestExchange(config) {
      const result = config.latest.value;
      if (!result) return h("div", { class: "pw4-result-empty" }, "暂无收发结果");
      const received = Number(result.received_bytes || 0);
      return h("div", { class: "pw4-result-strip" }, [
        h("span", null, [h("b", null, `${result.sent_bytes || 0} B`), " TX"]),
        h("span", { class: received ? "good" : "" }, [h("b", null, `${received} B`), " RX"]),
        h("span", null, [h("b", null, result.elapsed_ms == null ? "—" : `${result.elapsed_ms} ms`), " 耗时"]),
        h("code", { title: result.peer || "" }, result.peer || "—"),
      ]);
    }

    function runtimeState(id) {
      const config = configs[id];
      if (config.error?.value) return ["error", "异常"];
      if (id === "tcp-client" && config.connected.value) return ["online", "已连接"];
      if (id === "udp-client") return ["idle", "就绪"];
      if (id.startsWith("modbus-") && id.endsWith("client") && config.polling.value) return ["running", "轮询"];
      if (id.endsWith("server")) {
        const server = serverFor(config.kind);
        if (server) return ["online", "运行"];
      }
      return ["idle", "空闲"];
    }

    function sessionNavigation() {
      return h("aside", { class: "pw4-session-rail", "aria-label": "协议会话" }, [
        h("div", { class: "pw4-rail-title" }, [h("span", null, "会话"), h("b", null, "8")]),
        ...SESSION_GROUPS.map((group) => h("section", { class: "pw4-session-group" }, [
          h("h3", null, group.label),
          ...group.items.map((item) => {
            const [state, label] = runtimeState(item.id);
            return h("button", {
              type: "button",
              class: `pw4-session${activeSession.value === item.id ? " active" : ""}`,
              onClick: () => activateSession(item.id),
            }, [
              h("span", { class: "pw4-session-code", "aria-hidden": "true" }, item.code),
              h("span", { class: "pw4-session-copy" }, [h("strong", null, item.label), h("small", null, label)]),
              h("i", { class: `pw4-session-dot ${state}`, title: label }),
            ]);
          }),
        ])),
      ]);
    }

    function timelineDetail() {
      const item = selectedTimelineEvent();
      if (!item) return null;
      const body = displayMode.value === "hex"
        ? item.hex
        : displayMode.value === "text" ? item.text : `HEX\n${item.hex || "—"}\n\nTEXT\n${item.text || "—"}`;
      return h("section", { class: "pw4-event-detail" }, [
        h("header", null, [
          h("div", null, [h("b", null, `${item.time} · ${item.direction.toUpperCase()}`), h("span", null, item.endpoint || "本地")]),
          h("button", { type: "button", title: "关闭详情", "aria-label": "关闭详情", onClick: () => { selectedEvent.value = ""; } }, "×"),
        ]),
        segmented(displayMode, [["dual", "双栏"], ["hex", "HEX"], ["text", "文本"]], null, "small"),
        h("pre", null, body || item.detail || "—"),
        h("footer", null, [
          h("span", null, `${item.byteLength} B · ${SESSION_INDEX[item.sessionId]?.label || item.sessionId}`),
          commandButton(copyState.value || "复制内容", () => copyText(body || item.detail, "已复制"), { compact: true }),
        ]),
      ]);
    }

    function timelinePanel() {
      const items = visibleTimeline();
      return h("aside", { class: "pw4-timeline" }, [
        h("header", { class: "pw4-timeline-head" }, [
          h("div", null, [h("h2", null, "通信时间线"), h("span", null, `${items.length} / ${timeline.value.length}`)]),
          h("div", { class: "pw4-timeline-actions" }, [
            commandButton("导出", exportTimeline, { compact: true, disabled: !timeline.value.length }),
            commandButton("清空", clearTimeline, { compact: true, disabled: !timeline.value.length }),
          ]),
        ]),
        h("div", { class: "pw4-timeline-tools" }, [
          selectInput(timelineFilter, [["all", "全部"], ["current", "当前会话"], ["tx", "仅 TX"], ["rx", "仅 RX"], ["err", "仅错误"], ["sys", "系统事件"]]),
          h("input", {
            type: "search",
            value: timelineSearch.value,
            placeholder: "搜索端点或报文",
            onInput: (event) => { timelineSearch.value = event.target.value; },
          }),
        ]),
        h("div", { class: "pw4-event-list" }, items.length
          ? items.map((item) => h("button", {
              type: "button",
              class: `pw4-event ${item.direction}${selectedEvent.value === item.id ? " selected" : ""}`,
              onClick: () => { selectedEvent.value = item.id; },
            }, [
              h("span", { class: "pw4-event-meta" }, [
                h("time", null, item.time),
                h("b", null, item.direction.toUpperCase()),
                h("em", null, `${item.byteLength} B`),
              ]),
              h("strong", null, item.summary || "通信事件"),
              h("small", null, SESSION_INDEX[item.sessionId]?.label || item.sessionId),
              h("code", null, item.hex || item.detail || item.text || "—"),
            ]))
          : [h("div", { class: "pw4-timeline-empty" }, [h("b", null, "暂无通信记录"), h("span", null, timelineSearch.value ? "没有匹配项" : "等待会话事件")])]) ,
        timelineDetail(),
      ]);
    }

    function rawClientPanel(config, sessionId) {
      const isTcp = config.kind === "tcp";
      const connected = isTcp && config.connected.value;
      const status = connected
        ? statusBadge("online", "已连接")
        : statusBadge(isTcp ? "idle" : "ready", isTcp ? "未连接" : "数据报模式");
      const sendDisabled = config.busy.value || Boolean(config.composer.previewError.value) || !Number(config.composer.preview.value?.byte_count || 0);
      return h("section", { class: "pw4-panel" }, [
        panelHeader(
          isTcp ? "TCP Client" : "UDP Client",
          connected ? config.peer.value : `${config.host.value || "—"}:${config.port.value || "—"}`,
          status,
        ),
        errorBanner(config.error.value, () => { config.error.value = ""; }),
        h("section", { class: "pw4-config-block" }, [
          h("div", { class: "pw4-section-head" }, [h("div", null, [h("h3", null, "连接参数"), h("span", null, isTcp ? "TCP" : "UDP")])]),
          h("div", { class: "pw4-form-grid" }, [
            ...endpointFields(config),
            field("超时 (ms)", textInput(config.timeoutMs, { type: "number", min: "100", max: "30000", onInput: persist })),
            ...framingFields(config),
          ]),
          h("footer", { class: "pw4-config-actions" }, isTcp ? [
            connected
              ? commandButton(config.busy.value ? "断开中" : "断开", disconnectTcpClient, { kind: "danger", disabled: config.busy.value })
              : commandButton(config.busy.value ? "连接中" : "连接", connectTcpClient, { kind: "primary", disabled: config.busy.value }),
            commandButton("一次收发", runTcpExchangeOnce, { disabled: config.busy.value || sendDisabled }),
          ] : [statusBadge("ready", "无需连接")]),
        ]),
        composerPanel(config.composer, "发送报文", [
          commandButton(
            config.busy.value ? "发送中" : "发送",
            () => sendRawClient(config, sessionId),
            { kind: "primary", disabled: sendDisabled || (isTcp && !connected) },
          ),
        ]),
        h("section", { class: "pw4-last-result" }, [
          h("div", { class: "pw4-section-head" }, [h("div", null, [h("h3", null, "最近一次"), h("span", null, "TX / RX / RTT")])]),
          latestExchange(config),
        ]),
      ]);
    }

    function clientTable(clients) {
      return h("div", { class: "pw4-table-wrap compact" }, clients.length
        ? [h("table", null, [
            h("thead", null, [h("tr", null, [h("th", null, "客户端"), h("th", null, "状态"), h("th", null, "请求"), h("th", null, "RX / TX")])]) ,
            h("tbody", null, clients.map((client) => h("tr", null, [
              h("td", null, [h("code", null, client.peer)]),
              h("td", null, [statusBadge(client.online === false ? "idle" : "online", client.online === false ? "离线" : "在线")]),
              h("td", null, String(client.requests || 0)),
              h("td", null, [h("code", null, `${client.rx_bytes || 0} / ${client.tx_bytes || 0}`)]),
            ]))),
          ])]
        : [h("div", { class: "pw4-table-empty" }, "暂无客户端")]);
    }

    function rawServerPanel(config, sessionId) {
      const server = serverFor(config.kind);
      const clients = server?.clients || [];
      const isTcp = config.kind === "tcp";
      const composerTitle = config.responseMode.value === "fixed" ? "固定应答 / 主动发送" : "主动发送报文";
      const sendDisabled = config.busy.value || !server || !clients.some((item) => item.online !== false)
        || Boolean(config.composer.previewError.value) || !Number(config.composer.preview.value?.byte_count || 0);
      return h("section", { class: "pw4-panel" }, [
        panelHeader(
          isTcp ? "TCP Server" : "UDP Server",
          server?.address || `${config.host.value || "—"}:${config.port.value || "—"}`,
          statusBadge(server ? "online" : "idle", server ? "监听中" : "已停止"),
        ),
        errorBanner(config.error.value, () => { config.error.value = ""; }),
        h("section", { class: "pw4-config-block" }, [
          h("div", { class: "pw4-section-head" }, [
            h("div", null, [h("h3", null, "监听参数"), h("span", null, server ? `${server.requests || 0} 个请求` : isTcp ? "TCP" : "UDP")]),
          ]),
          h("div", { class: "pw4-form-grid" }, [
            ...endpointFields(config),
            field("应答模式", selectInput(config.responseMode, RESPONSE_MODES, { onChange: persist }), "span-2"),
            ...framingFields(config),
          ]),
          h("footer", { class: "pw4-config-actions" }, [
            server
              ? commandButton(config.busy.value ? "停止中" : "停止监听", () => stopServer(config, sessionId), { kind: "danger", disabled: config.busy.value })
              : commandButton(config.busy.value ? "启动中" : "启动监听", () => startServer(config, sessionId), { kind: "primary", disabled: config.busy.value }),
            server ? statusBadge("online", isTcp ? `${clients.filter((item) => item.online !== false).length} 个在线客户端` : "套接字已绑定") : null,
          ].filter(Boolean)),
        ]),
        (config.responseMode.value === "fixed" || isTcp)
          ? composerPanel(config.composer, composerTitle, isTcp ? [
              field("目标", h("select", {
                value: config.peer.value,
                onChange: (event) => { config.peer.value = event.target.value; },
              }, [
                h("option", { value: "*" }, "全部在线客户端"),
                ...clients.filter((item) => item.online !== false).map((item) => h("option", { value: item.peer }, item.peer)),
              ]), "peer-field"),
              commandButton("Server 发送", () => sendFromTcpServer(config, sessionId), { kind: "primary", disabled: sendDisabled }),
            ] : [])
          : null,
        isTcp ? h("section", { class: "pw4-clients" }, [
          h("div", { class: "pw4-section-head" }, [h("div", null, [h("h3", null, "客户端"), h("span", null, `${clients.length} 条记录`)])]),
          clientTable(clients),
        ]) : h("section", { class: "pw4-last-result" }, [
          h("div", { class: "pw4-section-head" }, [h("div", null, [h("h3", null, "最近数据报"), h("span", null, server ? `${server.requests || 0} 个请求` : "0 个请求")])]),
          latestExchange(config),
        ]),
      ]);
    }

    function modbusResultTable(config) {
      const rows = config.rows.value;
      const latest = config.latest.value;
      if (!rows.length) {
        return h("div", { class: "pw4-table-empty large" }, latest?.write_ack
          ? `写入已确认 · 地址 ${latest.write_ack.address} · ${latest.write_ack.quantity} 点`
          : "暂无寄存器数据");
      }
      return h("div", { class: "pw4-table-wrap modbus" }, [h("table", null, [
        h("thead", null, [h("tr", null, [h("th", null, "地址"), h("th", null, "原始值"), h("th", null, "解析值")])]) ,
        h("tbody", null, rows.map((row) => h("tr", null, [
          h("td", null, [h("code", null, row.label)]),
          h("td", null, [h("code", { class: "muted" }, row.raw)]),
          h("td", { class: "pw4-value" }, row.value),
        ]))),
      ])]);
    }

    function modbusClientPanel(config, sessionId) {
      const fn = Number(config.fn.value);
      const read = isReadFunction(fn);
      const bitFunction = fn === 1 || fn === 2 || fn === 5 || fn === 15;
      const transportLabel = config.transport === "tcp" ? "Modbus TCP" : "Modbus RTU";
      const endpoint = config.transport === "tcp"
        ? `${config.host.value || "—"}:${config.port.value || "—"}`
        : `${config.serialPort.value || "未选择串口"} · ${config.baudRate.value}`;
      return h("section", { class: "pw4-panel" }, [
        panelHeader(
          `${transportLabel} Client`,
          endpoint,
          statusBadge(config.polling.value ? "running" : "ready", config.polling.value ? `${config.pollMs.value} ms 轮询` : "就绪"),
        ),
        errorBanner(config.error.value, () => { config.error.value = ""; }),
        h("section", { class: "pw4-config-block" }, [
          h("div", { class: "pw4-section-head" }, [h("div", null, [h("h3", null, "通道"), h("span", null, config.transport.toUpperCase())])]),
          h("div", { class: "pw4-form-grid" }, [
            ...endpointFields(config),
            field("超时 (ms)", textInput(config.timeoutMs, { type: "number", min: "100", max: "30000", onInput: persist })),
          ]),
        ]),
        h("section", { class: "pw4-request" }, [
          h("div", { class: "pw4-section-head" }, [
            h("div", null, [h("h3", null, "请求"), h("span", null, `FC${fn.toString(16).toUpperCase().padStart(2, "0")}`)]),
            segmented(config.addressMode, [["zero", "0 基址"], ["plc", "PLC 地址"]], persist, "small"),
          ]),
          h("div", { class: "pw4-form-grid request-grid" }, [
            field("Unit ID", textInput(config.unitId, { type: "number", min: config.transport === "tcp" ? "0" : "1", max: config.transport === "tcp" ? "255" : "247", onInput: persist })),
            field("功能码", selectInput(config.fn, MODBUS_FUNCTIONS, { onChange: () => { stopPolling(config); config.rows.value = []; config.lastRequest = null; config.latest.value = null; persist(); } }), "span-2"),
            field("起始地址", textInput(config.address, { inputmode: "numeric", spellcheck: false, onInput: persist }), "span-2"),
            read
              ? field("数量", textInput(config.quantity, { type: "number", min: "1", max: fn <= 2 ? "2000" : "125", onInput: persist }))
              : field("写入值", textInput(config.values, { spellcheck: false, placeholder: bitFunction ? "0, 1, 1, 0" : "0, 100, 0x00FF", onInput: persist }), "span-3"),
            read && !bitFunction ? field("显示格式", selectInput(config.format, DATA_FORMATS, { onChange: () => refreshModbusRows(config) }), "span-2") : null,
            read && !bitFunction && registersPerValue(config.format.value) > 1
              ? field("字节 / 字序", selectInput(config.wordOrder, WORD_ORDERS, { onChange: () => refreshModbusRows(config) }), "span-2")
              : null,
            read ? field("轮询周期 (ms)", textInput(config.pollMs, { type: "number", min: "100", max: "60000", onInput: persist })) : null,
          ].filter(Boolean)),
          h("footer", { class: "pw4-config-actions request-actions" }, [
            commandButton(config.busy.value ? "执行中" : "执行一次", () => runModbus(config, sessionId), { kind: "primary", disabled: config.busy.value }),
            read ? commandButton(config.polling.value ? "停止轮询" : "启动轮询", () => togglePolling(config, sessionId), { kind: config.polling.value ? "danger" : "", disabled: config.busy.value && !config.polling.value }) : null,
            config.latest.value ? h("span", { class: "pw4-request-metrics" }, `${config.latest.value.elapsed_ms || 0} ms · TX ${config.latest.value.sent_bytes || 0} B · RX ${config.latest.value.received_bytes || 0} B`) : null,
          ].filter(Boolean)),
        ]),
        h("section", { class: "pw4-registers" }, [
          h("div", { class: "pw4-section-head" }, [
            h("div", null, [h("h3", null, read ? "数据" : "写入确认"), h("span", null, `${config.rows.value.length} 行`)]),
            config.latest.value ? commandButton("复制 RX", () => copyText(config.latest.value.received_hex, "RX 已复制"), { compact: true }) : null,
          ]),
          modbusResultTable(config),
        ]),
      ]);
    }

    function memoryValueEditor(row) {
      const bitArea = row.area === "coil" || row.area === "discrete";
      if (bitArea) {
        return toggleControl(row.value ? "ON" : "OFF", Boolean(row.value), (checked) => writeMemory(row, checked), row.writing);
      }
      return h("input", {
        type: "number",
        min: "0",
        max: "65535",
        value: row.value,
        disabled: row.writing,
        onChange: async (event) => {
          const input = event.target;
          await writeMemory(row, input.value);
          input.value = row.value;
        },
      });
    }

    function memoryTable() {
      return h("div", { class: "pw4-table-wrap memory" }, memoryRows.value.length
        ? [h("table", null, [
            h("thead", null, [h("tr", null, [h("th", null, "偏移地址"), h("th", null, "参考地址"), h("th", null, "模拟值")])]) ,
            h("tbody", null, memoryRows.value.map((row) => h("tr", null, [
              h("td", null, [h("code", null, String(row.address))]),
              h("td", null, [h("code", { class: "muted" }, formatModbusAddress(row.address, row.area, "plc"))]),
              h("td", null, [memoryValueEditor(row)]),
            ]))),
          ])]
        : [h("div", { class: "pw4-table-empty large" }, "暂无模拟数据")]);
    }

    function modbusServerPanel(config, sessionId) {
      const server = serverFor(config.kind);
      const isTcp = config.transport === "tcp";
      const clients = server?.clients || [];
      const endpoint = isTcp
        ? server?.address || `${config.host.value || "—"}:${config.port.value || "—"}`
        : server?.address || `${config.serialPort.value || "未选择串口"} · ${config.baudRate.value}`;
      return h("section", { class: "pw4-panel" }, [
        panelHeader(
          `Modbus ${isTcp ? "TCP" : "RTU"} Server`,
          endpoint,
          statusBadge(server ? "online" : "idle", server ? "运行中" : "已停止"),
        ),
        errorBanner(config.error.value, () => { config.error.value = ""; }),
        h("section", { class: "pw4-config-block" }, [
          h("div", { class: "pw4-section-head" }, [
            h("div", null, [h("h3", null, "从站通道"), h("span", null, server ? `${server.requests || 0} 个请求` : isTcp ? "TCP" : "RTU")]),
          ]),
          h("div", { class: "pw4-form-grid" }, endpointFields(config)),
          h("footer", { class: "pw4-config-actions" }, [
            server
              ? commandButton(config.busy.value ? "停止中" : "停止 Server", () => stopServer(config, sessionId), { kind: "danger", disabled: config.busy.value })
              : commandButton(config.busy.value ? "启动中" : "启动 Server", () => startServer(config, sessionId), { kind: "primary", disabled: config.busy.value }),
            server ? statusBadge("online", isTcp ? "响应所有 Unit ID" : `Unit ID ${config.unitId.value}`) : null,
          ].filter(Boolean)),
        ]),
        h("section", { class: "pw4-memory" }, [
          h("div", { class: "pw4-section-head memory-head" }, [
            h("div", null, [h("h3", null, "从站数据区"), h("span", null, `${memoryRows.value.length} 个地址`)]) ,
            segmented(memoryArea, MEMORY_AREAS, () => { persist(); refreshMemory(); }, "small"),
          ]),
          h("div", { class: "pw4-memory-range" }, [
            field("起始", textInput(memoryStart, { type: "number", min: "0", max: "65535", onInput: persist })),
            field("数量", textInput(memoryCount, { type: "number", min: "1", max: "256", onInput: persist })),
            commandButton(memoryBusy.value ? "刷新中" : "刷新", refreshMemory, { disabled: memoryBusy.value }),
          ]),
          errorBanner(memoryError.value, () => { memoryError.value = ""; }),
          memoryTable(),
        ]),
        isTcp ? h("section", { class: "pw4-clients server-clients" }, [
          h("div", { class: "pw4-section-head" }, [h("div", null, [h("h3", null, "客户端"), h("span", null, `${clients.filter((item) => item.online !== false).length} 个在线`)])]),
          clientTable(clients),
        ]) : null,
      ]);
    }

    function activePanel() {
      switch (activeSession.value) {
        case "tcp-client": return rawClientPanel(tcpClient, "tcp-client");
        case "udp-client": return rawClientPanel(udpClient, "udp-client");
        case "tcp-server": return rawServerPanel(tcpServer, "tcp-server");
        case "udp-server": return rawServerPanel(udpServer, "udp-server");
        case "modbus-tcp-client": return modbusClientPanel(modbusTcpClient, "modbus-tcp-client");
        case "modbus-rtu-client": return modbusClientPanel(modbusRtuClient, "modbus-rtu-client");
        case "modbus-tcp-server": return modbusServerPanel(modbusTcpServer, "modbus-tcp-server");
        case "modbus-rtu-server": return modbusServerPanel(modbusRtuServer, "modbus-rtu-server");
        default: return null;
      }
    }

    function topbar() {
      const active = SESSION_INDEX[activeSession.value];
      const runningServers = servers.value.filter((item) => item.running).length;
      return h("header", { class: "pw4-topbar" }, [
        h("div", { class: "pw4-title" }, [
          h("span", { class: "pw4-mark", "aria-hidden": "true" }, "PX"),
          h("div", null, [h("h1", null, "协议工作台"), h("p", null, `${active?.label || "会话"} · ${active?.role || ""}`)]),
        ]),
        h("div", { class: "pw4-runtime-summary" }, [
          statusBadge(tcpClient.connected.value ? "online" : "idle", tcpClient.connected.value ? "TCP 已连接" : "TCP 空闲"),
          statusBadge(runningServers ? "running" : "idle", `${runningServers} 个 Server`),
          h("span", { class: "pw4-event-count" }, `${timeline.value.length} 条事件`),
        ]),
      ]);
    }

    return () => h("div", { class: "protocol-workbench-v4" }, [
      topbar(),
      globalError.value ? errorBanner(globalError.value, () => { globalError.value = ""; }) : null,
      h("div", { class: "pw4-layout" }, [
        sessionNavigation(),
        h("main", { class: "pw4-workspace" }, [activePanel()]),
        timelinePanel(),
      ]),
    ]);
  },
});
