use bruto_ide::language::{BuildResult, Language};
use turbo_vision::views::syntax::{PlainTextHighlighter, SyntaxHighlighter};

pub struct OxvbaBrutoLanguage;

impl Language for OxvbaBrutoLanguage {
    fn name(&self) -> &str {
        "OxVba"
    }

    fn file_extension(&self) -> &str {
        "bas"
    }

    fn sample_program(&self) -> &str {
        SAMPLE_PROGRAM
    }

    fn create_highlighter(&self) -> Box<dyn SyntaxHighlighter> {
        Box::new(PlainTextHighlighter)
    }

    fn build(&self, _source: &str) -> Result<BuildResult, String> {
        Err(
            "OxVba Bruto build integration is not implemented yet; this lands in bd-br1.4"
                .to_string(),
        )
    }
}

const SAMPLE_PROGRAM: &str = "Sub Main()\n    Print \"Hello from OxVba\"\nEnd Sub\n";

#[cfg(test)]
mod tests {
    use super::OxvbaBrutoLanguage;
    use bruto_ide::language::Language;

    #[test]
    fn bruto_language_surface_is_stable() {
        let language = OxvbaBrutoLanguage;
        assert_eq!(language.name(), "OxVba");
        assert_eq!(language.file_extension(), "bas");
        assert!(language.sample_program().contains("Sub Main()"));
    }

    #[test]
    fn build_stub_is_explicit() {
        let language = OxvbaBrutoLanguage;
        let err = match language.build(language.sample_program()) {
            Ok(_) => panic!("scaffold build should remain unimplemented"),
            Err(err) => err,
        };
        assert!(err.contains("bd-br1.4"));
    }
}
