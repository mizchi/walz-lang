(module
  (%use "./model-inline.wlt" (func $main $sample) (func $summarize $aggregate) (func $summarize-nonnegative $nonnegative))
  (export "main" (func $sample))
  (export "summarize" (func $aggregate))
  (export "summarize-nonnegative" (func $nonnegative)))
