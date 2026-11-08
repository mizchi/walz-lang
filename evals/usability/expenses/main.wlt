(module
  (%use "./model.wlt" (func $main $sample) (func $summarize $aggregate))
  (export "main" (func $sample))
  (export "summarize" (func $aggregate)))
