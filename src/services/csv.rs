//! Escrita de CSV (RFC 4180): campos entre aspas quando precisam, aspas dobradas e linhas
//! terminadas em CRLF.

/// Campo de texto. Vai entre aspas se tiver vírgula, aspa ou quebra de linha. Um texto que
/// começa com `=`, `+`, `-`, `@`, tabulação ou retorno de carro ganha um `'` na frente, para uma
/// planilha nunca tratá-lo como fórmula (CSV injection).
pub fn text(value: &str) -> String {
    let safe = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if safe.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        safe
    }
}

/// Uma linha já escapada, terminada em CRLF.
pub fn row(fields: &[String]) -> String {
    let mut line = fields.join(",");
    line.push_str("\r\n");
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_left_alone() {
        assert_eq!(text("abc123"), "abc123");
        assert_eq!(text("#8NPAD4P5"), "#8NPAD4P5");
        assert_eq!(text(""), "");
    }

    #[test]
    fn commas_quotes_and_line_breaks_get_quoted() {
        assert_eq!(text("a,b"), "\"a,b\"");
        assert_eq!(text("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(text("a\nb"), "\"a\nb\"");
        assert_eq!(text("a\r\nb"), "\"a\r\nb\"");
    }

    #[test]
    fn text_that_could_be_a_formula_is_neutralised() {
        for formula in ["=1+1", "+1", "-1", "@SUM(A1)", "\tx", "\rx"] {
            assert!(
                text(formula).starts_with('\'') || text(formula).starts_with("\"'"),
                "{formula}"
            );
        }
        assert_eq!(text("=1+1"), "'=1+1");
        // Junto com vírgula: o apóstrofo fica dentro das aspas.
        assert_eq!(text("=a,b"), "\"'=a,b\"");
    }

    #[test]
    fn a_dash_or_equals_in_the_middle_is_fine() {
        assert_eq!(text("a-b=c+d@e"), "a-b=c+d@e");
    }

    #[test]
    fn a_row_ends_with_crlf() {
        assert_eq!(row(&["a".into(), "1".into(), "".into()]), "a,1,\r\n");
        // `row` junta o que recebe; quem escapa é `text`.
        assert_eq!(row(&[text("x,y"), "2".into()]), "\"x,y\",2\r\n");
    }
}
