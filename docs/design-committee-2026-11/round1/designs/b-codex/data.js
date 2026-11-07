// Deterministic design fixture. All timestamps are relative to this sample snapshot.
window.D = (() => {
  const H=3600000, DAY=24*H, NOW=new Date('2026-11-06T15:42:00+03:00').getTime();
  const tools=[
    {id:'claude',name:'Claude Code',installed:true,key:true,routed:true,quota:true,version:'2.1.32'},
    {id:'codex',name:'Codex',installed:true,key:true,routed:false,quota:true,version:'0.167.0'},
    {id:'opencode',name:'OpenCode',installed:true,key:false,routed:false,quota:false,version:'1.2.8'},
    {id:'agy',name:'Antigravity',installed:true,key:false,routed:false,quota:false,version:'1.8.0'},
    {id:'pi',name:'Pi',installed:false,key:true,routed:true,quota:false,version:null},
    {id:'omp',name:'Oh My Pi',installed:false,key:false,routed:false,quota:false,version:null}
  ];
  const win=(label,left,reset,extra={})=>({label,used:100-left,reset,at:extra.out||left===0?100:Math.min(99,100-left+15),...extra});
  const accounts=[
    {id:'codex-main',tool:'codex',main:true,plan:'Pro',email:'samy@example.com',kind:'subscription',windows:[win('5-hour',0,42*60000),win('Weekly',27,3.92*DAY,{out:1.31*DAY})],tokens:14.9e6,cost:61.28,on:['dev','msi'],read:2},
    {id:'claude-main',tool:'claude',main:true,plan:'Max 20x',email:'samykabu@gmail.com',kind:'subscription',windows:[win('5-hour',34,2.32*H),win('Weekly',8,4.29*DAY,{out:6.2*H}),win('Weekly · Opus',22,4.29*DAY)],tokens:21.4e6,cost:184.37,on:['dev'],read:2},
    {id:'sabushanab-resal',tool:'claude',plan:'Team',email:'sabus@resal.me',kind:'subscription',windows:[win('5-hour',14,18*60000),win('Weekly',87,4.29*DAY),win('Weekly · Opus',91,4.29*DAY)],tokens:6.8e6,cost:44.21,on:['dev'],read:2},
    {id:'service-account-resal',tool:'claude',plan:'Max 5x',email:'akusumah@resal.me',kind:'subscription',windows:[win('5-hour',98,4.13*H),win('Weekly',100,4.29*DAY),win('Weekly · Opus',100,4.29*DAY)],tokens:2.75e6,cost:1.61,on:[],read:2},
    {id:'samykabu-teksa',tool:'claude',plan:'Pro',email:'samy@teksa.com',kind:'subscription',windows:[win('5-hour',77,3.1*H),win('Weekly',77,5.2*DAY)],tokens:4.7e6,cost:28.34,on:['mac'],read:2},
    {id:'samykabu-personal',tool:'claude',plan:'Pro',email:'samykabu@gmail.com',kind:'subscription',windows:[win('5-hour',96,4.5*H),win('Weekly',96,6.58*DAY)],tokens:1.2e6,cost:9.88,on:[],read:2},
    {id:'opencode-main',tool:'opencode',main:true,plan:'Provider login',email:'samy@example.com',kind:'subscription',windows:[],tokens:3.9e6,cost:11.20,on:['mac'],read:null},
    {id:'agy-main',tool:'agy',main:true,plan:null,email:'samy@example.com',kind:'subscription',windows:[],tokens:null,cost:null,on:[],read:null,needsLogin:true}
  ];
  const sampleTwelve=[
    {id:'resal-router',tool:'claude',kind:'routed',sample:12,plan:'Routed · openrouter',email:'service@resal.me',route:'openrouter',cap:{cap:25,spent:11.2,reset:8.3*H},windows:[],tokens:2.4e6,cost:11.2,on:[],read:2},
    {id:'codex-key',tool:'codex',kind:'api_key',sample:12,plan:'Pay per use',email:'samy@example.com',windows:[],tokens:1.8e6,cost:8.4,on:[],read:null},
    {id:'codex-work',tool:'codex',kind:'subscription',sample:12,plan:'Team',email:'work@example.com',windows:[win('5-hour',62,1.4*H),win('Weekly',54,2.8*DAY)],tokens:4.1e6,cost:19.2,on:[],read:2},
    {id:'claude-research',tool:'claude',kind:'subscription',sample:12,plan:'Pro',email:'research@example.com',windows:[win('5-hour',81,3.7*H),win('Weekly',69,5.1*DAY)],tokens:3.2e6,cost:21.6,on:[],read:2}
  ];
  accounts.push(...sampleTwelve);
  const accountHues=['#818cf8','#f59e88','#4ac7b0','#e1b85c','#9eb8ea','#c899d9','#70b9cf','#afbf70','#ed90b3','#98b5aa','#c9a477','#9da8c6'];
  accounts.forEach((a,i)=>a.hue=accountHues[i%accountHues.length]);
  for(const a of accounts){const weights=Array.from({length:30},(_,i)=>.6+((i*7+a.id.length)%11)/10),base=weights.slice(-7).reduce((x,y)=>x+y,0);a.daily=weights.map(w=>Math.round((a.tokens||0)*w/base));if(a.tokens!=null)a.daily[29]+=a.tokens-a.daily.slice(-7).reduce((x,y)=>x+y,0)}
  const computers=[{id:'dev',name:'DEVELOPMENTPC',me:true,seen:0},{id:'msi',name:'SAMYMSI',seen:40},{id:'mac',name:'Samys-MacBook-Pro',seen:95}];
  const running=[
    {state:'writing',id:1,project:'HarnessManager',account:'claude-main',computer:'dev',model:'claude-opus-4-6',since:'32m',first:1.1},
    {state:'waiting',id:2,project:'Bunyan',account:'codex-main',computer:'dev',model:'gpt-6.1-sol',since:'18m',first:0.9},
    {state:'writing',id:3,project:'resal-wiki',account:'sabushanab-resal',computer:'dev',model:'claude-sonnet-4-6',since:'8m',first:1.3},
    {state:'waiting',id:4,project:'sanduq-api',account:'codex-main',computer:'msi',model:'gpt-6.1-sol',since:'54m',first:1.2},
    {state:'writing',id:5,project:'PropertyManagement',account:'samykabu-teksa',computer:'mac',model:'claude-sonnet-4-6',since:'1h 12m',first:1.6},
    {state:'writing',id:6,project:'model-adoption',account:'opencode-main',computer:'mac',model:'deepseek-v4',since:'21m',first:0.6}
  ];
  const days=Array.from({length:371},(_,i)=>{const tokens=i%11===0?0:Math.round((0.6+(i%17)/10)*1e7);return {t:NOW-(370-i)*DAY,tokens,by:{dev:Math.round(tokens*.6),msi:Math.round(tokens*.25),mac:Math.round(tokens*.15)}}});
  const replies=Array.from({length:12},(_,i)=>({time:`15:${String(41-i).padStart(2,'0')}`,account:accounts[i%6].id,model:i%2?'gpt-6.1-sol':'claude-opus-4-6',computer:computers[i%3].name,tokens:2380+i*417,first:(0.9+i*0.13).toFixed(1),speed:94+i*7}));
  return {locale:navigator.language,timeZone:Intl.DateTimeFormat().resolvedOptions().timeZone,H,DAY,NOW,tools,accounts,computers,running,days,replies,routes:['openrouter','deepseek-fast'],kpi:{today:{tokens:48.2e6,cost:184.37,cached:91,delta:'+12%',deltaCost:'+$22.10',deltaCached:'+3 pt'},'7d':{tokens:298.4e6,cost:1048.21,cached:93,delta:'+8%',deltaCost:'+$72.58',deltaCached:'+2 pt'},'30d':{tokens:1240e6,cost:4390.42,cached:92,delta:'+16%',deltaCost:'+$597.42',deltaCached:'+1 pt'},all:{tokens:7.62e9,cost:28741.11,cached:89,delta:'+21%',deltaCost:'+$4,988.11',deltaCached:'+4 pt'}}};
})();
