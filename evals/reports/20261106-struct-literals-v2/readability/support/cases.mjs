// Four spellings of the same fixed-field GC structs. Candidate forms are drafts.
export const variants=['positional','named','objectExplicit','objectShort'];
export const axes=['fields','types','order','layout','changes'];
export const declarations=`(%import "wlt:std" (type $text) (type $bytes))
(type $User (struct (field $name $text) (field $age i32) (field $email $text)))
(type $Bucket (struct (field $count (mut i64)) (field $total (mut i64)) (field $max (mut i64))))
(type $State (struct (field $accepted i64) (field $rejected i64) (field $ignored i64)
  (field $info (ref $Bucket)) (field $warn (ref $Bucket)) (field $error (ref $Bucket))))
(type $Pair (struct (field $left i32) (field $right i32)))
(type $Cell (struct (field $value (mut i32))))
(type $Link (struct (field $next (ref null $Link))))
(type $Job (struct (field $id $text) (field $key $bytes) (field $index i32)
  (field $duration i64) (field $deps (mut (ref null $Link)))))
(type $Trace (struct (field $log (mut i32))))`;
const layouts={User:['name','age','email'],Bucket:['count','total','max'],State:['accepted','rejected','ignored','info','warn','error'],Pair:['left','right'],Cell:['value'],Job:['id','key','index','duration','deps']};
const num=(type,value)=>({num:type,value}),text=value=>({text:value}),raw=source=>({raw:source});
const node=(type,fields)=>({type,fields:Object.entries(fields)});
const bucket=(count,total,max)=>node('Bucket',{count:num('i64',count),total:num('i64',total),max:num('i64',max)});
export const cases=[
  {id:'user',question:'name, age, emailの値を答える。',expression:node('User',{name:text('Alice'),age:num('i32',30),email:text('alice@example.com')}),answer:{name:'Alice',age:30,email:'alice@example.com'}},
  {id:'bucket',question:'count, total, maxの値とtotalType（i32/i64）を答える。',expression:bucket(7,91,22),answer:{count:7,total:91,max:22,totalType:'i64'}},
  {id:'nested',question:'accepted, warnTotal（warn.total）, errorMax（error.max）を答える。',expression:node('State',{accepted:num('i64',11),rejected:num('i64',2),ignored:num('i64',3),info:bucket(10,100,17),warn:bucket(4,19,8),error:bucket(1,5,5)}),answer:{accepted:11,warnTotal:19,errorMax:5}},
  {id:'order',question:'trace（呼出し番号の配列）, left, rightを答える。$markは引数の番号を$traceへ記録し、同じ番号を返す。',expression:node('Pair',{right:raw('(call $mark (local.get $trace) (i32.const 2))'),left:raw('(call $mark (local.get $trace) (i32.const 1))')}),answer:{trace:[2,1],left:1,right:2}},
  {id:'cell',question:'valueとmutable（構築後にstruct.setできるか、boolean）を答える。',expression:node('Cell',{value:num('i32',40)}),answer:{value:40,mutable:true}},
  {id:'job',question:'index, durationType（i32/i64）, depsNull（boolean）, keyFrom（id/index/duration）を答える。$id:$text="task-7", $index:i32=3, $duration:i64=40という束縛を利用できる。',expression:node('Job',{id:raw('(local.get $id)'),key:raw('(%text.bytes (local.get $id))'),index:raw('(local.get $index)'),duration:raw('(local.get $duration)'),deps:raw('(ref.null $Link)')}),answer:{index:3,durationType:'i64',depsNull:true,keyFrom:'id'}},
];
export function render(expr,variant,depth=0) {
  if(expr.raw)return expr.raw;
  if(expr.num)return variant==='objectShort'?String(expr.value):`(${expr.num}.const ${expr.value})`;
  if('text' in expr)return variant==='objectShort'?JSON.stringify(expr.text):`(%text.lit ${JSON.stringify(expr.text)})`;
  const indent='  '.repeat(depth+1),close='  '.repeat(depth);
  if(variant==='positional') {
    if(expr.fields.map(([name])=>name).join()!==layouts[expr.type].join()) {
      const bindings=expr.fields.map(([name,value])=>`($${name} ${render(value,variant,depth+1)})`).join('\n'+indent);
      return `(%let (${bindings})\n${indent}(struct.new $${expr.type} ${layouts[expr.type].map(name=>`(local.get $${name})`).join(' ')}))`;
    }
    return `(struct.new $${expr.type}\n${indent}${expr.fields.map(([,value])=>render(value,variant,depth+1)).join('\n'+indent)}\n${close})`;
  }
  if(variant==='named')return `(%struct.new $${expr.type}\n${indent}${expr.fields.map(([name,value])=>`($${name} ${render(value,variant,depth+1)})`).join('\n'+indent)}\n${close})`;
  return `(%${expr.type}::{\n${indent}${expr.fields.map(([name,value])=>`:${name} ${render(value,variant,depth+1)}`).join('\n'+indent)}\n${close}})`;
}
export const rules={
  positional:'(struct.new $T e1 e2 ...) は型定義のフィールド順に初期化する。値は左から右に各1回評価する。',
  named:'(%struct.new $T ($field expression) ...) はフィールド名で初期化する。記述順は任意。値は記述順に各1回評価する。',
  objectExplicit:'(%T::{:field expression ...}) は宣言された$Tをフィールド名で初期化する。記述順は任意。値は記述順に各1回評価する。値の式はWAT/WLTの通常の式。',
  objectShort:'(%T::{:field expression ...}) は宣言された$Tをフィールド名で初期化する。記述順は任意。値は記述順に各1回評価する。フィールドの期待型が$textなら裸の文字列、i32/i64ならその型の範囲内の整数リテラルを書ける。他の式は通常のWAT/WLT式。',
};
export const oracle={answers:Object.fromEntries(cases.map(c=>[c.id,c.answer])),edits:Object.fromEntries(variants.map(variant=>[variant,render(bucket(7,100,22),variant)]))};

export function baseline() {
  const checks=[
    '(i32.and (i32.eq (struct.get $User $age (local.get $value)) (i32.const 30)) (i32.and (i32.eq (%text.byte-len (struct.get $User $name (local.get $value))) (i32.const 5)) (i32.eq (%text.byte-len (struct.get $User $email (local.get $value))) (i32.const 17))))',
    '(i32.and (i64.eq (struct.get $Bucket $count (local.get $value)) (i64.const 7)) (i32.and (i64.eq (struct.get $Bucket $total (local.get $value)) (i64.const 91)) (i64.eq (struct.get $Bucket $max (local.get $value)) (i64.const 22))))',
    '(i32.and (i64.eq (struct.get $State $accepted (local.get $value)) (i64.const 11)) (i32.and (i64.eq (struct.get $Bucket $total (struct.get $State $warn (local.get $value))) (i64.const 19)) (i64.eq (struct.get $Bucket $max (struct.get $State $error (local.get $value))) (i64.const 5))))',
    '(i32.and (i32.eq (struct.get $Trace $log (local.get $trace)) (i32.const 21)) (i32.and (i32.eq (struct.get $Pair $left (local.get $value)) (i32.const 1)) (i32.eq (struct.get $Pair $right (local.get $value)) (i32.const 2))))',
    '(block (result i32) (struct.set $Cell $value (local.get $value) (i32.const 41)) (i32.eq (struct.get $Cell $value (local.get $value)) (i32.const 41)))',
    '(i32.and (i32.eq (struct.get $Job $index (local.get $value)) (i32.const 3)) (i32.and (i64.eq (struct.get $Job $duration (local.get $value)) (i64.const 40)) (ref.is_null (struct.get $Job $deps (local.get $value)))))',
  ];
  const funcs=cases.map((c,i)=>`(func $${c.id} (result i32)\n(%let $value ${render(c.expression,'positional')} ${checks[i]}))`).join('\n');
  return `(module\n${declarations}\n(func $mark (param $trace (ref $Trace)) (param $n i32) (result i32)\n(block (result i32) (struct.set $Trace $log (local.get $trace) (i32.add (i32.mul (struct.get $Trace $log (local.get $trace)) (i32.const 10)) (local.get $n))) (local.get $n)))\n${funcs.replace('(func $order (result i32)','(func $order-body (param $trace (ref $Trace)) (result i32)').replace('(func $job (result i32)','(func $job-body (param $id $text) (param $index i32) (param $duration i64) (result i32)')}\n(func $order (result i32) (call $order-body (struct.new $Trace (i32.const 0))))\n(func $job (result i32) (call $job-body (%text.lit "task-7") (i32.const 3) (i64.const 40)))\n(func $main (export "main") (result i32) (if (result i32) ${cases.reduceRight((rest,c)=>`(i32.and (call $${c.id}) ${rest})`,'(i32.const 1)')} (then (i32.const 42)) (else (i32.const 0)))))\n`;
}
