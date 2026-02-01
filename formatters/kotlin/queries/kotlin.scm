;; Kotlin Topiary Query File
;; Minimal formatting rules for Kotlin code

; Treat strings as leaf nodes - don't modify internal whitespace
(string_literal) @leaf

; Basic operator spacing
[
  "="
  "+"
  "-"
  "*"
  "/"
  "=="
  "!="
  "<"
  ">"
  "&&"
  "||"
] @prepend_space @append_space

; Comma spacing - antispace before, space after
("," @prepend_antispace @append_space)

; Basic block indentation
(block
  "{" @append_indent_start
  "}" @prepend_indent_end
)

; Function declaration
(function_declaration
  "fun" @append_space
)

; Class declaration
(class_declaration
  "class" @append_space
)
