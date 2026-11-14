// Maintained parity corpus v3. Recorded benchmark inputs are independent.
export const projects = [
  ['function-alias-and-reexport', [
    ['main.walz','(module (@use "./facade.walz" (func $answer $run)) (func $entry (export "f") (result i32) (call $run)))'],
    ['facade.walz','(@lib (@use "./impl.walz" (func $value $answer)) (@export (func $answer)))'],
    ['impl.walz','(@lib (@export (func $value)) (func $value (result i32) (i32.const 42)))'],
  ],42],
  ['opaque-type-alias-and-reexport', [
    ['main.walz','(module (@use "./facade.walz" (type $Public $Token) (func $create) (func $value)) (func $entry (export "f") (result i32) (call $value (call $create))))'],
    ['facade.walz','(@lib (@use "./impl.walz" (type $Token $Public) (func $make $create) (func $read $value)) (@export (type $Public) (func $create) (func $value)))'],
    ['impl.walz','(@lib (@export (opaque $Token) (func $make) (func $read)) (type $Token (struct (field $value i32))) (func $make (result (ref $Token)) (struct.new $Token 42)) (func $read (param $v (ref $Token)) (result i32) (struct.get $Token $value $v)))'],
  ],42],
  ['alias-keeps-native-function-indices', [
    ['main.walz','(module (@use "./impl.walz" (func $value $alias)) (func $local (result i32) (i32.const 42)) (export "f" (func 0)))'],
    ['impl.walz','(@lib (@export (func $value)) (func $value (result i32) (i32.const 7)))'],
  ],42],
];
export const programs = [];
for(const cls of ['Add','Sub','Mul','Div','Eq','Ord','Logical','Bitwise','Shift'])
  for(const type of cls==='Logical'?['i32']:['i32','i64'])
    programs.push([`constraint-${cls}-${type}`,`(module (func $accept (@type.params $T) (@where (${cls} $T)) (param $x $T) (result i32) (i32.const 42)) (func $entry (export "f") (result i32) (call $accept<${type}> (${type}.const 0))))`,42]);
export const invalid = [
  ['constraint-logical-i64','(module (func $accept (@type.params $T) (@where (Logical $T)) (param $x $T)) (func $entry (call $accept<i64> (i64.const 0))))',207],
  ['constraint-empty-class','(module (func $accept (@type.params $T) (@where (Add)) (param $x $T)) (func $entry (call $accept<i32> 0)))',207],
  ['constraint-reference-class','(module (type $Cell (struct)) (func $accept (@type.params $T) (@where (Eq $T)) (param $x $T)) (func $entry (call $accept<(ref $Cell)> (struct.new $Cell))))',207],
];
export const badProjects = [
  ['aliased-type-stays-opaque',[
    ['main.walz','(module (@use "./impl.walz" (type $Token $Alias)) (func (param $v (ref $Alias)) (result i32) (struct.get $Alias 0 $v)))'],
    ['impl.walz','(@lib (@export (opaque $Token)) (type $Token (struct (field i32))))'],
  ],202],
  ['aliased-function-stays-private',[
    ['main.walz','(module (@use "./facade.walz" (func $hidden)))'],
    ['facade.walz','(@lib (@use "./impl.walz" (func $value $hidden)))'],
    ['impl.walz','(@lib (@export (func $value)) (func $value))'],
  ],201],
  ['aliases-do-not-create-native-indices',[
    ['main.walz','(module (@use "./impl.walz" (func $value $alias)) (func $entry (result i32) (call 1)))'],
    ['impl.walz','(@lib (@export (func $value)) (func $value (result i32) (i32.const 42)))'],
  ],201],
];
programs.push(
  ['named-struct','(module (type $Pair (struct (field $a i64) (field $b i32))) (func $entry (export "f") (result i32) (struct.get $Pair $b (@struct.new $Pair ($b 42) ($a 20)))))',42],
  ['named-struct-source-order',`(module
    (type $Pair (struct (field $a i32) (field $b i32)))
    (type $Counter (struct (field (mut i32))))
    (func $entry (export "f") (result i32) (@effect reads writes allocates)
      (@let $counter (struct.new $Counter 0)
        (@let $pair (@struct.new $Pair
          ($b (block (result i32) (struct.set $Counter 0 $counter 4) (struct.get $Counter 0 $counter)))
          ($a (block (result i32) (struct.set $Counter 0 $counter (+ (* (struct.get $Counter 0 $counter) 10) 2)) (struct.get $Counter 0 $counter))))
          (struct.get $Pair $a $pair)))))`,42],
  ['named-struct-generic','(module (type $Box (@type.params $T) (struct (field $value $T))) (func $entry (export "f") (result i32) (struct.get $Box<i32> $value (@struct.new $Box<i32> ($value 42)))))',42],
);
invalid.push(
  ['named-struct-duplicate','(module (type $S (struct (field $x i32))) (func $entry (result (ref $S)) (@struct.new $S ($x 1) ($x 2))))',224],
  ['named-struct-missing','(module (type $S (struct (field $x i32))) (func $entry (result (ref $S)) (@struct.new $S)))',224],
  ['named-struct-unknown','(module (type $S (struct (field $x i32))) (func $entry (result (ref $S)) (@struct.new $S ($z 1))))',224],
  ['named-struct-anonymous','(module (type $S (struct (field i32))) (func $entry (result (ref $S)) (@struct.new $S ($x 1))))',224],
  ['named-struct-type','(module (type $S (struct (field $x i64))) (func $entry (result (ref $S)) (@struct.new $S ($x (i32.const 1)))))',110],
);
programs.push(
  ['named-curly','(module (type $S (struct (field $x i32))) (func $entry (export "f") (result i32) (struct.get $S $x (@struct.new $S {:x 42}))))',42],
  ['named-type-prefix','(module (type $S (struct (field $x i32))) (func $entry (export "f") (result i32) (struct.get $S $x (@$S::{:x 42}))))',42],
  ['named-generic-prefix','(module (type $Box (@type.params $T) (struct (field $value $T))) (func $entry (export "f") (result i32) (struct.get $Box<i32> $value (@$Box<i32>::{:value 42}))))',42],
);
invalid.push(
  ['named-curly-odd','(module (type $S (struct (field $x i32))) (func $entry (result (ref $S)) (@$S::{:x})))',224],
  ['named-curly-label','(module (type $S (struct (field $x i32))) (func $entry (result (ref $S)) (@$S::{x 1})))',224],
);

invalid.push(['unmatched-brace','(module (func $brace} (result i32) (i32.const 42)))',3]);

// Native numeric references exercise the bootstrap's wider folded-WAT path;
// Rust's typed @use path requires named exports. Keep this separate from the
// identical-input differential gate, while retaining the bootstrap regression.
export const nativeIndexProjects = projects.splice(projects.findIndex(([name])=>name==='alias-keeps-native-function-indices'),1);

invalid.push(['named-quoted-head','(module (type $S (struct (field $x i32))) (func $entry (result (ref $S)) ("@$S::" {:x 42})))',224]);

programs.push(['named-adjacent-curly','(module (type $S (struct (field $x i32))) (func $entry (export "f") (result i32) (struct.get $S $x (@struct.new $S{:x 42}))))',42]);
projects.push(['generic-alias-and-reexport',[
  ['main.walz','(module (@use "./facade.walz" (type $Box2 $Cell) (func $id2 $id)) (func $entry (export "f") (result i32) (struct.get $Cell<i32> $value (@struct.new $Cell<i32> ($value (call $id<i32> 42))))))'],
  ['facade.walz','(@lib (@use "./impl.walz" (type $Box $Box2) (func $id $id2)) (@export (type $Box2) (func $id2)))'],
  ['impl.walz','(@lib (@export (type $Box) (func $id)) (type $Box (@type.params $T) (struct (field $value $T))) (func $id (@type.params $T) (param $x $T) (result $T) $x))'],
],42]);

programs.push(['named-generic-heap-argument','(module (type $S (struct (field $x i32))) (func $make (@type.params $T) (result (ref $T)) (@struct.new $T ($x 42))) (func $entry (export "f") (result i32) (struct.get $S $x (call $make<$S>))))',42]);

// Explicit resource rejection is outside identical-input acceptance parity.
export const boundedInvalid = [['named-field-budget',`(module (type $S (struct ${Array.from({length:513},(_,i)=>`(field $f${i} i32)`).join(' ')})) (func $entry (result (ref $S)) (@struct.new $S ${Array.from({length:513},(_,i)=>`($f${i} 0)`).join(' ')})))`,150]];
