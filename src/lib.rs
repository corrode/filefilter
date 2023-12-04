//! Example code for the blog post on language patterns in Rust.
//!
//! Shows how to implement an object-oriented file system walker that can be
//! configured with different predicates to filter the files that are returned.
//!
//! The code is based on the [walkdir](https://crates.io/crates/walkdir) crate,
//! which is the production-ready solution for this problem.
#![warn(clippy::all, clippy::pedantic)]
#![warn(
    absolute_paths_not_starting_with_crate,
    rustdoc::invalid_html_tags,
    missing_copy_implementations,
    semicolon_in_expressions_from_macros,
    unreachable_pub,
    unused_extern_crates,
    variant_size_differences,
    clippy::missing_const_for_fn
)]
#![deny(anonymous_parameters, macro_use_extern_crate, pointer_structural_match)]
#![deny(missing_docs)]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// A type for our predicate functions, which take a `Path` and return a `bool`.
type Predicate = dyn Fn(&Path) -> bool;

/// A file system walker.
///
/// Provides a recursive iterator over the files in a directory tree
/// based on the given configuration
pub struct FileFilter {
    /// The predicate to use to filter files.
    predicates: Vec<Box<Predicate>>,
    /// The start path.
    ///
    /// This is only `Some(...)` at the beginning.
    /// After the first iteration, this is always `None`.
    start: Option<PathBuf>,
    /// The stack of directories to traverse
    stack: Vec<fs::ReadDir>,
}

impl FileFilter {
    /// Create a new `FileFilter` that will recursively walk the directory
    /// starting at `root`.
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        FileFilter {
            predicates: vec![],
            start: Some(root.as_ref().to_path_buf()),
            stack: vec![],
        }
    }

    /// Add a new filter predicate.
    ///
    /// Returns `self` to allow chaining.
    #[must_use]
    pub fn add_filter(mut self, predicate: impl Fn(&Path) -> bool + 'static) -> Self {
        self.predicates.push(Box::new(predicate));
        self
    }

    /// Process a single entry and check if it matches the predicates.
    fn process_entry(&mut self, path: PathBuf) -> Option<Result<PathBuf>> {
        if path.is_dir() {
            // Push directories onto the stack
            if let Err(e) = self.push(&path) {
                return Some(Err(e));
            }
            None
        } else {
            // Check files against the predicates
            if self.predicates.iter().all(|f| f(&path)) {
                Some(Ok(path))
            } else {
                None
            }
        }
    }

    /// Read dir and push it onto the stack
    fn push(&mut self, entry: &PathBuf) -> Result<()> {
        let rd = fs::read_dir(entry)?;
        self.stack.push(rd);
        Ok(())
    }
}

impl Iterator for FileFilter {
    type Item = Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        // Takes the value out of the option, leaving a `None` in its place.
        // In the next iteration, this will be `None` and we'll skip this block.
        if let Some(start) = self.start.take() {
            // Process the initial start path
            if let Some(result) = self.process_entry(start) {
                return Some(result);
            }
        }

        // `last_mut` returns a mutable pointer to the last item in the slice.
        // We need this to be able to call `rd.next()`, which mutates the
        // iterator.
        while let Some(rd) = self.stack.last_mut() {
            match rd.next() {
                Some(Ok(entry)) => {
                    if let Some(result) = self.process_entry(entry.path()) {
                        return Some(result);
                    }
                }
                Some(Err(e)) => return Some(Err(Box::new(e))),
                None => {
                    // Pop empty directory
                    self.stack.pop();
                }
            }
        }

        // If the stack is empty, we're done
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_txt_file(path: &Path) -> bool {
        path.extension().unwrap_or_default() == "txt"
    }

    fn has_prefix(path: &Path) -> bool {
        path.file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .starts_with("prefix_")
    }

    #[test]
    fn test_filter_txt_files() -> Result<()> {
        let file_filter = FileFilter::new("fixtures/test_structure").add_filter(is_txt_file);
        let entries = file_filter.collect::<Result<Vec<_>>>()?;

        assert!(!entries.is_empty());

        for entry in entries {
            assert_eq!(entry.extension().unwrap().to_str().unwrap(), "txt");
        }
        Ok(())
    }

    #[test]
    fn test_filter_prefix_files() -> Result<()> {
        let file_filter = FileFilter::new("fixtures/test_structure").add_filter(has_prefix);
        let entries = file_filter.collect::<Result<Vec<_>>>()?;

        assert!(!entries.is_empty());

        for entry in entries {
            let file_name = entry.file_name().unwrap().to_str().unwrap();
            assert!(file_name.starts_with("prefix_"));
        }
        Ok(())
    }

    #[test]
    fn test_filter_txt_and_prefix_files() -> Result<()> {
        let file_filter = FileFilter::new("fixtures/test_structure")
            .add_filter(has_prefix)
            .add_filter(is_txt_file);

        let entries = file_filter.collect::<Result<Vec<_>>>()?;

        assert!(!entries.is_empty());

        for entry in entries {
            let file_name = entry.file_name().unwrap().to_str().unwrap();
            assert!(file_name.starts_with("prefix_"));
            assert_eq!(entry.extension().unwrap().to_str().unwrap(), "txt");
        }
        Ok(())
    }
}
