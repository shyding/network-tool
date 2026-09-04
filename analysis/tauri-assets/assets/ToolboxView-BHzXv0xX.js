import{a as E,r as $,B as j,c as b,e as d,F as M,i as B,u as F,n as S,f as k,h as H,w as D,y as W,l as A,m as y,v as z,s as R,_ as K}from"./index-DNL4tSY9.js";const a=(t,e=0)=>{const n=Number(t);return Number.isFinite(n)?n:e},r=(t,e,n,s,o,l)=>({key:t,label:e,value:n,max:Math.max(s,1e-9),unit:o,color:l});function V(t){const e=t.trim();if(/^\d+$/.test(e)){const s=Number(e)>>>0;return[s>>>24&255,s>>>16&255,s>>>8&255,s&255]}const n=e.split(".").map(s=>Number(s));return n.length!==4||n.some(s=>!Number.isFinite(s)||s<0||s>255)?null:n}const C=[{id:"cctv",label:"监控计算",tools:[{id:"cctv_storage",title:"码流 · 带宽 · 存储",desc:"预估 NVR 带宽与硬盘容量",fields:[{key:"res",label:"分辨率",type:"select",default:"1080P/200万",options:["720P/100万","1080P/200万","300万","400万","500万","600万","800万/4K"]},{key:"enc",label:"编码方式",type:"select",default:"H.265 (省~50%)",options:["H.264","H.265 (省~50%)","智能编码 (省~60%)"]},{key:"ch",label:"通道数(路)",default:"16"},{key:"hr",label:"每天录像(h)",default:"24"},{key:"days",label:"保存天数",default:"30"}],compute:t=>{const e={"720P/100万":2,"1080P/200万":4,"300万":6,"400万":8,"500万":12,"600万":14,"800万/4K":16},n={"H.264":1,"H.265 (省~50%)":.5,"智能编码 (省~60%)":.4},s=(e[t.res]||4)*(n[t.enc]||1),o=a(t.ch,16),l=a(t.hr,24),i=a(t.days,30),c=s/8*3600/1024,u=c*l*i*o;return[`单路码率：${s.toFixed(1)} Mbps  (含编码折算)`,`总带宽(NVR接入)：${(s*o).toFixed(1)} Mbps`,"--------------------------------",`单路每小时：${c.toFixed(2)} GB`,`单路每天(${l}h)：${(c*l).toFixed(2)} GB`,"--------------------------------",`总存储需求：${u.toFixed(0)} GB ≈ ${(u/1024).toFixed(2)} TB`,"","说明：CBR 估算；实际受场景动静、帧率、","      编码档位影响，建议再留 10~20% 余量。"].join(`
`)},metrics:t=>{const e={"720P/100万":2,"1080P/200万":4,"300万":6,"400万":8,"500万":12,"600万":14,"800万/4K":16},n={"H.264":1,"H.265 (省~50%)":.5,"智能编码 (省~60%)":.4},s=(e[t.res]||4)*(n[t.enc]||1),o=a(t.ch,16),l=s/8*3600/1024*a(t.hr,24)*a(t.days,30)*o/1024;return[r("br","单路码率",s,20,"Mbps","#0d9488"),r("bw","总带宽",s*o,Math.max(200,s*o),"Mbps","#0284c7"),r("tb","总存储",l,Math.max(8,l),"TB","#ea580c"),r("ch","通道",o,Math.max(32,o),"路","#d97706")]}},{id:"lens",title:"镜头焦距 · 视场角",desc:"算焦距/视场，或按目标宽度反推焦距",fields:[{key:"sensor",label:"传感器尺寸",type:"select",default:'1/2.8"',options:['1/3"','1/2.8"','1/2.7"','1/2.5"','1/2"','1/1.8"']},{key:"dist",label:"监控距离(m)",default:"20"},{key:"focal",label:"焦距(mm)",default:"4"},{key:"tw",label:"目标宽度(m)",default:"6"}],compute:t=>{const n={'1/3"':4.8,'1/2.8"':5,'1/2.7"':5.4,'1/2.5"':5.8,'1/2"':6.4,'1/1.8"':7.2}[t.sensor]||5,s=a(t.dist),o=a(t.focal),l=a(t.tw),i=[];if(o>0&&s>0){const c=n*s/o,u=2*Math.atan(n/(2*o))*180/Math.PI;i.push(`焦距 ${o}mm @ ${s}m：`,`  水平视场宽度：约 ${c.toFixed(1)} m`,`  水平视场角：约 ${u.toFixed(1)}°`,"")}return l>0&&s>0&&i.push(`覆盖 ${l}m 宽 @ ${s}m：`,`  建议焦距：约 ${(n*s/l).toFixed(1)} mm`,""),i.push("说明：基于传感器水平尺寸估算；","      焦距越大视野越窄、看得越远。"),i.join(`
`)},metrics:t=>{const n={'1/3"':4.8,'1/2.8"':5,'1/2.7"':5.4,'1/2.5"':5.8,'1/2"':6.4,'1/1.8"':7.2}[t.sensor]||5,s=a(t.dist,20),o=a(t.focal,4),l=o>0?n*s/o:0,i=o>0?2*Math.atan(n/(2*o))*180/Math.PI:0,c=a(t.tw)>0&&s>0?n*s/a(t.tw):0;return[r("cover","视场宽",l,Math.max(20,l),"m","#0d9488"),r("fov","视场角",i,180,"°","#0284c7"),r("focal","焦距",o,Math.max(12,o),"mm","#ea580c"),r("need","建议焦距",c,Math.max(12,c),"mm","#d97706")]}},{id:"dori",title:"DORI 识别距离",desc:"按像素密度估算侦测/观察/辨认/识别距离",fields:[{key:"hpix",label:"水平分辨率(px)",default:"1920"},{key:"sensor",label:"传感器尺寸",type:"select",default:'1/2.8"',options:['1/3"','1/2.8"','1/2.7"','1/2.5"','1/2"','1/1.8"']},{key:"focal",label:"焦距(mm)",default:"4"}],compute:t=>{const e={'1/3"':4.8,'1/2.8"':5,'1/2.7"':5.4,'1/2.5"':5.8,'1/2"':6.4,'1/1.8"':7.2},n=a(t.hpix,1920),s=e[t.sensor]||5,o=a(t.focal,4);if(o<=0||s<=0||n<=0)return"请输入有效的水平像素/焦距。";const l=[["识别 Identify (250px/m)",250],["辨认 Recognize (125px/m)",125],["观察 Observe (63px/m)",63],["侦测 Detect (25px/m)",25]];return[`水平像素 ${n}px，焦距 ${o}mm`,"--------------------------------",...l.map(([i,c])=>`${i}: ≤ ${(n*o/(s*c)).toFixed(1)} m`),"","依据 EN 62676-4 (DORI) 像素密度标准估算，","距离越近像素密度越高、越能看清细节。"].join(`
`)},metrics:t=>{const e={'1/3"':4.8,'1/2.8"':5,'1/2.7"':5.4,'1/2.5"':5.8,'1/2"':6.4,'1/1.8"':7.2},n=a(t.hpix,1920),s=e[t.sensor]||5,o=a(t.focal,4),l=c=>o>0?n*o/(s*c):0,i=l(25);return[r("id","识别",l(250),Math.max(50,i),"m","#e11d48"),r("re","辨认",l(125),Math.max(50,i),"m","#ea580c"),r("ob","观察",l(63),Math.max(50,i),"m","#d97706"),r("de","侦测",i,Math.max(50,i),"m","#0d9488")]}},{id:"disk_days",title:"硬盘可录天数",desc:"已知硬盘容量，反推能录多少天",fields:[{key:"disk",label:"硬盘总容量(TB)",default:"8"},{key:"br",label:"单路码率(Mbps)",default:"4"},{key:"ch",label:"通道数(路)",default:"16"},{key:"hr",label:"每天录像(h)",default:"24"}],compute:t=>{const e=a(t.disk),n=a(t.br,4),s=a(t.ch,16),o=a(t.hr,24);if(n<=0||s<=0||o<=0)return"请输入有效的码率/通道/时长。";const l=e*1024,i=n/8*3600/1024*o,c=i*s,u=c?l/c:0;return[`硬盘总容量：${e} TB (${l.toFixed(0)} GB)`,`每路每天：${i.toFixed(2)} GB`,`全部通道每天：${c.toFixed(1)} GB`,"------------------------------",`可连续录像：约 ${u.toFixed(1)} 天 (${(u/30).toFixed(1)} 个月)`,"","说明：按满负荷 CBR 估算，实际通常更久。"].join(`
`)},metrics:t=>{const e=a(t.disk),n=a(t.br,4)/8*3600/1024*a(t.hr,24)*a(t.ch,16),s=n>0?e*1024/n:0;return[r("disk","容量",e,Math.max(16,e),"TB","#0284c7"),r("day","日耗",n,Math.max(100,n),"GB","#ea580c"),r("days","可录",s,Math.max(90,s),"天","#0d9488"),r("mo","约月",s/30,Math.max(6,s/30),"月","#d97706")]}}]},{id:"power",label:"供电 / 机房",tools:[{id:"poe",title:"PoE 功耗预算",desc:"校验受电与推荐交换机供电预算",fields:[{key:"std",label:"PoE 标准",type:"select",default:"802.3at (Type2,30W)",options:["802.3af (Type1,15.4W)","802.3at (Type2,30W)","802.3bt (Type3,60W)","802.3bt (Type4,90W)"]},{key:"n",label:"受电设备数量",default:"8"},{key:"p",label:"单台功耗(W)",default:"12"}],compute:t=>{const e={"802.3af (Type1,15.4W)":[15.4,12.95],"802.3at (Type2,30W)":[30,25.5],"802.3bt (Type3,60W)":[60,51],"802.3bt (Type4,90W)":[90,71.3]},[n,s]=e[t.std]||[30,25.5],o=a(t.n,8),l=a(t.p,12),i=o*l,c=i/.85*1.1;return[`PoE 标准：${t.std}`,`  单端口供电上限(PSE)：${n} W`,`  受电设备可用上限(PD)：${s} W`,"--------------------------------",`设备：${o} 台 × ${l}W = ${i.toFixed(1)} W`,l>s?`单台 ${l}W 超过该标准 PD 上限 ${s}W，请选更高标准`:"单台功耗在该标准允许范围内","",`建议交换机 PoE 总预算 ≥ ${c.toFixed(0)} W`,"  （已计入约 15% 线损 + 10% 余量）","","提示：整机 PoE 预算通常 < 端口数×单端口上限，","      选型以交换机「PoE 总功率」为准。"].join(`
`)},metrics:t=>{const e={"802.3af (Type1,15.4W)":[15.4,12.95],"802.3at (Type2,30W)":[30,25.5],"802.3bt (Type3,60W)":[60,51],"802.3bt (Type4,90W)":[90,71.3]},[,n]=e[t.std]||[30,25.5],s=a(t.n,8),o=a(t.p,12),l=s*o,i=l/.85*1.1;return[r("total","总功耗",l,Math.max(i,l),"W","#ea580c"),r("rec","建议预算",i,Math.max(300,i),"W","#0d9488"),r("p","单台",o,n,"W",o>n?"#e11d48":"#0284c7"),r("n","台数",s,Math.max(24,s),"台","#d97706")]}},{id:"ups",title:"UPS 续航估算",desc:"按负载与电池估算后备时间",fields:[{key:"load",label:"负载功率(W)",default:"500"},{key:"volt",label:"电池电压(V)",type:"select",default:"12",options:["12","24","48","240"]},{key:"ah",label:"单组容量(Ah)",default:"9"},{key:"n",label:"电池组数",default:"2"},{key:"eff",label:"逆变效率(%)",default:"90"}],compute:t=>{const e=a(t.load,500);if(e<=0)return"请输入负载功率。";const n=a(t.volt,12)*a(t.ah,9)*a(t.n,2)*.8*(a(t.eff,90)/100),s=n/e;return[`负载功率：${e} W (≈ ${(e/.8).toFixed(0)} VA)`,`电池组：${t.volt}V × ${t.ah}Ah × ${t.n} 组`,"------------------------------",`可用电量：${n.toFixed(0)} Wh (放电深度80%, 效率${a(t.eff,90)}%)`,`预计续航：约 ${(s*60).toFixed(0)} 分钟 (${s.toFixed(2)} 小时)`,"","说明：估算值；实际受电池老化、温度、","      放电倍率(Peukert)影响，选型留余量。"].join(`
`)},metrics:t=>{const e=a(t.load,500),n=a(t.volt,12)*a(t.ah,9)*a(t.n,2)*.8*(a(t.eff,90)/100),s=e>0?n/e*60:0;return[r("wh","电量",n,Math.max(2e3,n),"Wh","#0284c7"),r("min","续航",s,Math.max(120,s),"分","#0d9488"),r("load","负载",e,Math.max(1e3,e),"W","#ea580c"),r("va","约 VA",e/.8,Math.max(1250,e/.8),"VA","#d97706")]}},{id:"rack",title:"机柜功耗 · 散热",desc:"估算发热量、制冷需求与 U 位占用",fields:[{key:"w",label:"设备总功率(W)",default:"2000"},{key:"totalu",label:"机柜总U",default:"42"},{key:"usedu",label:"已占用U",default:"20"}],compute:t=>{const e=a(t.w,2e3),n=a(t.totalu,42),s=a(t.usedu,20),o=e*3.412,l=e*1.2,i=n?s/n*100:0;return[`设备总功率：${e} W (${(e/1e3).toFixed(2)} kW)`,"------------------------------",`发热量：${o.toFixed(0)} BTU/h`,`建议制冷量：≥ ${l.toFixed(0)} W (${(l/1e3).toFixed(2)} kW)`,"------------------------------",`机柜 U 位：${s}/${n} U，占用 ${i.toFixed(0)}%`,`剩余：${Math.max(n-s,0)} U`,"","说明：1W≈3.412BTU/h；制冷含 20% 余量。"].join(`
`)},metrics:t=>{const e=a(t.w,2e3),n=a(t.totalu,42),s=a(t.usedu,20);return[r("w","功率",e,Math.max(5e3,e),"W","#ea580c"),r("cool","制冷",e*1.2,Math.max(6e3,e*1.2),"W","#0284c7"),r("u","占用U",s,Math.max(n,s),"U","#0d9488"),r("util","占用率",n?s/n*100:0,100,"%","#d97706")]}},{id:"dcdrop",title:"直流供电压降",desc:"12/24/48V 摄像头等直流供电线损估算",fields:[{key:"volt",label:"电压(V)",type:"select",default:"12",options:["12","24","48"]},{key:"cur",label:"负载电流(A)",default:"2"},{key:"area",label:"线径(mm²)",type:"select",default:"0.75",options:["0.5","0.75","1.0","1.5","2.5","4.0"]},{key:"dist",label:"单程距离(m)",default:"30"}],compute:t=>{const e=a(t.volt,12),n=a(t.cur,2),s=a(t.area,.75),o=a(t.dist,30);if(s<=0||e<=0)return"请输入有效的线径/电压。";const l=.0175*2*o/s,i=n*l,c=e-i,u=i/e*100,h=u>10?"压降过大(>10%)，加粗线径/缩短距离/就近供电":u>5?"压降偏大(>5%)，建议优化":"压降在合理范围";return[`供电电压：${e} V，负载电流：${n} A`,`线径：${s} mm²，单程：${o} m`,"------------------------------",`回路电阻：${l.toFixed(3)} Ω`,`线路压降：${i.toFixed(2)} V (${u.toFixed(1)}%)`,`末端电压：${c.toFixed(2)} V`,h].join(`
`)},metrics:t=>{const e=a(t.volt,12),n=a(t.cur,2),s=a(t.area,.75),o=a(t.dist,30),l=s>0?.0175*2*o/s:0,i=n*l,c=e>0?i/e*100:0;return[r("drop","压降",i,Math.max(e*.2,i),"V",c>5?"#e11d48":"#0d9488"),r("pct","压降%",c,20,"%",c>5?"#ea580c":"#0284c7"),r("end","末端",e-i,e,"V","#d97706"),r("r","电阻",l,Math.max(1,l),"Ω","#64748b")]}}]},{id:"fiber",label:"光纤 / 线缆",tools:[{id:"optical",title:"光功率预算",desc:"链路损耗与富余度估算",fields:[{key:"tx",label:"发射功率 Tx(dBm)",default:"-3"},{key:"rx",label:"接收灵敏度 Rx(dBm)",default:"-23"},{key:"dist",label:"距离(km)",default:"10"},{key:"conn",label:"连接器数量",default:"2"},{key:"splice",label:"熔接点数量",default:"2"},{key:"fiber",label:"光纤类型",type:"select",default:"单模 1310nm (0.35dB/km)",options:["单模 1310nm (0.35dB/km)","单模 1550nm (0.22dB/km)","多模 850nm (3.5dB/km)","多模 1300nm (1.5dB/km)"]}],compute:t=>{const e={"单模 1310nm (0.35dB/km)":.35,"单模 1550nm (0.22dB/km)":.22,"多模 850nm (3.5dB/km)":3.5,"多模 1300nm (1.5dB/km)":1.5},n=a(t.tx,-3),s=a(t.rx,-23),o=a(t.dist,10),l=a(t.conn,2),i=a(t.splice,2),c=e[t.fiber]||.35,u=c*o,h=l*.5,x=i*.1,w=u+h+x,v=n-s,P=v-w,I=P>=3?"富余充足，链路可正常工作":P>=0?"富余偏低(<3dB)，建议留更多余量":"预算不足，缩短距离或换模块";return[`链路总预算(Tx-Rx)：${v.toFixed(1)} dB`,"--------------------------------",`光纤损耗：${u.toFixed(2)} dB  (${c}×${o}km)`,`连接器损耗：${h.toFixed(2)} dB  (${l}×0.5)`,`熔接损耗：${x.toFixed(2)} dB  (${i}×0.1)`,`总损耗：${w.toFixed(2)} dB`,"--------------------------------",`链路富余度：${P.toFixed(2)} dB`,I,"","常见光模块参考：","千兆多模 SX 850nm ~550m | 万兆多模 SR 850nm ~300m","千兆单模 LX 1310nm ~10km | 万兆单模 LR 1310nm ~10km","千兆单模 1310nm ~20/40km | 万兆单模 ER 1550nm ~40km","千兆单模 1550nm ~80km   | 万兆单模 ZR 1550nm ~80km","单纤 BIDI 注意 Tx/Rx 波长配对。"].join(`
`)},metrics:t=>{const e={"单模 1310nm (0.35dB/km)":.35,"单模 1550nm (0.22dB/km)":.22,"多模 850nm (3.5dB/km)":3.5,"多模 1300nm (1.5dB/km)":1.5},n=a(t.tx,-3),s=a(t.rx,-23),o=(e[t.fiber]||.35)*a(t.dist,10)+a(t.conn,2)*.5+a(t.splice,2)*.1,l=n-s,i=l-o;return[r("budget","预算",l,Math.max(30,l),"dB","#0284c7"),r("loss","总损耗",o,Math.max(30,o),"dB","#ea580c"),r("margin","富余",i,Math.max(10,Math.abs(i)),"dB",i>=3?"#0d9488":"#e11d48"),r("dist","距离",a(t.dist,10),Math.max(40,a(t.dist,10)),"km","#d97706")]}},{id:"cable",title:"网线衰减 / 传输能力",desc:"估算插入损耗与千兆/万兆可达性",fields:[{key:"cat",label:"线缆类别",type:"select",default:"六类 Cat6",options:["超五类 Cat5e","六类 Cat6","超六类 Cat6a","七类 Cat7"]},{key:"len",label:"线缆长度(m)",default:"50"}],compute:t=>{const e={"超五类 Cat5e":[24,0],"六类 Cat6":[21.3,55],"超六类 Cat6a":[20.9,100],"七类 Cat7":[20.8,100]},[n,s]=e[t.cat]||[21.3,55],o=a(t.len,50),l=n*o/100,i=[`线缆类别：${t.cat}`,`线缆长度：${o.toFixed(0)} m`,"--------------------------------",`估算插入损耗(@100MHz)：${l.toFixed(1)} dB`,"标准布线最大长度：100m（永久链路建议≤90m）",""];return o<=0?i.push("请输入有效长度。"):o<=100?(i.push("长度在标准范围内","  • 千兆(1000BASE-T)：支持(≤100m)"),s>=o?i.push(`  • 万兆(10GBASE-T)：支持(该类≤${s}m)`):s>0?i.push(`  • 万兆：超出该类10G距离(${s}m)，建议缩短/升级`):i.push("  • 万兆：该类不支持，建议 Cat6a 以上")):i.push("超过 100m 标准限值","  • 建议加交换机/中继，或改光纤"),i.push("","说明：插损为线性近似工程估算，以现场认证为准。"),i.join(`
`)},metrics:t=>{const e={"超五类 Cat5e":[24,0],"六类 Cat6":[21.3,55],"超六类 Cat6a":[20.9,100],"七类 Cat7":[20.8,100]},[n,s]=e[t.cat]||[21.3,55],o=a(t.len,50);return[r("len","长度",o,100,"m",o>100?"#e11d48":"#0d9488"),r("atten","插损",n*o/100,30,"dB","#ea580c"),r("g1","千兆",o<=100?100:0,100,"%","#0284c7"),r("g10","万兆",s>0&&o<=s?100:0,100,"%","#d97706")]}}]},{id:"net",label:"网络换算",tools:[{id:"ipconv",title:"IP 地址转换",desc:"点分十进制 ↔ 整数/十六进制/二进制",fields:[{key:"ip",label:"IP 或整数",default:"192.168.1.10"}],compute:t=>{const e=V(t.ip);if(!e)return"无效输入，请检查格式。";const n=(e[0]<<24|e[1]<<16|e[2]<<8|e[3])>>>0,s=e.join("."),o=e.map(x=>x.toString(2).padStart(8,"0")).join("."),l=e.map(x=>x.toString(16).toUpperCase().padStart(2,"0")).join("."),i=e[0],c=i<128?"A":i<192?"B":i<224?"C":i<240?"D(组播)":"E(保留)",u=e[0]===10||e[0]===172&&e[1]>=16&&e[1]<=31||e[0]===192&&e[1]===168,h=e[0]===127;return[`点分十进制：${s}`,`整数(十进制)：${n}`,`十六进制：0x${n.toString(16).toUpperCase().padStart(8,"0")}  (${l})`,`二进制：${o}`,"----------------------------------",`传统分类：${c} 类`,`类型：${u?"私有地址":"公网地址"}${h?"  回环":""}`].join(`
`)},metrics:t=>{const e=V(t.ip)||[0,0,0,0];return[r("a","A",e[0],255,"","#0d9488"),r("b","B",e[1],255,"","#0284c7"),r("c","C",e[2],255,"","#ea580c"),r("d","D",e[3],255,"","#d97706")]}},{id:"cidr",title:"子网掩码速查",desc:"CIDR /8~/32 速查表",fields:[{key:"prefix",label:"定位前缀",type:"select",default:"/24",options:Array.from({length:25},(t,e)=>`/${e+8}`)}],compute:t=>{const e=Number((t.prefix||"/24").replace("/","")),n=["CIDR  子网掩码            可用主机数    总地址数","-".repeat(52)];for(let s=8;s<=32;s++){const o=s===0?0:4294967295<<32-s>>>0,l=[24,16,8,0].map(h=>o>>>h&255).join("."),i=2**(32-s);let c=0;s<=30?c=Math.max(i-2,0):s===31?c=2:c=1;const u=s===e?" ◀":"";n.push(`/${String(s).padEnd(3)}  ${l.padEnd(18)} ${String(c).padEnd(12)} ${i}${u}`)}return n.push("","记忆：/24=256(254可用)  /25=128  /26=64  /27=32","      /28=16  /29=8  /30=4(2可用, 点对点)"),n.join(`
`)},metrics:t=>{const e=Number((t.prefix||"/24").replace("/",""))||24,n=2**(32-e),s=e<=30?Math.max(n-2,0):e===31?2:1;return[r("p","前缀",e,32,"","#0d9488"),r("host","主机位",32-e,24,"","#0284c7"),r("usable","可用",s,Math.max(s,1),"","#ea580c"),r("total","总数",n,Math.max(n,1),"","#d97706")]}},{id:"bwtime",title:"带宽 · 下载时间",desc:"按文件大小与带宽估算传输耗时",fields:[{key:"size",label:"文件大小",default:"10"},{key:"unit",label:"单位",type:"select",default:"GB",options:["MB","GB","TB"]},{key:"bw",label:"带宽(Mbps)",default:"100"},{key:"util",label:"线路利用率(%)",default:"90"}],compute:t=>{const e={MB:1e6,GB:1e9,TB:1e12},n=a(t.size),s=a(t.bw),o=a(t.util,90)/100,l=n*(e[t.unit]||1e9)*8,i=s*1e6*o;if(i<=0||n<=0)return"请输入有效的文件大小与带宽。";const c=l/i,u=Math.floor(c/3600),h=Math.floor(c%3600/60),x=c%60,w=[];return u&&w.push(`${u} 小时`),h&&w.push(`${h} 分`),w.push(`${x.toFixed(1)} 秒`),[`文件大小：${n} ${t.unit}`,`带宽：${s} Mbps × 利用率 ${(o*100).toFixed(0)}%`,"------------------------------",`理论下载时间：${w.join(" ")}`,`（合计约 ${c.toFixed(1)} 秒）`,"","说明：Mbps 为比特/秒，1字节=8比特；","      实际受协议开销、并发、拥塞影响。"].join(`
`)},metrics:t=>{const e={MB:1e6,GB:1e9,TB:1e12},n=a(t.size),s=a(t.util,90),o=n*(e[t.unit]||1e9)*8,l=a(t.bw)*1e6*(s/100),i=l>0?o/l:0;return[r("bw","带宽",a(t.bw),Math.max(100,a(t.bw)),"Mbps","#0d9488"),r("min","耗时",i/60,Math.max(10,i/60),"分","#ea580c"),r("size","大小",n,Math.max(20,n),t.unit,"#0284c7"),r("util","利用率",s,100,"%","#d97706")]}},{id:"dataunit",title:"数据单位换算",desc:"bit / Byte / KB / MB / GB / TB 互换",fields:[{key:"val",label:"数值",default:"100"},{key:"from",label:"从",type:"select",default:"MB",options:["bit","Byte","KB","MB","GB","TB","Kbit","Mbit","Gbit"]},{key:"to",label:"到",type:"select",default:"Mbit",options:["bit","Byte","KB","MB","GB","TB","Kbit","Mbit","Gbit"]}],compute:t=>{const e={bit:1,Byte:8,KB:8e3,MB:8e6,GB:8e9,TB:8e12,Kbit:1e3,Mbit:1e6,Gbit:1e9},n=a(t.val),s=n*(e[t.from]||8)/(e[t.to]||8);return[`${n} ${t.from}  =`,`  ${Number(s.toPrecision(6))} ${t.to}`,"--------------------------","换算基准：1 Byte = 8 bit；","K/M/G/T 按 1000 进制(网络常用)。"].join(`
`)},metrics:t=>{const e={bit:1,Byte:8,KB:8e3,MB:8e6,GB:8e9,TB:8e12,Kbit:1e3,Mbit:1e6,Gbit:1e9},n=a(t.val)*(e[t.from]||8);return[r("mb","MB",n/8e6,Math.max(1,n/8e6),"","#0d9488"),r("mbit","Mbit",n/1e6,Math.max(1,n/1e6),"","#0284c7"),r("gb","GB",n/8e9,Math.max(1,n/8e9),"","#ea580c"),r("gbit","Gbit",n/1e9,Math.max(1,n/1e9),"","#d97706")]}},{id:"base",title:"进制转换",desc:"十/十六/二/八 进制互转",fields:[{key:"val",label:"数值",default:"255"},{key:"from",label:"输入进制",type:"select",default:"十进制",options:["十进制","十六进制","二进制","八进制"]}],compute:t=>{const e={十进制:10,十六进制:16,二进制:2,八进制:8},n=t.val.trim(),s=e[t.from]||10;if(!n)return"请输入数值。";try{const o=parseInt(n,s);if(!Number.isFinite(o))throw new Error("bad");return[`十进制：${o}`,`十六进制：0x${o.toString(16).toUpperCase()}`,`二进制：0b${o.toString(2)}`,`八进制：0o${o.toString(8)}`].join(`
`)}catch{return`“${n}” 不是有效的${t.from}数。`}},metrics:t=>{const e={十进制:10,十六进制:16,二进制:2,八进制:8};try{const n=parseInt(t.val.trim(),e[t.from]||10);return Number.isFinite(n)?[r("dec","十进制",n,Math.max(255,n),"","#0d9488"),r("hex","十六进制长",n.toString(16).length,8,"","#0284c7"),r("bin","二进制位",n.toString(2).length,32,"","#ea580c"),r("oct","八进制长",n.toString(8).length,11,"","#d97706")]:[]}catch{return[]}}},{id:"raid",title:"RAID 容量",desc:"估算不同 RAID 级别可用容量与容错",fields:[{key:"level",label:"RAID 级别",type:"select",default:"RAID5",options:["RAID0","RAID1","RAID5","RAID6","RAID10"]},{key:"n",label:"磁盘数量",default:"4"},{key:"size",label:"单盘容量(TB)",default:"4"}],compute:t=>{const e=t.level,n=a(t.n,4),s=a(t.size,4);if(n<=0||s<=0)return"请输入有效的磁盘数量与容量。";let o=0,l="",i="";if(e==="RAID0")o=n*s,l="0 (无冗余)";else if(e==="RAID1")o=s,l=`${n-1} 块`,i="镜像，容量=单盘";else if(e==="RAID5"){if(n<3)return"RAID5 至少需要 3 块盘。";o=(n-1)*s,l="1 块"}else if(e==="RAID6"){if(n<4)return"RAID6 至少需要 4 块盘。";o=(n-2)*s,l="2 块"}else if(e==="RAID10"){if(n<4||n%2)return"RAID10 需要偶数且 ≥4 块盘。";o=n/2*s,l="每组各 1 块"}const c=n*s,u=[`级别：${e}`,`磁盘：${n} 块 × ${s} TB = ${c} TB(裸容量)`,"------------------------------",`可用容量：约 ${o} TB`,`利用率：${(o/c*100).toFixed(0)}%`,`容错：可坏 ${l}`];return i&&u.push(`说明：${i}`),u.join(`
`)},metrics:t=>{const e=a(t.n,4),n=a(t.size,4),s=e*n;let o=0;return t.level==="RAID0"?o=s:t.level==="RAID1"?o=n:t.level==="RAID5"&&e>=3?o=(e-1)*n:t.level==="RAID6"&&e>=4?o=(e-2)*n:t.level==="RAID10"&&e>=4&&e%2===0&&(o=e/2*n),[r("raw","裸容量",s,Math.max(s,1),"TB","#0284c7"),r("use","可用",o,Math.max(s,1),"TB","#0d9488"),r("util","利用率",s?o/s*100:0,100,"%","#ea580c"),r("n","盘数",e,Math.max(12,e),"块","#d97706")]}}]}],N={"Windows 网络":`ipconfig /all                 查看全部网卡配置
ipconfig /release             释放 DHCP 地址
ipconfig /renew               重新获取 DHCP 地址
ipconfig /flushdns            清空 DNS 缓存
ipconfig /displaydns          查看 DNS 缓存
ping -t 目标                  持续 ping
ping -n 10 -l 1000 目标       指定次数/包大小
ping -a IP                    反查主机名
tracert 目标                  路由追踪
tracert -d 目标               不解析域名, 更快
pathping 目标                 路由+丢包统计
arp -a                        查看 ARP 表
arp -d *                      清空 ARP 表
route print                   查看路由表
route add 目标网段 mask 掩码 网关   加静态路由
netstat -ano                  连接与端口(含PID)
netstat -rn                   路由表
nslookup 域名                 DNS 查询
nslookup 域名 8.8.8.8         指定DNS查询
getmac                        查看本机MAC
telnet 主机 端口              测试端口连通(需开启)`,"Windows 系统/诊断":`netsh wlan show profiles      已保存的WiFi
netsh wlan show profile name=X key=clear  查看WiFi密码
netsh interface ip show config            查看IP配置
netsh interface ip set address "以太网" static IP 掩码 网关
netsh interface ip set dns "以太网" static 223.5.5.5
netsh int tcp show global      TCP 全局参数
netsh winsock reset            重置Winsock(修网络)
netsh int ip reset             重置TCP/IP栈
Test-NetConnection IP -Port 端口   PS: 测端口
Get-NetAdapter                 PS: 网卡列表
Get-NetIPAddress               PS: IP地址
Resolve-DnsName 域名           PS: DNS查询
sfc /scannow                   系统文件校验修复
shutdown /r /t 0               立即重启
tasklist / taskkill /PID x /F  进程查看/结束`,"Linux 网络":`ip addr / ip a                查看IP地址
ip -br a                      简洁显示IP
ip route / ip r               查看路由
ip route add 网段 via 网关     加静态路由
ip link set eth0 up/down       启停网卡
ping -c 4 目标                测试连通
traceroute 目标               路由追踪
mtr 目标                      实时路由+丢包
ss -tunlp                     监听端口与进程
ss -s                         连接统计
netstat -anp                  连接状态(旧)
dig 域名 / dig +short 域名     DNS 查询
host 域名 / nslookup 域名     DNS 查询
nmcli dev status              网卡状态
nmcli con show                连接配置
ethtool eth0                  网卡速率/双工
ethtool -S eth0               网卡收发/错包统计
curl -I http://url            查看HTTP头
curl -o file http://url       下载文件
nc -vz IP 端口                测端口连通`,"Linux 服务器":`top / htop                    实时资源
free -h                       内存使用
df -h                         磁盘空间
du -sh 目录                   目录大小
iostat -x 1                   磁盘IO
vmstat 1                      系统负载
systemctl status 服务         服务状态
systemctl restart 服务        重启服务
journalctl -u 服务 -f         跟踪服务日志
ps aux | grep 进程           查进程
kill -9 PID                   强杀进程
lsof -i:端口                  查端口占用
tail -f /var/log/messages     跟踪系统日志
firewall-cmd --list-all       防火墙规则(firewalld)
iptables -L -n                防火墙规则(iptables)
crontab -l / crontab -e       定时任务`,"华为交换机(VRP)":`system-view                     进入系统视图
sysname SW1                     修改设备名
quit / return                   退出/回用户视图
display version                 版本信息
display device                  单板/型号
display current-configuration   当前配置
display saved-configuration     已保存配置
display this                    当前视图配置
display interface brief         端口概览
display ip interface brief      三层口概览
vlan batch 10 20 30             批量建VLAN
vlan 10 / description 办公       建VLAN并描述
display vlan                    查看VLAN
interface GigabitEthernet0/0/1  进入端口
port link-type access           接入模式
port default vlan 10            端口划入VLAN
port link-type trunk            干道模式
port trunk pvid vlan 10         干道本征VLAN
port trunk allow-pass vlan all  干道放行VLAN
port link-type hybrid           混合模式
undo shutdown / shutdown        开/关端口
speed 1000 / duplex full        速率/双工
display mac-address             MAC地址表
display arp                     ARP表
display ip routing-table        路由表
ip route-static 0.0.0.0 0 网关  默认路由
interface Vlanif 10             三层VLAN口
  ip address IP 掩码            配三层IP
display stp brief               生成树状态
interface Eth-Trunk 1           创建聚合口
display transceiver interface verbose  光模块/光功率
display cpu-usage / memory-usage  CPU/内存
display interface GE0/0/1       端口收发/错包
reset counters interface GE0/0/1 清端口计数
display logbuffer               日志
telnet server enable / stelnet server enable  远程
save                            保存配置
reset saved-configuration / reboot  清空/重启
display current-configuration | include vlan  过滤查看`,"思科 Cisco(IOS)":`enable                          进入特权模式
configure terminal              进入全局配置
hostname SW1                    改设备名
end / exit                      返回/退出
show version                    版本信息
show running-config             当前配置
show startup-config             启动配置
show ip interface brief         端口IP概览
show interfaces status          端口状态
vlan 10 / name Office           建VLAN
show vlan brief                 查看VLAN
interface Gi0/1                 进入端口
interface range Gi0/1 - 24      批量端口
switchport mode access          接入模式
switchport access vlan 10       划入VLAN
switchport mode trunk           干道模式
switchport trunk encapsulation dot1q  封装(部分型号)
switchport trunk allowed vlan 10,20   放行VLAN
switchport trunk native vlan 10       本征VLAN
no shutdown / shutdown          开/关端口
speed 1000 / duplex full        速率/双工
show mac address-table          MAC地址表
show ip arp                     ARP表
show ip route                   路由表
ip route 0.0.0.0 0.0.0.0 网关   默认路由
interface Vlan 10               三层VLAN口
  ip address IP 掩码            配三层IP
show spanning-tree              生成树
spanning-tree mode rapid-pvst   STP模式
interface port-channel 1        聚合口
channel-group 1 mode active     加入聚合(LACP)
show etherchannel summary       聚合状态
show interfaces Gi0/1 transceiver detail  光模块
show processes cpu / memory     CPU/内存
show logging                    日志
show cdp neighbors              邻居发现(CDP)
copy running-config startup-config  保存(=write memory)
reload                          重启`,"锐捷 Ruijie(RGOS)":`enable                          进入特权模式
configure terminal              进入全局配置
hostname SW1                    改设备名
show version                    版本信息
show running-config             当前配置
show interfaces status          端口状态
show ip interface brief         端口IP概览
vlan 10 / name Office           建VLAN
show vlan                       查看VLAN
interface GigabitEthernet 0/1   进入端口
interface range g0/1-24         批量端口
switchport mode access          接入模式
switchport access vlan 10       划入VLAN
switchport mode trunk           干道模式
switchport trunk allowed vlan 10,20  放行VLAN
switchport trunk native vlan 10      本征VLAN
no shutdown / shutdown          开/关端口
show mac-address-table          MAC地址表
show arp                        ARP表
show ip route                   路由表
ip route 0.0.0.0 0.0.0.0 网关   默认路由
interface vlan 10               三层VLAN口
  ip address IP 掩码            配三层IP
show spanning-tree              生成树
spanning-tree mode rstp         STP模式
interface aggregateport 1       聚合口(AP)
port-group 1                    端口加入聚合
show aggregateport summary      聚合状态
show interfaces transceiver     光模块
show cpu / show memory          CPU/内存
show logging                    日志
copy running-config startup-config / write  保存
reload                          重启
说明: RGOS 与思科高度相似，聚合用 aggregateport。`,"华三 H3C(Comware)":`system-view                     进入系统视图
sysname SW1                     改设备名
quit / return                   退出/回用户视图
display version                 版本信息
display device                  单板/型号
display current-configuration   当前配置
display this                    当前视图配置
display interface brief         端口概览
vlan 10 / name Office           建VLAN
display vlan                    查看VLAN
interface GigabitEthernet1/0/1  进入端口
port link-type access           接入模式
port access vlan 10             端口划入VLAN
port link-type trunk            干道模式
port trunk permit vlan all      干道放行VLAN
port trunk pvid vlan 10         干道本征VLAN
port link-type hybrid           混合模式
undo shutdown / shutdown        开/关端口
display mac-address             MAC地址表
display arp                     ARP表
display ip routing-table        路由表
ip route-static 0.0.0.0 0 网关  默认路由
interface Vlan-interface 10     三层VLAN口
  ip address IP 掩码            配三层IP
display stp brief               生成树状态
stp global enable               全局启STP
interface Bridge-Aggregation 1  聚合口
port link-aggregation group 1   端口加入聚合
display link-aggregation verbose 聚合详情
display transceiver interface   光模块
display transceiver diagnosis interface  光功率
display cpu-usage / display memory  CPU/内存
display logbuffer               日志
save / reboot                   保存/重启
说明: H3C 与华为同源；接入用 port access vlan，
      干道放行用 port trunk permit vlan。`,"路由 / 三层":`华为: ip route-static 网段 掩码 下一跳    静态路由
思科: ip route 网段 掩码 下一跳           静态路由
查看: 华为 display ip routing-table / 思科 show ip route
华为: ospf 1 → area 0 → network ...      启OSPF
思科: router ospf 1 → network ... area 0 启OSPF
邻居: 华为 display ospf peer / 思科 show ip ospf neighbor
LSDB: 华为 display ospf lsdb / 思科 show ip ospf database
BGP:  bgp 65000 → peer/neighbor ...      建邻
查看: 华为 display bgp peer / 思科 show ip bgp summary
DHCP: 华为 dhcp enable + ip pool          服务
DHCP: 思科 ip dhcp pool NAME → network    服务
网关冗余: 华为 vrrp vrid 1 virtual-ip ...  VRRP
网关冗余: 思科 standby 1 ip ...            HSRP
NAT: 华为 nat outbound acl 2000
NAT: 思科 ip nat inside/outside + acl
静态ARP绑定 华为: arp static IP MAC`,"ACL / QoS / 安全":`华为: acl 2000(基本)/3000(高级)            创建ACL
华为: rule permit source 网段 反掩码        ACL规则
华为: traffic-filter inbound acl 3000      接口应用
思科: access-list 100 permit ip ...        扩展ACL
思科: ip access-group 100 in               接口应用
H3C:  acl advanced 3000 → rule permit ...  ACL
端口安全 华为: port-security enable
端口安全 思科: switchport port-security
DHCP Snooping 华为: dhcp snooping enable
DHCP Snooping 思科: ip dhcp snooping
IPSG/DAI: 防私接IP、ARP欺骗
限速 华为: qos car / traffic-policy
限速 思科: policy-map / service-policy
AAA: 华为 aaa / 思科 aaa new-model
SSH: 思科 crypto key generate rsa
SSH: 华为 stelnet server enable + rsa`,"STP / 环路 / 聚合":`查看: 华为 display stp brief / 思科 show spanning-tree
模式: 华为 stp mode rstp|mstp
模式: 思科 spanning-tree mode rapid-pvst
根桥: 华为 stp root primary
根桥: 思科 spanning-tree vlan 1 root primary
边缘口 华为: stp edged-port enable
边缘口 思科: spanning-tree portfast
BPDU保护 华为: stp bpdu-protection
BPDU保护 思科: spanning-tree bpduguard enable
环路检测 华为: loopback-detect enable
环路检测 H3C: loopback-detection enable
聚合 华为: interface Eth-Trunk 1 / mode lacp-static
聚合 思科: interface port-channel 1 / channel-group 1 mode active
聚合 锐捷: interface aggregateport 1 / port-group 1
聚合 H3C:  interface Bridge-Aggregation 1
环路征兆: 广播/非单播暴增、灯狂闪、大面积卡顿
处理: 启STP/环检 → 分段拔线 → 定位误接口`,"OLT-华为(MA5600T/5800)":`enable → config                进入配置
display ont autofind all       自动发现待注册ONT
interface gpon 0/1             进入GPON框/槽
ont add 0 sn-auth SN omci \\
  ont-lineprofile-id X ont-srvprofile-id X   注册ONT
ont confirm 0 ontid X sn-auth SN ...  确认注册
display ont info 0 1 all       PON口下ONT列表
display ont info summary 0/1/0 汇总信息
display ont optical-info 0 1 ontid X  ONT光功率
display ont register-info      注册历史
display ont version 0 1 ontid X  ONT版本
ont port native-vlan ...       ONT端口VLAN
service-port vlan X gpon 0/1/0 ont X gemport X  业务下发
display board 0                单板信息
display port state 0/1         PON口状态
display current-configuration  当前配置
save                           保存
提示: ONT收光 -8~-27dBm；lineprofile/srvprofile 需先建。`,"OLT-中兴(C300/C600)":`show gpon onu uncfg            未注册ONU
show gpon onu state gpon-olt_1/1/1  PON口ONU状态
interface gpon-olt_1/1/1       进入PON口
onu 1 type ZTE-F660 sn ZTEGxxxxxxxx  注册ONU
interface gpon-onu_1/1/1:1     进入ONU
tcont 1 profile PROF           创建TCONT
gemport 1 tcont 1              绑定GEM
service-port 1 vport 1 user-vlan X vlan X  业务
show gpon onu detail-info gpon-onu_1/1/1:1  ONU详情
show pon power onu-rx gpon-onu_1/1/1:1  ONU收光
show pon power attenuation ...  光衰
show running-config            当前配置
write / show version           保存/版本
提示: 命令随版本(C300/C600)略有差异，以实际为准。`,"无线 / AP":`华为AC: display ap all          AP列表
华为AC: display ap-run-info      AP运行信息
华为AC: display station all      在线终端
华为AC: display station assoc-info  终端关联
华为AC: display wlan ap-performance 性能
华为AC: display radio all        射频状态
华为AC: display vap all          VAP/SSID
思科WLC: show ap summary         AP汇总
思科WLC: show client summary     终端汇总
思科WLC: show wlan summary       SSID汇总
常查: 信道 / 功率 / 接入数 / 干扰 / SNR
2.4G 用 1/6/11 不重叠信道；5G 信道多优先用
终端信号 <-70dBm 体验差；漫游重叠覆盖 15~20%`,"抓包 / 诊断":`tcpdump -i eth0 -nn            基本抓包(Linux)
tcpdump -i eth0 host IP        指定主机
tcpdump -i eth0 port 80        指定端口
tcpdump -i eth0 -w a.pcap      存包供Wireshark
Windows: netsh trace start capture=yes  系统抓包
Windows: netsh trace stop      停止并生成etl
Wireshark: ip.addr==IP         按IP过滤
Wireshark: tcp.port==443       按端口
Wireshark: http / dns / arp    按协议
Wireshark: tcp.flags.syn==1    看握手
Wireshark: tcp.analysis.retransmission  重传
Wireshark: tcp.analysis.zero_window     零窗口
端口镜像 华为: observing-port + mirror to
端口镜像 思科: monitor session 1 source/destination`,排障速查:`不通先看三层：网关 → 外网IP → DNS
  1) ping 网关         判断二层/本地
  2) ping 223.5.5.5    判断外网IP层
  3) ping www.baidu.com 判断DNS解析
端口不通：telnet IP 端口 / Test-NetConnection
时通时断：查双工、错包、环路、IP冲突(arp -a)
网速慢：查双工(半双工?)、光功率、错包计数、限速
IP冲突：arp -a 看同IP多MAC；改静态或查DHCP
环路：非单播暴增、灯狂闪 → 启STP/环检、分段拔线
PoE不上电：核对标准(af/at/bt)、总功率、网线质量/长度
光路：收光过低查熔接/法兰/弯折，过高查是否近距无衰减
DNS问题：换 223.5.5.5 / 119.29.29.29 验证`},X={class:"page toolbox"},q={class:"body"},Z={class:"nav"},Q={class:"g-title"},J=["onClick"],Y={class:"group"},tt={key:0,class:"content calc"},et={class:"calc-hd"},st={class:"calc-main"},nt={class:"params"},ot=["onUpdate:modelValue"],at=["value"],it=["onUpdate:modelValue"],lt={key:0,class:"mini-metrics"},rt={class:"mm-row"},ct={class:"bar"},dt={class:"result-box"},pt={key:1,class:"content cmds"},ut={class:"cmd-top"},mt={class:"search-row"},ft={class:"cmd-main"},ht={class:"cmd-cats"},bt=["onClick"],yt={class:"cmd-out"},gt=E({__name:"ToolboxView",setup(t){const e=$(C[0].id),n=$(C[0].tools[0].id),s=$("calc"),o=$({}),l=$(Object.keys(N)[0]),i=$(""),c=$(!1),u=A(()=>C.find(m=>m.id===e.value)||C[0]),h=A(()=>u.value.tools.find(m=>m.id===n.value)||u.value.tools[0]);function x(m){const f={};for(const p of m.fields)f[p.key]=p.default;o.value=f}j(h,m=>{s.value==="calc"&&x(m)},{immediate:!0});const w=A(()=>{try{return h.value.compute(o.value)}catch(m){return String(m)}}),v=A(()=>{try{return h.value.metrics(o.value)}catch{return[]}}),P=A(()=>{const m=i.value.trim().toLowerCase();if(!m)return N[l.value]||"";const f=[];for(const[p,g]of Object.entries(N)){const T=g.split(`
`).filter(U=>U.toLowerCase().includes(m));T.length&&(f.push(`【${p}】`),f.push(...T),f.push(""))}return f.join(`
`)||`未找到包含 “${i.value}” 的命令`});function I(m,f){s.value="calc",e.value=m,n.value=f}function _(){s.value="cmds"}function L(m){return Number.isFinite(m)?Math.abs(m)>=100?String(Math.round(m)):Math.abs(m)>=10?m.toFixed(1):m.toFixed(2):"—"}function G(m){return m.max?Math.min(100,Math.max(0,m.value/m.max*100)):0}async function O(){try{await navigator.clipboard.writeText(P.value),c.value=!0,setTimeout(()=>{c.value=!1},1200)}catch{}}return(m,f)=>(y(),b("div",X,[f[7]||(f[7]=d("header",{class:"header"},[d("div",{class:"ht"},[d("span",{class:"title"},"运维百宝箱"),d("span",{class:"sub"},"监控 · 供电 · 网络换算 · 命令大全　← 左侧选择工具")])],-1)),d("div",q,[d("aside",Z,[(y(!0),b(M,null,B(F(C),p=>(y(),b("div",{key:p.id,class:"group"},[d("div",Q,k(p.label),1),(y(!0),b(M,null,B(p.tools,g=>(y(),b("button",{key:g.id,type:"button",class:S(["nav-item",{on:s.value==="calc"&&n.value===g.id&&e.value===p.id}]),onClick:T=>I(p.id,g.id)},k(g.title),11,J))),128))]))),128)),d("div",Y,[f[1]||(f[1]=d("div",{class:"g-title"},"命令大全",-1)),d("button",{type:"button",class:S(["nav-item",{on:s.value==="cmds"}]),onClick:_}," 常用命令大全 ",2)])]),s.value==="calc"?(y(),b("section",tt,[d("div",et,[d("h2",null,k(h.value.title),1),d("p",null,k(h.value.desc),1)]),d("div",st,[d("div",nt,[f[2]||(f[2]=d("div",{class:"box-title"},"参数",-1)),(y(!0),b(M,null,B(h.value.fields,p=>(y(),b("label",{key:p.key,class:"field"},[d("span",null,k(p.label),1),p.type==="select"?D((y(),b("select",{key:0,"onUpdate:modelValue":g=>o.value[p.key]=g},[(y(!0),b(M,null,B(p.options,g=>(y(),b("option",{key:g,value:g},k(g),9,at))),128))],8,ot)),[[z,o.value[p.key]]]):D((y(),b("input",{key:1,"onUpdate:modelValue":g=>o.value[p.key]=g},null,8,it)),[[W,o.value[p.key]]])]))),128)),v.value.length?(y(),b("div",lt,[(y(!0),b(M,null,B(v.value,p=>(y(),b("div",{key:p.key,class:"mm"},[d("div",rt,[d("span",null,k(p.label),1),d("b",{style:R({color:p.color})},k(L(p.value))+" "+k(p.unit),5)]),d("div",ct,[d("i",{style:R({width:G(p)+"%",background:p.color})},null,4)])]))),128))])):H("",!0)]),d("div",dt,[f[3]||(f[3]=d("div",{class:"box-title"},"计算结果",-1)),d("pre",null,k(w.value),1)])])])):(y(),b("section",pt,[d("div",ut,[d("label",mt,[f[4]||(f[4]=d("span",null,"搜索",-1)),D(d("input",{"onUpdate:modelValue":f[0]||(f[0]=p=>i.value=p),placeholder:"跨分类检索命令 / 说明关键字…"},null,512),[[W,i.value]])]),d("button",{type:"button",class:"btn-ghost",onClick:O},k(c.value?"已复制":"复制"),1)]),d("div",ft,[d("div",ht,[f[5]||(f[5]=d("div",{class:"box-title"},"分类",-1)),(y(!0),b(M,null,B(Object.keys(F(N)),p=>(y(),b("button",{key:p,type:"button",class:S(["cat-item",{on:l.value===p&&!i.value}]),onClick:g=>{l.value=p,i.value=""}},k(p),11,bt))),128))]),d("div",yt,[f[6]||(f[6]=d("div",{class:"box-title"},"命令",-1)),d("pre",null,k(P.value),1)])])]))])]))}}),xt=K(gt,[["__scopeId","data-v-bb695580"]]);export{xt as default};
