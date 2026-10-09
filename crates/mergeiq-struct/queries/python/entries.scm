; Python: imports, module and class bodies.
(module) @container
(class_definition body: (block) @container)

(import_statement
  name: (_) @entry.key) @entry @import

(import_from_statement
  module_name: (_) @entry.key) @entry @import

((function_definition name: (identifier) @entry.key) @entry
  (#set! entry.kind "def"))

((class_definition name: (identifier) @entry.key) @entry
  (#set! entry.kind "def"))

((decorated_definition
  definition: [
    (function_definition name: (identifier) @entry.key)
    (class_definition name: (identifier) @entry.key)
  ]) @entry
  (#set! entry.kind "def"))

(expression_statement
  (assignment left: (_) @entry.key)) @entry
