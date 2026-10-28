// source.go:5
pub trait Source {
    fn text(&self) -> &str;
    fn file_name(&self) -> &str;
    fn ecma_line_map(&self) -> &[u32];
}

// Go: *ast.SourceFile satisfies Source implicitly through its Text/FileName/ECMALineMap methods.
impl Source for crate::tsrs_ast::SourceFile {
    fn text(&self) -> &str {
        crate::tsrs_ast::SourceFile::text(self)
    }
    fn file_name(&self) -> &str {
        crate::tsrs_ast::SourceFile::file_name(self)
    }
    fn ecma_line_map(&self) -> &[u32] {
        crate::tsrs_ast::SourceFile::ecma_line_map(self)
    }
}
