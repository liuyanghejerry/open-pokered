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
console.log('Jev dex player: audit counts, source remediation, snapshot-safe history and navigation PASS');
