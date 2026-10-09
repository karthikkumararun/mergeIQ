; JavaScript: imports, declarations, class bodies, object literals.
(program) @container
(class_body) @container
(object) @container

(object "," @sep)

(import_statement
  source: (string (string_fragment) @entry.key)) @entry @import

((function_declaration name: (identifier) @entry.key) @entry
  (#set! entry.kind "decl"))
((class_declaration name: (identifier) @entry.key) @entry
  (#set! entry.kind "decl"))
((lexical_declaration (variable_declarator name: (_) @entry.key)) @entry
  (#set! entry.kind "decl"))
((variable_declaration (variable_declarator name: (_) @entry.key)) @entry
  (#set! entry.kind "decl"))
((export_statement
  declaration: [
    (function_declaration name: (identifier) @entry.key)
    (class_declaration name: (identifier) @entry.key)
    (lexical_declaration (variable_declarator name: (_) @entry.key))
    (variable_declaration (variable_declarator name: (_) @entry.key))
  ]) @entry
  (#set! entry.kind "decl"))

(method_definition
  name: (_) @entry.key) @entry
(method_definition
  "static" @entry.key
  name: (_) @entry.key) @entry
(method_definition
  "get" @entry.key
  name: (_) @entry.key) @entry
(method_definition
  "set" @entry.key
  name: (_) @entry.key) @entry
(field_definition
  property: (_) @entry.key) @entry

(pair
  key: (_) @entry.key) @entry
((shorthand_property_identifier) @entry @entry.key)
