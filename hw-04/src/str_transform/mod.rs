use std::str::FromStr;
use std::path::Path;
use std::error::Error;

pub mod csv_pretty;

pub struct StrTransformMessage {
    pub operation: StrTransformOperation,
    pub args: String
}

impl StrTransformMessage {
    pub fn new(operation: StrTransformOperation, args: String) -> Self {
        StrTransformMessage{operation,args}
    }
}

pub enum StrTransformOperation {
    Lowercase, Uppercase, NoSpaces, Slugify, Csv
}

impl StrTransformOperation {
    pub fn perform(&self, input: &str) -> Result<String, Box<dyn Error>> {
        match self {
            Self::Lowercase => lowercase(input),
            Self::Uppercase => uppercase(input),
            Self::NoSpaces => no_spaces(input),
            Self::Slugify => slugify(input),
            Self::Csv => csv_pretty::render_file(Path::new(input.trim()))
        }
    }
}

#[derive(Debug,thiserror::Error)]
pub enum StrTransformError {
    #[error("Invalid transformation.")]
    InvalidTransform
}

impl FromStr for StrTransformOperation {
    type Err = StrTransformError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lowercase" => Ok(Self::Lowercase),
            "uppercase" => Ok(Self::Uppercase),
            "no-spaces" => Ok(Self::NoSpaces),
            "slugify" => Ok(Self::Slugify),
            "csv" => Ok(Self::Csv),
            _ => Err(StrTransformError::InvalidTransform)
        }
    }
}

pub fn slugify(input: &str) -> Result<String, Box<dyn Error>> {
    match input.chars().last() {
        Some('\n') => Ok(format!("{}\n", slug::slugify(input))),
        _ => Ok(input.to_string())
    }
}

pub fn lowercase(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(input.to_lowercase())
}

pub fn uppercase(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(input.to_uppercase())
}

pub fn no_spaces(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(input.replace(' ', ""))
}
