//! Supported languages: extension mapping, grammar loading and embedded entry queries.

use tree_sitter::Language as TsLanguage;

/// A language the structural merger understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    /// Java (`.java`).
    Java,
    /// Kotlin (`.kt`, `.kts`).
    Kotlin,
    /// Python (`.py`, `.pyi`).
    Python,
    /// YAML (`.yml`, `.yaml`).
    Yaml,
    /// JSON and JSON with comments (`.json`, `.jsonc`).
    Json,
    /// JavaScript (`.js`, `.mjs`, `.cjs`, `.jsx`).
    JavaScript,
    /// TypeScript (`.ts`, `.mts`, `.cts`).
    TypeScript,
    /// TypeScript with JSX (`.tsx`).
    Tsx,
    /// Go (`.go`).
    Go,
}

impl Lang {
    /// Every supported language.
    pub const ALL: [Lang; 9] = [
        Lang::Java,
        Lang::Kotlin,
        Lang::Python,
        Lang::Yaml,
        Lang::Json,
        Lang::JavaScript,
        Lang::TypeScript,
        Lang::Tsx,
        Lang::Go,
    ];

    /// Picks a language from a file path's extension (case-insensitive).
    /// Returns `None` for anything unsupported.
    pub fn from_path(path: &str) -> Option<Lang> {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        Some(match ext.as_str() {
            "java" => Lang::Java,
            "kt" | "kts" => Lang::Kotlin,
            "py" | "pyi" => Lang::Python,
            "yml" | "yaml" => Lang::Yaml,
            "json" | "jsonc" => Lang::Json,
            "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "go" => Lang::Go,
            _ => return None,
        })
    }

    /// A short lowercase name, also the directory name under `queries/`.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Java => "java",
            Lang::Kotlin => "kotlin",
            Lang::Python => "python",
            Lang::Yaml => "yaml",
            Lang::Json => "json",
            Lang::JavaScript => "javascript",
            Lang::TypeScript => "typescript",
            Lang::Tsx => "tsx",
            Lang::Go => "go",
        }
    }

    /// The tree-sitter grammar for this language.
    pub fn grammar(self) -> TsLanguage {
        match self {
            Lang::Java => tree_sitter_java::LANGUAGE.into(),
            Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::Yaml => tree_sitter_yaml::LANGUAGE.into(),
            Lang::Json => tree_sitter_json::LANGUAGE.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }

    /// The embedded `entries.scm` query source (see `queries/<lang>/entries.scm`).
    pub fn entries_query_source(self) -> &'static str {
        match self {
            Lang::Java => include_str!("../queries/java/entries.scm"),
            Lang::Kotlin => include_str!("../queries/kotlin/entries.scm"),
            Lang::Python => include_str!("../queries/python/entries.scm"),
            Lang::Yaml => include_str!("../queries/yaml/entries.scm"),
            Lang::Json => include_str!("../queries/json/entries.scm"),
            Lang::JavaScript => include_str!("../queries/javascript/entries.scm"),
            Lang::TypeScript => include_str!("../queries/typescript/entries.scm"),
            Lang::Tsx => include_str!("../queries/typescript/entries.scm"),
            Lang::Go => include_str!("../queries/go/entries.scm"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_per_spec() {
        let cases = [
            ("A.java", Lang::Java),
            ("a.kt", Lang::Kotlin),
            ("build.gradle.kts", Lang::Kotlin),
            ("a.py", Lang::Python),
            ("a.pyi", Lang::Python),
            ("a.yml", Lang::Yaml),
            ("a.yaml", Lang::Yaml),
            ("a.json", Lang::Json),
            ("tsconfig.jsonc", Lang::Json),
            ("a.js", Lang::JavaScript),
            ("a.mjs", Lang::JavaScript),
            ("a.cjs", Lang::JavaScript),
            ("a.jsx", Lang::JavaScript),
            ("a.ts", Lang::TypeScript),
            ("a.mts", Lang::TypeScript),
            ("a.cts", Lang::TypeScript),
            ("a.tsx", Lang::Tsx),
            ("a.go", Lang::Go),
            ("src/deep\\dir/Mixed.JAVA", Lang::Java),
        ];
        for (path, lang) in cases {
            assert_eq!(Lang::from_path(path), Some(lang), "{path}");
        }
    }

    #[test]
    fn unsupported_extension_is_none() {
        assert_eq!(Lang::from_path("README.md"), None);
        assert_eq!(Lang::from_path("Makefile"), None);
        assert_eq!(Lang::from_path("dir.java/file"), None);
    }

    #[test]
    fn every_grammar_loads() {
        for lang in Lang::ALL {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&lang.grammar())
                .unwrap_or_else(|e| panic!("{}: {e}", lang.name()));
        }
    }
}
