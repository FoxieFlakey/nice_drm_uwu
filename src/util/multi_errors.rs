use std::fmt;

#[derive(Debug)]
pub struct MultipleErrors {
    errors: Vec<anyhow::Error>,
}

// CHAT GPT generated

impl MultipleErrors {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn push(&mut self, error: anyhow::Error) {
        self.errors.push(error);
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for MultipleErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MultipleErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} errors occurred:", self.errors.len())?;

        for (index, error) in self.errors.iter().enumerate() {
            writeln!(f, "  {}. {error:#?}", index + 1)?;
        }

        Ok(())
    }
}

impl std::error::Error for MultipleErrors {}
