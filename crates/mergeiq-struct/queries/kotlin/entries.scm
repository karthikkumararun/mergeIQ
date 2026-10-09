; Kotlin: imports, declarations and class/object bodies.
(source_file) @container
(class_body) @container

(import
  (qualified_identifier) @entry.key) @entry @import

[
  (class_declaration name: (identifier) @entry.key)
  (object_declaration name: (identifier) @entry.key)
] @entry

(function_declaration
  name: (identifier) @entry.key
  (function_value_parameters) @entry.key) @entry

(property_declaration
  (variable_declaration (identifier) @entry.key)) @entry

(companion_object) @entry
