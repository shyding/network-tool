import { a as defineComponent, r as ref, o as onMounted, e as h, t as invoke } from "./index-DNL4tSY9.js";

export default defineComponent({
  name: "ProtocolWorkbenchView",
  setup() {
    const mode = ref("modbus_tcp");
    const host = ref("127.0.0.1");
    const port = ref(502);
    const timeoutMs = ref(2000);
    const encoding = ref("hex");
    const payload = ref("01 03 00 00 00 02");
    const unitId = ref(1);
    const fn = ref(3);
    const address = ref(0);
    const quantity = ref(2);
    const values = ref("0");
    const serialPort = ref("");
    const baudRate = ref(9600);
    const parity = ref("none");
    const serialPorts = ref([]);
    const catalog = ref({ groups: [], implemented: [] });
    const busy = ref(false);
    const error = ref("");
    const result = ref(null);
    const history = ref([]);
    const modes = [
      ["modbus_tcp", "Modbus TCP"], ["modbus_rtu", "Modbus RTU"],
      ["tcp", "TCP Client"], ["udp", "UDP Client"]
    ];

    onMounted(async () => {
      try { catalog.value = await invoke("protocol_workbench_catalog"); } catch (_) {}
      try {
        serialPorts.value = await invoke("serial_list_ports");
        if (serialPorts.value.length) serialPort.value = serialPorts.value[0].name;
      } catch (_) {}
    });

    const numberValues = () => values.value.split(/[\s,;]+/).filter(Boolean).map(v => {
      const text = String(v).trim();
      return text.toLowerCase().startsWith("0x") ? parseInt(text.slice(2), 16) : Number(text);
    }).filter(Number.isFinite).map(v => Math.max(0, Math.min(65535, Math.trunc(v))));

    async function execute() {
      busy.value = true; error.value = ""; result.value = null;
      const started = new Date();
      try {
        let command, req;
        if (mode.value === "tcp" || mode.value === "udp") {
          command = mode.value === "tcp" ? "protocol_tcp_exchange" : "protocol_udp_exchange";
          req = { host:host.value, port:Number(port.value), payload:payload.value, encoding:encoding.value, timeout_ms:Number(timeoutMs.value) };
        } else if (mode.value === "modbus_tcp") {
          command = "protocol_modbus_tcp";
          req = { host:host.value, port:Number(port.value), unit_id:Number(unitId.value), function:Number(fn.value), address:Number(address.value), quantity:Number(quantity.value), values:numberValues(), timeout_ms:Number(timeoutMs.value) };
        } else {
          command = "protocol_modbus_rtu";
          req = { port_name:serialPort.value, baud_rate:Number(baudRate.value), data_bits:8, stop_bits:1, parity:parity.value, unit_id:Number(unitId.value), function:Number(fn.value), address:Number(address.value), quantity:Number(quantity.value), values:numberValues(), timeout_ms:Number(timeoutMs.value) };
        }
        result.value = await invoke(command, { req });
        history.value = [{ time:started.toLocaleTimeString(), mode:modes.find(v=>v[0]===mode.value)?.[1], ok:true, summary:`TX ${result.value.sent_bytes} / RX ${result.value.received_bytes} · ${result.value.elapsed_ms} ms` }, ...history.value].slice(0, 50);
      } catch (cause) {
        error.value = String(cause);
        history.value = [{ time:started.toLocaleTimeString(), mode:modes.find(v=>v[0]===mode.value)?.[1], ok:false, summary:error.value }, ...history.value].slice(0, 50);
      } finally { busy.value = false; }
    }

    const field = (label, child, wide=false) => h("label", { class:["pw-field", wide&&"wide"] }, [h("span", null, label), child]);
    const input = (state, type="text", attrs={}) => h("input", { type, value:state.value, ...attrs, onInput:e=>state.value=e.target.value });
    const select = (state, options) => h("select", { value:state.value, onChange:e=>state.value=e.target.value }, options.map(v=>h("option", { value:v[0] }, v[1])));

    return () => h("div", { class:"protocol-workbench" }, [
      h("header", { class:"pw-head" }, [
        h("div", null, [h("h1", null, "工业协议调试台"), h("p", null, "参考 HslCommunication Demo 的调试模式 · 请求/响应可追溯")]),
        h("span", { class:"pw-ready" }, `${catalog.value.implemented?.length || 5} 项已可用`)
      ]),
      h("nav", { class:"pw-tabs" }, modes.map(item=>h("button", { class:mode.value===item[0]?"on":"", onClick:()=>{mode.value=item[0]; if(item[0]==="modbus_tcp")port.value=502;} }, item[1]))),
      h("div", { class:"pw-layout" }, [
        h("section", { class:"pw-card pw-form" }, [
          h("h2", null, "连接与报文"),
          h("div", { class:"pw-grid" }, [
            ...(mode.value!=="modbus_rtu" ? [field("目标主机",input(host)),field("端口",input(port,"number",{min:1,max:65535}))] : [
              field("串口",select(serialPort,serialPorts.value.map(p=>[p.name,`${p.name} · ${p.description}`]))),
              field("波特率",select(baudRate,[[9600,"9600"],[19200,"19200"],[38400,"38400"],[57600,"57600"],[115200,"115200"]]))
            ]),
            field("超时(ms)",input(timeoutMs,"number",{min:100,max:30000})),
            ...(mode.value==="modbus_rtu" ? [field("校验",select(parity,[["none","None"],["even","Even"],["odd","Odd"]]))] : []),
            ...(mode.value.startsWith("modbus") ? [
              field("站号",input(unitId,"number",{min:0,max:255})),
              field("功能码",select(fn,[[1,"01 读线圈"],[2,"02 读离散量"],[3,"03 读保持寄存器"],[4,"04 读输入寄存器"],[5,"05 写单线圈"],[6,"06 写单寄存器"],[15,"0F 写多线圈"],[16,"10 写多寄存器"]])),
              field("起始地址",input(address,"number",{min:0,max:65535})),
              field("数量",input(quantity,"number",{min:1,max:2000})),
              field("写入值（逗号分隔，支持 0x）",input(values),true)
            ] : [
              field("数据格式",select(encoding,[["hex","HEX"],["ascii","UTF-8 文本"]])),
              field("发送数据",h("textarea",{value:payload.value,onInput:e=>payload.value=e.target.value,spellcheck:false}),true)
            ])
          ]),
          h("button", { class:"pw-run", disabled:busy.value, onClick:execute }, busy.value?"执行中…":"发送 / 执行") ,
          error.value ? h("div", { class:"pw-error" }, error.value) : null
        ]),
        h("section", { class:"pw-card pw-result" }, [
          h("h2", null, "响应解析"),
          result.value ? h("div", null, [
            h("div",{class:"pw-metrics"},[
              h("span",null,`${result.value.elapsed_ms} ms`),h("span",null,`发送 ${result.value.sent_bytes} B`),h("span",null,`接收 ${result.value.received_bytes} B`)
            ]),
            h("h3",null,"发送 HEX"),h("pre",null,result.value.sent_hex||"—"),
            h("h3",null,"接收 HEX"),h("pre",null,result.value.received_hex||"（无响应数据）"),
            h("h3",null,"文本 / 协议数据"),h("pre",null,result.value.received_text||result.value.data_hex||"—")
          ]) : h("div",{class:"pw-empty"},"配置连接参数后发送报文，结果将在这里显示。")
        ])
      ]),
      h("div",{class:"pw-bottom"},[
        h("section",{class:"pw-card"},[h("h2",null,"调试历史"),history.value.length?h("div",{class:"pw-history"},history.value.map(row=>h("div",{class:row.ok?"ok":"bad"},[h("time",null,row.time),h("b",null,row.mode),h("span",null,row.summary)]))):h("p",{class:"pw-muted"},"暂无记录")]),
        h("section",{class:"pw-card"},[h("h2",null,"参考工程协议目录"),h("div",{class:"pw-catalog"},(catalog.value.groups||[]).map(group=>h("div",null,[h("b",null,group.name),h("p",null,group.items.join(" · "))])))])
      ])
    ]);
  }
});

