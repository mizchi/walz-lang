(module
  ;; Separate from ABI scratch. Parallel builds substitute a fixed shared import.
  (memory $memory 256 256)
  (%export (memory $memory)))
