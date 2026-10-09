; Java: imports, types, and class/interface/record members.
(program) @container
(class_body) @container
(interface_body) @container
(annotation_type_body) @container

(import_declaration
  (_) @entry.key) @entry @import

[
  (class_declaration name: (identifier) @entry.key)
  (interface_declaration name: (identifier) @entry.key)
  (enum_declaration name: (identifier) @entry.key)
  (record_declaration name: (identifier) @entry.key)
  (annotation_type_declaration name: (identifier) @entry.key)
] @entry

(field_declaration
  declarator: (variable_declarator name: (identifier) @entry.key)) @entry

(method_declaration
  name: (identifier) @entry.key
  parameters: (formal_parameters
    (formal_parameter type: (_) @entry.key)*)) @entry

(constructor_declaration
  name: (identifier) @entry.key
  parameters: (formal_parameters
    (formal_parameter type: (_) @entry.key)*)) @entry
