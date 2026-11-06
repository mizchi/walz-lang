(module
  (%import "wlt:std" (type $text))
  (type $User (struct (field $name $text) (field $age i32) (field $email $text)))
  (func $main
    (export "main")
    (result i32)
    (%effect allocates)
    (%let
      (($alice (%$User::{:name "Alice" :age 30 :email "alice@example.com"}))
       ($bob
         (%struct.new $User
           ($email (%text.lit "bob@example.com"))
           ($age (i32.const 12))
           ($name (%text.lit "Bob"))))
      )
      (i32.add
        (struct.get $User $age (local.get $alice))
        (struct.get $User $age (local.get $bob))))
  ))
