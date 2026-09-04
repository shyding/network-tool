import{a as $t,r as v,o as St,t as S,b as Ct,c as d,e,n as A,f as i,h as W,g as C,w as g,y as N,C as lt,k as at,u as ot,v as Lt,F as B,i as K,q as st,l as w,p as q,m as u,_ as Mt}from"./index-DNL4tSY9.js";import{s as nt}from"./index-dl-4Ci8K.js";import{u as zt,a as Nt,s as it,i as Ft}from"./useThemeColors-DEU6HLyc.js";import{a as H,d as ut,e as Tt}from"./beijingTime-OyBL5kLd.js";import{i as jt}from"./install-Bg0Syj5C.js";import{i as At}from"./install-B1KIHPBh.js";import{i as Vt}from"./install-CIgk3cSO.js";import{i as Ot}from"./install-DSEpQEQI.js";import"./points-qIwBD3ow.js";import"./dataSample-Cqp3lA6C.js";const ct="ntk_linkmon_config_v1",P={alert_feishu:!1,alert_wecom:!1,feishu_webhook:"",wecom_webhook:""};function rt(){try{const s=localStorage.getItem(ct);return s?{...P,...JSON.parse(s)}:{...P}}catch{return{...P}}}function dt(s){localStorage.setItem(ct,JSON.stringify(s))}function E(s){return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}function Ut(s,L){const y=s?"ok":L.includes("中断")?"bad":"warn",F=s?"OK":L.includes("中断")?"FAIL":"WARN";return`<span class="badge ${y}">${F}</span>`}function Wt(s){return s>=95?"avail-good":s>=80?"avail-warn":"avail-bad"}function It(s){const L=H(),y=new Map;for(const o of s){const p=y.get(o.target)||[];p.push(o),y.set(o.target,p)}const F=y.size,V=s.filter(o=>o.ok).length,T=s.length?V/s.length*100:0,I=s.reduce((o,p)=>o+(p.avg_latency_ms??0),0)/(s.length||1),b=s.reduce((o,p)=>o+p.loss_pct,0)/(s.length||1),O=[...y.entries()].map(([o,p])=>{const h=p.length,j=p.filter(n=>n.ok),_=p.reduce((n,z)=>n+(z.avg_latency_ms??0),0)/h,$=p.reduce((n,z)=>n+z.loss_pct,0)/h,r=p.reduce((n,z)=>n+z.jitter_ms,0)/h,J=h?j.length*100/h:0,M=Wt(J);return`<tr>
      <td class="mono target">${E(o)}</td>
      <td>${h}</td>
      <td><span class="avail ${M}">${J.toFixed(1)}%</span></td>
      <td>${_.toFixed(1)}</td>
      <td>${$.toFixed(1)}%</td>
      <td>${r.toFixed(1)}</td>
    </tr>`}),f=500,c=s.length>f?s.slice(-f):s,k=s.length>f?`<p class="note">明细仅展示最近 ${f} 条（共 ${s.length} 条）</p>`:"",U=c.map(o=>`<tr class="${o.ok?"row-ok":"row-bad"}">
      <td class="ts">${E(o.timestamp)}</td>
      <td class="mono">${E(o.target)}</td>
      <td>${E(o.status)}</td>
      <td>${Ut(o.ok,o.status)}</td>
      <td>${o.last_latency_ms??"—"}</td>
      <td>${o.avg_latency_ms??"—"}</td>
      <td>${o.loss_pct}</td>
      <td>${o.jitter_ms}</td>
    </tr>`);return`<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>链路监控报告 · 网络测试工具箱</title>
  <style>
    :root {
      --teal: #0d9488; --teal-light: #ecfdf5;
      --amber: #d97706; --rose: #e11d48;
      --text: #1e293b; --muted: #64748b;
      --line: #e2e8f0; --bg: #f8fafc;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0; font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
      background: var(--bg); color: var(--text); line-height: 1.5;
    }
    .hero {
      background: linear-gradient(135deg, #0f766e 0%, #0d9488 45%, #0284c7 100%);
      color: #fff; padding: 32px 40px 28px;
    }
    .hero h1 { margin: 0 0 6px; font-size: 26px; font-weight: 700; letter-spacing: .5px; }
    .hero .sub { opacity: .88; font-size: 14px; }
    .wrap { max-width: 1100px; margin: 0 auto; padding: 24px 28px 40px; }
    .kpis {
      display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px;
      margin: -36px 28px 24px; max-width: 1100px; margin-left: auto; margin-right: auto;
    }
    .kpi {
      background: #fff; border-radius: 12px; padding: 16px 18px;
      box-shadow: 0 4px 20px rgba(15,118,110,.12); border: 1px solid var(--line);
    }
    .kpi .label { font-size: 12px; color: var(--muted); margin-bottom: 4px; }
    .kpi .val { font-size: 22px; font-weight: 800; color: var(--teal); }
    .kpi .val.warn { color: var(--amber); }
    .kpi .val.bad { color: var(--rose); }
    section { margin-bottom: 28px; }
    section h2 {
      font-size: 16px; font-weight: 700; margin: 0 0 12px;
      padding-left: 10px; border-left: 4px solid var(--teal);
    }
    .card {
      background: #fff; border-radius: 12px; border: 1px solid var(--line);
      box-shadow: 0 1px 4px rgba(0,0,0,.04); overflow: hidden;
    }
    table { width: 100%; border-collapse: collapse; font-size: 13px; }
    th {
      background: #f1f5f9; text-align: left; padding: 10px 12px;
      font-weight: 600; color: var(--muted); font-size: 12px;
      border-bottom: 2px solid var(--line);
    }
    td { padding: 9px 12px; border-bottom: 1px solid var(--line); }
    tr:last-child td { border-bottom: none; }
    tbody tr:hover { background: #f8fafc; }
    .mono { font-family: Consolas, "Courier New", monospace; font-size: 12px; }
    .target { font-weight: 600; color: var(--teal); }
    .ts { color: var(--muted); font-size: 12px; white-space: nowrap; }
    .avail { font-weight: 700; padding: 2px 8px; border-radius: 99px; font-size: 12px; }
    .avail-good { background: #dcfce7; color: #166534; }
    .avail-warn { background: #fef3c7; color: #92400e; }
    .avail-bad { background: #ffe4e6; color: #9f1239; }
    .badge {
      display: inline-block; padding: 2px 8px; border-radius: 4px;
      font-size: 11px; font-weight: 700;
    }
    .badge.ok { background: #dcfce7; color: #166534; }
    .badge.warn { background: #fef3c7; color: #92400e; }
    .badge.bad { background: #ffe4e6; color: #9f1239; }
    .row-bad { background: #fff5f5; }
    .note { font-size: 12px; color: var(--muted); margin: 0 0 10px; }
    .foot {
      text-align: center; font-size: 12px; color: var(--muted);
      padding-top: 16px; border-top: 1px solid var(--line);
    }
    @media print {
      .hero { print-color-adjust: exact; -webkit-print-color-adjust: exact; }
      .kpis { break-inside: avoid; }
    }
    @media (max-width: 720px) {
      .kpis { grid-template-columns: 1fr 1fr; margin: -20px 16px 20px; }
      .wrap { padding: 16px; }
    }
  </style>
</head>
<body>
  <header class="hero">
    <h1>链路质量监控报告</h1>
    <div class="sub">网络测试工具箱 V9 · 生成于 ${E(L)}</div>
  </header>
  <div class="kpis">
    <div class="kpi"><div class="label">监控目标</div><div class="val">${F}</div></div>
    <div class="kpi"><div class="label">整体可用率</div><div class="val ${T>=95?"":T>=80?"warn":"bad"}">${T.toFixed(1)}%</div></div>
    <div class="kpi"><div class="label">平均延迟</div><div class="val">${I.toFixed(1)} ms</div></div>
    <div class="kpi"><div class="label">平均丢包</div><div class="val ${b<5?"":"warn"}">${b.toFixed(1)}%</div></div>
  </div>
  <div class="wrap">
    <section>
      <h2>目标汇总</h2>
      <div class="card">
        <table>
          <thead><tr>
            <th>目标</th><th>采样次数</th><th>可用率</th>
            <th>平均延迟(ms)</th><th>平均丢包</th><th>平均抖动(ms)</th>
          </tr></thead>
          <tbody>${O.join("")}</tbody>
        </table>
      </div>
    </section>
    <section>
      <h2>采样明细</h2>
      ${k}
      <div class="card">
        <table>
          <thead><tr>
            <th>时间</th><th>目标</th><th>状态</th><th>结果</th>
            <th>最近延迟</th><th>平均延迟</th><th>丢包率</th><th>抖动</th>
          </tr></thead>
          <tbody>${U.join("")}</tbody>
        </table>
      </div>
    </section>
    <div class="foot">共 ${s.length} 条采样记录 · 网络测试工具箱</div>
  </div>
</body>
</html>`}const Jt={class:"page link fit"},Rt={class:"toolbar"},Dt={class:"pill ok"},Et={class:"pill warn"},Ht={class:"pill bad"},Bt={class:"acts"},Kt=["disabled"],Gt=["disabled"],qt={key:0,class:"flash ok"},Pt={key:1,class:"flash err"},Yt={key:0,class:"cfg panel"},Qt={class:"cfg-main"},Xt={class:"targets-lbl"},Zt=["disabled"],te={class:"thr-grid"},ee={class:"notify-compact"},le={class:"chk"},ae={class:"chk"},oe={class:"kpi-row"},se={class:"kpi"},ne={class:"kpi"},ie={class:"kpi"},ue={class:"kpi muted"},re={class:"body"},de={class:"charts"},ce={class:"panel"},ve={key:1,class:"empty"},pe={class:"panel"},me={class:"sec-head"},he={class:"sec"},ge=["value"],be={key:1,class:"empty"},fe={class:"panel table-panel"},_e={class:"sec"},xe={class:"table-wrap"},ke={key:0},ye=["onClick"],we={class:"mono"},$e={key:1,class:"empty"},Se={class:"panel side-panel"},Ce={class:"tabs"},Le={class:"log"},Me={key:0,class:"empty"},ze={class:"log mono"},Ne={key:0,class:"empty"},Fe=`223.5.5.5
119.29.29.29`,Te=$t({__name:"LinkMonitorView",setup(s){zt([jt,At,Vt,Ot,Ft]);const L=v(Fe),y=v(5),F=v(1e3),V=v(80),T=v(10),I=v(30),b=v(!1),O=v(!1),f=v("alert"),c=v(""),k=v(""),U=v({}),o=v({}),p=v({}),h=v([]),j=v([]),_=v([]),$=v(""),r=v(rt()),{colors:J}=Nt();let M=[];const n=w(()=>Object.values(U.value).sort((l,t)=>l.target.localeCompare(t.target))),z=w(()=>n.value.filter(l=>l.level==="ok").length),vt=w(()=>n.value.filter(l=>l.level==="warn").length),pt=w(()=>n.value.filter(l=>l.level==="bad").length),G=w(()=>{const l=n.value.length||1;return Math.round(z.value/l*100)}),Y=w(()=>{const l=n.value.map(t=>t.avg_ms).filter(t=>t!=null);return l.length?l.reduce((t,a)=>t+a,0)/l.length:null}),Q=w(()=>n.value.length?n.value.reduce((l,t)=>l+t.loss_pct,0)/n.value.length:null),X=w(()=>n.value.length?n.value.reduce((l,t)=>l+t.jitter_ms,0)/n.value.length:null),Z=w(()=>{const l=J.value,t=n.value.map(x=>x.target).slice(0,6);if(!t.length)return null;const a=Math.max(...t.map(x=>(o.value[x]||[]).length),1),m=["#0d9488","#0284c7","#d97706","#7c3aed","#ea580c","#059669"];return{backgroundColor:"transparent",tooltip:{trigger:"axis"},legend:{top:0,textStyle:{color:l.muted,fontSize:10}},grid:{left:44,right:12,top:28,bottom:24},xAxis:{type:"category",data:Array.from({length:a},(x,D)=>`${D}`),axisLabel:{color:l.muted,fontSize:9}},yAxis:{type:"value",name:"ms",axisLabel:{color:l.muted,fontSize:9},splitLine:{lineStyle:{color:l.split}}},series:t.map((x,D)=>({name:x,type:"line",smooth:!0,showSymbol:!1,data:o.value[x]||[],lineStyle:{color:m[D%m.length],width:2}}))}}),tt=w(()=>{var m;const l=J.value,t=$.value||((m=n.value[0])==null?void 0:m.target)||"",a=p.value[t]||[];return t?{backgroundColor:"transparent",tooltip:{trigger:"axis"},grid:{left:40,right:12,top:16,bottom:24},xAxis:{type:"category",data:a.map((x,D)=>`${D}`),axisLabel:{color:l.muted,fontSize:9}},yAxis:{type:"value",max:100,axisLabel:{color:l.muted,fontSize:9},splitLine:{lineStyle:{color:l.split}}},series:[{name:"丢包%",type:"line",smooth:!0,showSymbol:!1,data:a,lineStyle:{color:"#d97706",width:2},areaStyle:{color:"rgba(217,119,6,0.12)"}}]}:null});function mt(l){const t={OK:"正常",WARN:"波动",DOWN:"中断"};return{timestamp:H(l.ts_ms),target:l.target,ok:l.ok,status:t[l.status]||l.status,last_latency_ms:l.last_ms!=null?Math.round(l.last_ms*100)/100:null,avg_latency_ms:l.avg_ms!=null?Math.round(l.avg_ms*100)/100:null,loss_pct:Math.round(l.loss_pct*100)/100,jitter_ms:Math.round(l.jitter_ms*100)/100,sent:l.sent,recv:l.recv}}function ht(l){_.value=[..._.value,mt(l)].slice(-1e4)}function R(l){j.value=[`[${Tt()}] ${l}`,...j.value].slice(0,80)}async function gt(l,t){dt(r.value);const a=H(),m=`【链路告警】
目标: ${l}
内容: ${t}
时间: ${a}`;if(r.value.alert_feishu&&r.value.feishu_webhook.trim())try{await S("send_feishu_webhook",{url:r.value.feishu_webhook.trim(),text:m}),R("飞书告警已发送")}catch(x){R(`飞书通知失败: ${x}`)}if(r.value.alert_wecom&&r.value.wecom_webhook.trim())try{await S("send_wecom_webhook",{url:r.value.wecom_webhook.trim(),text:m}),R("企微告警已发送")}catch(x){R(`企微通知失败: ${x}`)}}async function bt(){c.value="";const l=r.value.feishu_webhook.trim();if(!l){c.value="请先填写飞书 Webhook";return}const t=`【链路监控测试】
时间: ${H()}
状态: 通道可用`;try{await S("send_feishu_webhook",{url:l,text:t}),k.value="飞书测试成功"}catch(a){c.value=String(a)}}async function ft(){c.value="";const l=r.value.wecom_webhook.trim();if(!l){c.value="请先填写企微 Webhook";return}const t=`【链路监控测试】
时间: ${H()}
状态: 通道可用`;try{await S("send_wecom_webhook",{url:l,text:t}),k.value="企微测试成功"}catch(a){c.value=String(a)}}async function _t(){M.forEach(l=>l()),M=[],M.push(await q("linkmon:tick",l=>{const t=l.payload;U.value={...U.value,[t.target]:t},$.value||($.value=t.target);const a=[...o.value[t.target]||[],t.last_ms??(t.ok?0:V.value*2)].slice(-60),m=[...p.value[t.target]||[],t.loss_pct].slice(-60);o.value={...o.value,[t.target]:a},p.value={...p.value,[t.target]:m},ht(t)})),M.push(await q("linkmon:alert",l=>{h.value=[l.payload,...h.value].slice(0,50),f.value="alert",gt(l.payload.target,l.payload.message)})),M.push(await q("linkmon:log",l=>{R(l.payload),l.payload==="stopped"&&(b.value=!1)}))}async function xt(){const l=L.value.split(/\r?\n/).map(t=>t.trim()).filter(Boolean);if(!l.length){c.value="请填写监控目标";return}c.value="",k.value="",dt(r.value),U.value={},o.value={},p.value={},h.value=[],_.value=[],b.value=!0;try{await S("start_link_monitor",{targets:l,intervalSec:y.value,timeoutMs:F.value,latTh:V.value,lossTh:T.value,jitTh:I.value}),k.value="监控已启动"}catch(t){c.value=String(t),b.value=!1}}async function et(){await S("stop_link_monitor"),k.value="已停止"}function kt(){j.value=[],h.value=[]}async function yt(){if(c.value="",!_.value.length){c.value="暂无记录";return}const l=ut(),t=await nt({title:"导出 HTML 报告",defaultPath:`link-monitor-${l}.html`,filters:[{name:"HTML",extensions:["html"]}]});if(t)try{await S("write_text_file",{path:t,content:It(_.value)}),k.value="HTML 已导出"}catch(a){c.value=String(a)}}async function wt(){if(c.value="",!_.value.length){c.value="暂无记录";return}const l=ut(),t=await nt({title:"导出 JSON",defaultPath:`link-monitor-${l}.json`,filters:[{name:"JSON",extensions:["json"]}]});if(t)try{await S("write_text_file",{path:t,content:JSON.stringify(_.value,null,2)}),k.value="JSON 已导出"}catch(a){c.value=String(a)}}return St(async()=>{r.value=rt();try{b.value=await S("link_monitor_running")}catch{}await _t()}),Ct(()=>{M.forEach(l=>l()),b.value&&et()}),(l,t)=>(u(),d("div",Jt,[e("header",Rt,[t[14]||(t[14]=e("span",{class:"page-title"},"链路监控",-1)),e("span",{class:A(["live",{on:b.value}])},null,2),e("span",Dt,"OK "+i(z.value),1),e("span",Et,"WARN "+i(vt.value),1),e("span",Ht,"DOWN "+i(pt.value),1),e("div",Bt,[e("button",{type:"button",class:A(["btn-ghost",{on:O.value}]),onClick:t[0]||(t[0]=a=>O.value=!O.value)},"配置",2),b.value?(u(),d("button",{key:1,type:"button",class:"btn-ghost",onClick:et},"停止")):(u(),d("button",{key:0,type:"button",class:"btn-primary",onClick:xt},"启动")),e("button",{type:"button",class:"btn-ghost",onClick:kt},"清空"),e("button",{type:"button",class:"btn-ghost",disabled:!_.value.length,onClick:yt},"HTML",8,Kt),e("button",{type:"button",class:"btn-ghost",disabled:!_.value.length,onClick:wt},"JSON",8,Gt)]),k.value?(u(),d("p",qt,i(k.value),1)):W("",!0),c.value?(u(),d("p",Pt,i(c.value),1)):W("",!0)]),O.value?(u(),d("div",Yt,[e("div",Qt,[e("label",Xt,[t[15]||(t[15]=C("监控目标 ",-1)),g(e("textarea",{"onUpdate:modelValue":t[1]||(t[1]=a=>L.value=a),spellcheck:"false",rows:"3",disabled:b.value,placeholder:`223.5.5.5
119.29.29.29`},null,8,Zt),[[N,L.value]])]),e("div",te,[e("label",null,[t[16]||(t[16]=C("间隔(s)",-1)),g(e("input",{"onUpdate:modelValue":t[2]||(t[2]=a=>y.value=a),type:"number",min:"1"},null,512),[[N,y.value,void 0,{number:!0}]])]),e("label",null,[t[17]||(t[17]=C("超时(ms)",-1)),g(e("input",{"onUpdate:modelValue":t[3]||(t[3]=a=>F.value=a),type:"number",min:"200"},null,512),[[N,F.value,void 0,{number:!0}]])]),e("label",null,[t[18]||(t[18]=C("延迟(ms)",-1)),g(e("input",{"onUpdate:modelValue":t[4]||(t[4]=a=>V.value=a),type:"number",min:"1"},null,512),[[N,V.value,void 0,{number:!0}]])]),e("label",null,[t[19]||(t[19]=C("丢包(%)",-1)),g(e("input",{"onUpdate:modelValue":t[5]||(t[5]=a=>T.value=a),type:"number",min:"0"},null,512),[[N,T.value,void 0,{number:!0}]])]),e("label",null,[t[20]||(t[20]=C("抖动(ms)",-1)),g(e("input",{"onUpdate:modelValue":t[6]||(t[6]=a=>I.value=a),type:"number",min:"1"},null,512),[[N,I.value,void 0,{number:!0}]])])])]),e("div",ee,[e("label",le,[g(e("input",{"onUpdate:modelValue":t[7]||(t[7]=a=>r.value.alert_feishu=a),type:"checkbox"},null,512),[[lt,r.value.alert_feishu]]),t[21]||(t[21]=C("飞书",-1))]),g(e("input",{"onUpdate:modelValue":t[8]||(t[8]=a=>r.value.feishu_webhook=a),class:"wh",placeholder:"飞书 Webhook"},null,512),[[N,r.value.feishu_webhook]]),e("button",{type:"button",class:"btn-ghost xs",onClick:bt},"测"),e("label",ae,[g(e("input",{"onUpdate:modelValue":t[9]||(t[9]=a=>r.value.alert_wecom=a),type:"checkbox"},null,512),[[lt,r.value.alert_wecom]]),t[22]||(t[22]=C("企微",-1))]),g(e("input",{"onUpdate:modelValue":t[10]||(t[10]=a=>r.value.wecom_webhook=a),class:"wh",placeholder:"企微 Webhook"},null,512),[[N,r.value.wecom_webhook]]),e("button",{type:"button",class:"btn-ghost xs",onClick:ft},"测")])])):W("",!0),e("div",oe,[e("div",{class:A(["kpi",G.value>=80?"good":G.value>=50?"warn":"bad"])},[e("b",null,i(n.value.length?G.value:"—"),1),t[23]||(t[23]=e("span",null,"健康分",-1))],2),e("div",se,[e("b",null,i(Y.value!=null?Y.value.toFixed(0):"—"),1),t[24]||(t[24]=e("span",null,"均延迟 ms",-1))]),e("div",ne,[e("b",null,i(Q.value!=null?Q.value.toFixed(0):"—"),1),t[25]||(t[25]=e("span",null,"均丢包 %",-1))]),e("div",ie,[e("b",null,i(X.value!=null?X.value.toFixed(1):"—"),1),t[26]||(t[26]=e("span",null,"均抖动 ms",-1))]),e("div",ue,[e("b",null,i(_.value.length),1),t[27]||(t[27]=e("span",null,"采样记录",-1))])]),e("div",re,[e("div",de,[e("section",ce,[t[28]||(t[28]=e("div",{class:"sec"},"延迟曲线",-1)),Z.value?(u(),at(ot(it),{key:0,class:"ech",option:Z.value,autoresize:""},null,8,["option"])):(u(),d("div",ve,i(b.value?"采样中…":"启动后显示"),1))]),e("section",pe,[e("div",me,[e("span",he,"丢包 · "+i($.value||"—"),1),n.value.length?g((u(),d("select",{key:0,"onUpdate:modelValue":t[11]||(t[11]=a=>$.value=a),class:"sel-sm"},[(u(!0),d(B,null,K(n.value,a=>(u(),d("option",{key:a.target,value:a.target},i(a.target),9,ge))),128))],512)),[[Lt,$.value]]):W("",!0)]),tt.value?(u(),at(ot(it),{key:0,class:"ech",option:tt.value,autoresize:""},null,8,["option"])):(u(),d("div",be,"—"))])]),e("section",fe,[e("div",_e,"实时状态 · "+i(n.value.length),1),e("div",xe,[n.value.length?(u(),d("table",ke,[t[29]||(t[29]=e("thead",null,[e("tr",null,[e("th",null,"目标"),e("th",null,"状态"),e("th",null,"最近"),e("th",null,"平均"),e("th",null,"丢包"),e("th",null,"抖动")])],-1)),e("tbody",null,[(u(!0),d(B,null,K(n.value,a=>(u(),d("tr",{key:a.target,class:A(a.level),onClick:m=>$.value=a.target},[e("td",we,i(a.target),1),e("td",null,i(a.status),1),e("td",null,i(a.last_ms!=null?a.last_ms.toFixed(0):"—"),1),e("td",null,i(a.avg_ms!=null?a.avg_ms.toFixed(0):"—"),1),e("td",null,i(a.loss_pct.toFixed(0))+"%",1),e("td",null,i(a.jitter_ms.toFixed(0)),1)],10,ye))),128))])])):(u(),d("div",$e,"默认监控阿里/腾讯 DNS，点启动"))])]),e("section",Se,[e("div",Ce,[e("button",{type:"button",class:A({on:f.value==="alert"}),onClick:t[12]||(t[12]=a=>f.value="alert")},"告警 "+i(h.value.length||""),3),e("button",{type:"button",class:A({on:f.value==="log"}),onClick:t[13]||(t[13]=a=>f.value="log")},"日志",2)]),g(e("div",Le,[(u(!0),d(B,null,K(h.value,(a,m)=>(u(),d("div",{key:m,class:A(["alog",a.level])},[e("b",null,i(a.target),1),C(" "+i(a.message),1)],2))),128)),h.value.length?W("",!0):(u(),d("div",Me,"暂无告警"))],512),[[st,f.value==="alert"]]),g(e("div",ze,[(u(!0),d(B,null,K(j.value,(a,m)=>(u(),d("div",{key:m},i(a),1))),128)),j.value.length?W("",!0):(u(),d("div",Ne,"—"))],512),[[st,f.value==="log"]])])])]))}}),Ee=Mt(Te,[["__scopeId","data-v-960a2463"]]);export{Ee as default};
