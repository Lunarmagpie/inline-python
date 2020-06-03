use proc_macro2::{Span, TokenStream};
use pyo3::type_object::PyTypeObject;
use pyo3::{PyAny, AsPyRef, PyErr, PyResult, Python, ToPyObject};

/// Format a nice error message for a python compilation error.
pub fn emit_compile_error_msg(py: Python, error: PyErr, tokens: TokenStream) {
	let value = error.to_object(py);

	if value.is_none() {
		Span::call_site()
			.unwrap()
			.error(format!("python: {}", error.ptype.as_ref(py).name()))
			.emit();
		return;
	}

	if error.matches(py, pyo3::exceptions::SyntaxError::type_object()) {
		let line: Option<usize> = value.getattr(py, "lineno").ok().and_then(|x| x.extract(py).ok());
		let msg: Option<String> = value.getattr(py, "msg").ok().and_then(|x| x.extract(py).ok());
		if let (Some(line), Some(msg)) = (line, msg) {
			if let Some(span) = span_for_line(tokens.clone(), line) {
				span.unwrap().error(format!("python: {}", msg)).emit();
				return;
			}
		}
	}

	if let Some(tb) = &error.ptraceback {
		if let Ok((file, line)) = get_traceback_info(tb.as_ref(py)) {
			if file == Span::call_site().unwrap().source_file().path().to_string_lossy() {
				if let Ok(msg) = value.as_ref(py).str() {
					if let Some(span) = span_for_line(tokens, line) {
						span.unwrap().error(format!("python: {}", msg)).emit();
						return;
					}
				}
			}
		}
	}

	Span::call_site()
		.unwrap()
		.error(format!("python: {}", value.as_ref(py).str().unwrap()))
		.emit();
}

fn get_traceback_info(tb: &PyAny) -> PyResult<(String, usize)> {
	let frame = tb.getattr("tb_frame")?;
	let code = frame.getattr("f_code")?;
	let file: String = code.getattr("co_filename")?.extract()?;
	let line: usize = frame.getattr("f_lineno")?.extract()?;
	Ok((file, line))
}

/// Get a span for a specific line of input from a TokenStream.
fn span_for_line(input: TokenStream, line: usize) -> Option<Span> {
	let mut spans = input
		.into_iter()
		.map(|x| x.span().unwrap())
		.skip_while(|span| span.start().line < line)
		.take_while(|span| span.start().line == line);

	let mut result = spans.next()?;
	for span in spans {
		result = match result.join(span) {
			None => return Some(Span::from(result)),
			Some(span) => span,
		}
	}

	Some(Span::from(result))
}