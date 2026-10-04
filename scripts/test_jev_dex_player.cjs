// Exercise the actual player script without a generated full-run artifact.
const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
const html=fs.readFileSync(path.join(__dirname,'../docs/jev-retrospective-assets/dex-run/jev-dex-player.html'),'utf8');
const script=html.match(/<script>([\s\S]*?)<\/script>/)[1];
const base={owned:2,seen:2,owned_species:['Cubone','Marowak'],method_counts:{grass:1,unknown:1}};
const pending={...base,validated_owned:1,pending_source_validation:['Marowak']};
const verified={...base,validated_owned:2,pending_source_validation:[]};
const data={target:{solo_ceiling:124},species:[{number:104,name:'Cubone',status:'owned'},
 {number:105,name:'Marowak',status:'owned'}],progress:[
 {...pending,source_s:0,acquired:['Cubone'],method:'grass',map:'PokemonTower7F'},
 {...pending,source_s:.5,acquired:[]},
 {...verified,source_s:1,acquired:[],source_revalidated:['Marowak'],method:'evolution'},
 {...verified,source_s:1.5,acquired:[]},
 {...verified,source_s:2,acquired:['Other'],method:'unknown'}],
 decisions:[{source_s:0,dex_progress:{owned:2,validated_owned:1},candidates:[]}]};
const elements=new Map();
const el=id=>{if(!elements.has(id))elements.set(id,{style:{setProperty(){}},textContent:'',innerHTML:''});return elements.get(id)};
const video=el('#video');Object.assign(video,{currentTime:.5,duration:10,pause(){},addEventListener(){}});
const context=vm.createContext({window:{JEV_DEX_DASHBOARD:data},document:{querySelector:el}});
vm.runInContext(script,context);
assert.equal(el('#score').textContent,'1/124');
assert.equal(el('#acquired').textContent,'Cubone','resumed snapshot must not erase last acquisition');
assert.match(el('#audit').textContent,/原生登记 2 种.*Marowak/);
assert.match(el('#methods').innerHTML,/<b>1<\/b>来源未记录/);
assert.match(el('#species').innerHTML,/class="mon pending"/);
assert.match(el('#decision-title').textContent,/1\/124/);
el('#next').onclick();assert.equal(video.currentTime,1);
assert.equal(el('#score').textContent,'2/124');
assert.equal(el('#acquired').textContent,'Marowak');
assert.match(el('#acquired-meta').textContent,/合法来源补证/);
assert.doesNotMatch(el('#species').innerHTML,/class="mon pending"/);
video.currentTime=1.5;el('#previous').onclick();assert.equal(video.currentTime,1);
el('#previous').onclick();assert.equal(video.currentTime,0);
video.currentTime=2;el('#next').onclick();assert.equal(video.currentTime,2,'end must not loop backward');
vm.runInContext('paint()',context);assert.match(el('#acquired-meta').textContent,/来源未记录/);
// Prospective native witnesses use wild_capture: terrain labels in old traces
// remain separate, and absent producer evidence must still remain unknown.
const wildElements=new Map();
const wildEl=id=>{if(!wildElements.has(id))wildElements.set(id,{style:{setProperty(){}},textContent:'',innerHTML:''});return wildElements.get(id)};
const wildVideo=wildEl('#video');Object.assign(wildVideo,{currentTime:0,duration:10,pause(){},addEventListener(){}});
const wildData={target:{solo_ceiling:124},species:[{number:92,name:'Gastly',status:'owned'},{number:16,name:'Pidgey',status:'owned'}],decisions:[],progress:[
 {source_s:0,owned:1,seen:1,owned_species:['Gastly'],acquired:['Gastly'],method:'wild_capture',map:'PokemonTower7F',method_counts:{wild_capture:1}},
 {source_s:1,owned:2,seen:2,owned_species:['Gastly','Pidgey'],acquired:['Pidgey'],method:'unknown',map:'Route1',method_counts:{wild_capture:1,unknown:1}}]};
const wildContext=vm.createContext({window:{JEV_DEX_DASHBOARD:wildData},document:{querySelector:wildEl}});
vm.runInContext(script,wildContext);
assert.match(wildEl('#acquired-meta').textContent,/野生捕获/);
assert.match(wildEl('#methods').innerHTML,/<b>1<\/b>野生捕获/);
for(const label of ['草丛','水面','钓鱼','狩猎','赠送','静态','NPC交换','进化','兑换','来源未记录']){
 assert(wildEl('#methods').innerHTML.includes(label),`legacy source bucket ${label} must remain available`);
}
wildVideo.currentTime=1;vm.runInContext('paint()',wildContext);
assert.match(wildEl('#acquired-meta').textContent,/来源未记录/);
assert.match(wildEl('#methods').innerHTML,/<b>1<\/b>野生捕获/);
assert.match(wildEl('#methods').innerHTML,/<b>1<\/b>来源未记录/);
const sourceTotal=[...wildEl('#methods').innerHTML.matchAll(/<b>(\d+)<\/b>/g)].reduce((sum,match)=>sum+Number(match[1]),0);
assert.equal(sourceTotal,2,'all displayed producer counts must include native wild_capture');
// Schema 4 uses deduplicated complete public descriptions; legacy rows still work.
const decodedData={...wildData,candidate_descriptions:{facts:{establish:['catch','ViridianForest',true],note:'<script>unsafe</script>'}},decisions:[
 {source_s:0,choice:'a',candidate_count:6,dex_progress:{owned:71},candidates:[
  {id:'a',label:'["catch","ViridianForest",true]',probability:.7,description_ref:'facts'},
  {id:'none',label:'Wait',probability:null}]},
 {source_s:2,choice:'legacy',dex_progress:{owned:72},candidates:[{id:'legacy',label:'Legacy plain criterion',probability:.2}]}]};
const decodedElements=new Map();
const decodedEl=id=>{if(!decodedElements.has(id))decodedElements.set(id,{style:{setProperty(){}},textContent:'',innerHTML:''});return decodedElements.get(id)};
const decodedVideo=decodedEl('#video');Object.assign(decodedVideo,{currentTime:0,duration:10,pause(){},addEventListener(){}});
const decodedContext=vm.createContext({window:{JEV_DEX_DASHBOARD:decodedData},document:{querySelector:decodedEl}});
vm.runInContext(script,decodedContext);
assert.match(decodedEl('#decision-title').textContent,/71\/124/);
assert.match(decodedEl('#decision-scope').textContent,/本次请求.*非捕获率.*6/);
assert.match(decodedEl('#candidates').innerHTML,/✓.*ViridianForest/);
assert.match(decodedEl('#candidates').innerHTML,/70%/);
assert.match(decodedEl('#candidates').innerHTML,/未记录/);
assert.match(decodedEl('#candidates').innerHTML,/公开候选事实/);
assert.match(decodedEl('#candidates').innerHTML,/&lt;script&gt;unsafe&lt;\/script&gt;/);
assert.doesNotMatch(decodedEl('#candidates').innerHTML,/<script>/);
decodedEl('#candidates').innerHTML='OPEN_DETAILS_SENTINEL';
vm.runInContext('paint()',decodedContext);
assert.equal(decodedEl('#candidates').innerHTML,'OPEN_DETAILS_SENTINEL','clock repaint must not close expanded facts');
decodedVideo.currentTime=2;vm.runInContext('paint()',decodedContext);
assert.match(decodedEl('#candidates').innerHTML,/✓ Legacy plain criterion/);
assert.match(decodedEl('#candidates').innerHTML,/20%/);
decodedVideo.currentTime=0;vm.runInContext('paint()',decodedContext);
assert.match(decodedEl('#candidates').innerHTML,/ViridianForest/);
console.log('Jev dex player: source audit, wild/legacy buckets, decoded facts, escaping, request scope, stable details and seeking PASS');
