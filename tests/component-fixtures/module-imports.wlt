(module
  (%use "./import-library.wlt" (func $answer $library.answer))
  (%use "./import-library.wlt" (func $answer $answer))
  (export "answer" (func $answer)))
