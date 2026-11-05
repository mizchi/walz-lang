(module
  (namespace $graph
    (public
      (opaque $Graph)
      (opaque $Job)
      (opaque $Link)
      (func $new)
      (func $size)
      (func $at)
      (func $id)
      (func $index)
      (func $duration)
      (func $dependencies)
      (func $edge-job)
      (func $edge-next)
      (func $add)
      (func $after))
    (type $Job
      (struct
        (field $id text)
        (field $key bytes)
        (field $index i32)
        (field $duration i64)
        (field $deps (mut (ref null $Link))))
    )
    (type $Link (struct (field $job (ref $Job)) (field $next (ref null $Link))))
    (type $Jobs (array (mut (ref null $Job))))
    ;; Lookup links are not fields of Job: retaining a public Job reference
    ;; must not retain unrelated jobs just because their hash values collide.
    (type $Entry (struct (field $job (ref $Job)) (field $next (ref null $Entry))))
    (type $Buckets (array (mut (ref null $Entry))))
    (type $Graph
      (struct
        (field $jobs (mut (ref $Jobs)))
        (field $buckets (mut (ref $Buckets)))
        (field $size (mut i32))
        (field $edges (mut i32))))
    (func $new (result (ref $Graph)) (effects allocates)
      (struct.new $Graph
        (array.new $Jobs (ref.null $Job) (i32.const 8))
        (array.new $Buckets (ref.null $Entry) (i32.const 64))
        (i32.const 0)
        (i32.const 0)))

    (func $size (param $g (ref $Graph)) (result i32) (effects reads)
      (struct.get $Graph $size (local.get $g)))

    (func $at (param $g (ref $Graph)) (param $i i32) (result (ref $Job)) (effects reads)
      (ref.as_non_null
        (array.get $Jobs (struct.get $Graph $jobs (local.get $g)) (local.get $i))))

    (func $id (param $j (ref $Job)) (result text) (effects pure)
      (struct.get $Job $id (local.get $j)))

    (func $index (param $j (ref $Job)) (result i32) (effects pure)
      (struct.get $Job $index (local.get $j)))

    (func $duration (param $j (ref $Job)) (result i64) (effects pure)
      (struct.get $Job $duration (local.get $j)))

    (func $dependencies (param $j (ref $Job)) (result (ref null $Link)) (effects reads)
      (struct.get $Job $deps (local.get $j)))

    (func $edge-job (param $l (ref $Link)) (result (ref $Job)) (effects pure)
      (struct.get $Link $job (local.get $l)))

    (func $edge-next (param $l (ref $Link)) (result (ref null $Link)) (effects pure)
      (struct.get $Link $next (local.get $l)))

    (func $hash (param $key bytes) (result i32) (effects pure)
      (loop $bytes
        (bind $i i32 (i32.const 0))
        (bind $h i32 (i32.const 0))
        (result i32)
        (if
          (result i32)
          (i32.eq (local.get $i) (bytes.len (local.get $key)))
          (then (local.get $h))
          (else
            (recur $bytes
              (i32.add (local.get $i) (i32.const 1))
              (i32.add
                (i32.mul (local.get $h) (i32.const 31))
                (bytes.get (local.get $key) (local.get $i))))
          ))))

    (func $bucket (param $buckets (ref $Buckets)) (param $key bytes) (result i32)
      (effects pure)
      (i32.and
        (call $hash (local.get $key))
        (i32.sub (array.len (local.get $buckets)) (i32.const 1))))

    (func $find (param $g (ref $Graph)) (param $key bytes) (result (option (ref $Job)))
      (effects reads)
      (let $buckets (struct.get $Graph $buckets (local.get $g))
        (loop $search
          (bind $entry
            (ref null $Entry)
            (array.get $Buckets
              (local.get $buckets)
              (call $bucket (local.get $buckets) (local.get $key))))
          (result (option (ref $Job)))
          (if
            (result (option (ref $Job)))
            (ref.is_null (local.get $entry))
            (then (option.none))
            (else
              (let $node (ref.as_non_null (local.get $entry))
                (let $job (struct.get $Entry $job (local.get $node))
                  (if
                    (result (option (ref $Job)))
                    (call $lex.equal
                      (struct.get $Job $key (local.get $job))
                      (local.get $key))
                    (then (option.some (local.get $job)))
                    (else (recur $search (struct.get $Entry $next (local.get $node))))
                  ))))
          ))))

    (func $rehash (param $g (ref $Graph)) (effects reads writes allocates)
      (let $old (struct.get $Graph $buckets (local.get $g))
        (if
          (i32.eq (call $size (local.get $g)) (array.len (local.get $old)))
          (then
            (let $next
              (array.new $Buckets (ref.null $Entry) (i32.mul (array.len (local.get $old)) (i32.const 2)))
              (block
                (for $relink
                  (bind $i i32 (i32.const 0))
                  (while (i32.lt_u (local.get $i) (call $size (local.get $g))))
                  (step (i32.add (local.get $i) (i32.const 1)))
                  (let $job (call $at (local.get $g) (local.get $i))
                    (let $bucket
                      (call $bucket (local.get $next) (struct.get $Job $key (local.get $job)))
                      (array.set $Buckets
                        (local.get $next)
                        (local.get $bucket)
                        (struct.new $Entry
                          (local.get $job)
                          (array.get $Buckets (local.get $next) (local.get $bucket))))
                    )))
                (struct.set $Graph $buckets (local.get $g) (local.get $next))))
          )
          (else (nop))))
    )

    (func $grow (param $g (ref $Graph)) (effects reads writes allocates)
      (let $old (struct.get $Graph $jobs (local.get $g))
        (if
          (i32.eq (call $size (local.get $g)) (array.len (local.get $old)))
          (then
            (let $next
              (array.new $Jobs (ref.null $Job) (i32.mul (array.len (local.get $old)) (i32.const 2)))
              (block
                (for $copy
                  (bind $i i32 (i32.const 0))
                  (while (i32.lt_u (local.get $i) (array.len (local.get $old))))
                  (step (i32.add (local.get $i) (i32.const 1)))
                  (array.set $Jobs
                    (local.get $next)
                    (local.get $i)
                    (array.get $Jobs (local.get $old) (local.get $i))))
                (struct.set $Graph $jobs (local.get $g) (local.get $next))))
          )
          (else (nop))))
    )

    (func $add (param $g (ref $Graph)) (param $id text) (param $duration i64)
      (result (result unit (ref $domain.Error))) (effects reads writes allocates)
      (match (option (ref $Job)) (result (result unit (ref $domain.Error)))
        (call $find (local.get $g) (text.bytes (local.get $id)))
        (case some ($existing)
          (result.err (variant.new $domain.Error duplicate (local.get $id))))
        (case none ()
          (if
            (result (result unit (ref $domain.Error)))
            (i32.ge_u (call $size (local.get $g)) (i32.const 4096))
            (then (result.err (variant.new $domain.Error capacity (text.lit "tasks"))))
            (else
              (if
                (result (result unit (ref $domain.Error)))
                (i32.or
                  (i64.gt_u (local.get $duration) (i64.const 1000000))
                  (i32.or
                    (i32.eqz (text.byte-len (local.get $id)))
                    (i32.gt_u (text.byte-len (local.get $id)) (i32.const 128))))
                (then
                  (result.err (variant.new $domain.Error invalid (text.lit "format"))))
                (else
                  (let $i (call $size (local.get $g))
                    (block
                      (result (result unit (ref $domain.Error)))
                      (call $grow (local.get $g))
                      (call $rehash (local.get $g))
                      (let $buckets (struct.get $Graph $buckets (local.get $g))
                        (let $bucket
                          (call $bucket (local.get $buckets) (text.bytes (local.get $id)))
                          (let $job
                            (struct.new $Job (local.get $id) (text.bytes (local.get $id)) (local.get $i) (local.get $duration) (ref.null $Link))
                            (block
                              (array.set $Jobs
                                (struct.get $Graph $jobs (local.get $g))
                                (local.get $i)
                                (local.get $job))
                              (array.set $Buckets
                                (local.get $buckets)
                                (local.get $bucket)
                                (struct.new $Entry
                                  (local.get $job)
                                  (array.get $Buckets
                                    (local.get $buckets)
                                    (local.get $bucket))))
                            ))))
                      (struct.set $Graph $size
                        (local.get $g)
                        (i32.add (local.get $i) (i32.const 1)))
                      (result.ok))))
              ))))
      ))

    (func $contains (param $head (ref null $Link)) (param $job (ref $Job)) (result i32)
      (effects pure)
      (loop $walk
        (bind $link (ref null $Link) (local.get $head))
        (result i32)
        (if
          (result i32)
          (ref.is_null (local.get $link))
          (then (i32.const 0))
          (else
            (let $edge (ref.as_non_null (local.get $link))
              (if
                (result i32)
                (ref.eq (call $edge-job (local.get $edge)) (local.get $job))
                (then (i32.const 1))
                (else (recur $walk (call $edge-next (local.get $edge))))
              ))))
      ))

    (func $after (param $g (ref $Graph)) (param $id text) (param $dependency text)
      (result (result unit (ref $domain.Error))) (effects reads writes allocates)
      (match (option (ref $Job)) (result (result unit (ref $domain.Error)))
        (call $find (local.get $g) (text.bytes (local.get $id)))
        (case none () (result.err (variant.new $domain.Error unknown (local.get $id))))
        (case some ($job)
          (match (option (ref $Job)) (result (result unit (ref $domain.Error)))
            (call $find (local.get $g) (text.bytes (local.get $dependency)))
            (case none ()
              (result.err (variant.new $domain.Error unknown (local.get $dependency))))
            (case some ($prerequisite)
              (let $head (call $dependencies (local.get $job))
                (if
                  (result (result unit (ref $domain.Error)))
                  (call $contains (local.get $head) (local.get $prerequisite))
                  (then (result.ok))
                  (else
                    (if
                      (result (result unit (ref $domain.Error)))
                      (i32.ge_u
                        (struct.get $Graph $edges (local.get $g))
                        (i32.const 65536))
                      (then
                        (result.err
                          (variant.new $domain.Error capacity (text.lit "dependencies"))))
                      (else
                        (block
                          (result (result unit (ref $domain.Error)))
                          (struct.set $Job $deps
                            (local.get $job)
                            (struct.new $Link
                              (local.get $prerequisite)
                              (local.get $head)))
                          (struct.set $Graph $edges
                            (local.get $g)
                            (i32.add
                              (struct.get $Graph $edges (local.get $g))
                              (i32.const 1)))
                          (result.ok))))
                  ))))
          ))))
  ))
