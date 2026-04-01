fn main() -> turbo_vision::core::error::Result<()> {
    bruto_ide::ide::run(Box::new(oxvba_bruto_lang::OxvbaBrutoLanguage))
}
