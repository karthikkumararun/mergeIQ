; YAML: block mapping pairs keyed by their key; nested block mappings recurse.
; Containers holding anchors, aliases or tags are skipped by the extractor.
(block_mapping) @container

(block_mapping_pair
  key: (_) @entry.key) @entry
