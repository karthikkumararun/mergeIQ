; TypeScript / TSX: JavaScript rules plus interfaces, type literals, enums, namespaces.
(program) @container
(class_body) @container
(object) @container
(interface_body) @container
(object_type) @container
(enum_body) @container
(internal_module body: (statement_block) @container)
(module body: (statement_block) @container)

(object "," @sep)
(interface_body ["," ";"] @sep)
(object_type ["," ";"] @sep)
(enum_body "," @sep)

(import_statement
  source: (string (string_fragment) @entry.key)) @entry @import

((function_declaration name: (identifier) @entry.key) @entry
  (#set! entry.kind "decl"))
((class_declaration name: (type_identifier) @entry.key) @entry
  (#set! entry.kind "decl"))
((abstract_class_declaration name: (type_identifier) @entry.key) @entry
  (#set! entry.kind "decl"))
((lexical_declaration (variable_declarator name: (_) @entry.key)) @entry
  (#set! entry.kind "decl"))
((variable_declaration (variable_declarator name: (_) @entry.key)) @entry
  (#set! entry.kind "decl"))
((interface_declaration name: (type_identifier) @entry.key) @entry
  (#set! entry.kind "interface"))
((type_alias_declaration name: (type_identifier) @entry.key) @entry
  (#set! entry.kind "type"))
((enum_declaration name: (identifier) @entry.key) @entry
  (#set! entry.kind "enum"))
((export_statement
  declaration: [
    (function_declaration name: (identifier) @entry.key)
    (class_declaration name: (type_identifier) @entry.key)
    (abstract_class_declaration name: (type_identifier) @entry.key)
    (lexical_declaration (variable_declarator name: (_) @entry.key))
    (variable_declaration (variable_declarator name: (_) @entry.key))
  ]) @entry
  (#set! entry.kind "decl"))
((export_statement
  declaration: (interface_declaration name: (type_identifier) @entry.key)) @entry
  (#set! entry.kind "interface"))
((export_statement
  declaration: (type_alias_declaration name: (type_identifier) @entry.key)) @entry
  (#set! entry.kind "type"))
((export_statement
  declaration: (enum_declaration name: (identifier) @entry.key)) @entry
  (#set! entry.kind "enum"))

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
(public_field_definition
  name: (_) @entry.key) @entry
(public_field_definition
  "static" @entry.key
  name: (_) @entry.key) @entry

(property_signature
  name: (_) @entry.key) @entry
(method_signature
  name: (_) @entry.key) @entry

(enum_assignment
  name: (_) @entry.key) @entry
(enum_body (property_identifier) @entry @entry.key)

(pair
  key: (_) @entry.key) @entry
((shorthand_property_identifier) @entry @entry.key)
