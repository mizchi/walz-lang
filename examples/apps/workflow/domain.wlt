(module
  (namespace $domain
    (public (type $Command) (type $Error))
    (type $Command
      (variant (skip) (invalid) (task text i64) (after text text) (plan i32)))
    (type $Error
      (variant
        (invalid text)
        (duplicate text)
        (unknown text)
        (capacity text)
        (cycle text))))
)
