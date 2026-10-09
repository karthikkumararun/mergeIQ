; Go: imports, top-level declarations, grouped declarations and struct fields.
(source_file) @container
(import_spec_list) @container
(var_spec_list) @container
(field_declaration_list) @container
(const_declaration "(") @container
(type_declaration "(") @container

; Ungrouped single-spec imports; grouped imports are one entry that recurses.
(import_declaration
  (import_spec path: (_) @entry.key)) @entry @import

((import_declaration (import_spec_list)) @entry
  (#set! entry.kind "importgroup"))

(import_spec_list
  (import_spec path: (_) @entry.key) @entry @import)

(function_declaration
  name: (identifier) @entry.key) @entry

(method_declaration
  receiver: (parameter_list (parameter_declaration type: (_) @entry.key))
  name: (field_identifier) @entry.key) @entry

(type_declaration . (type_spec name: (type_identifier) @entry.key) .) @entry
((type_declaration "(") @entry
  (#set! entry.kind "typegroup"))
(type_declaration (type_spec name: (type_identifier) @entry.key) @entry)

(const_declaration . (const_spec name: (identifier) @entry.key) .) @entry
((const_declaration "(") @entry
  (#set! entry.kind "constgroup"))
(const_declaration (const_spec name: (identifier) @entry.key) @entry)

(var_declaration . (var_spec name: (identifier) @entry.key) .) @entry
((var_declaration (var_spec_list)) @entry
  (#set! entry.kind "vargroup"))
(var_spec_list (var_spec name: (identifier) @entry.key) @entry)

(field_declaration_list
  (field_declaration name: (field_identifier) @entry.key) @entry)
