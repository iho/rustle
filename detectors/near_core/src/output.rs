//! Append-mode writer for detector `.tmp` output files.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub struct TmpWriter {
    path: PathBuf,
}

impl TmpWriter {
    /// Create a writer for `$TMP_DIR/.<detector_name>.tmp`.
    /// Falls back to `./.tmp/` if `TMP_DIR` is not set.
    pub fn new(detector_name: &str) -> Self {
        let tmp_dir = std::env::var("TMP_DIR").unwrap_or_else(|_| "./.tmp/".to_string());
        let tmp_dir = tmp_dir.trim_end_matches('/');
        std::fs::create_dir_all(tmp_dir)
            .unwrap_or_else(|e| panic!("cannot create TMP_DIR '{}': {}", tmp_dir, e));
        let path = PathBuf::from(format!("{}/.{}.tmp", tmp_dir, detector_name));
        // Create the file if it doesn't exist (mirrors C++ behavior of opening at startup)
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("cannot create {:?}: {}", path, e));
        TmpWriter { path }
    }

    /// Append `func_name@filename@line\n` — standard finding format.
    pub fn write(&self, func_name: &str, filename: &str, line: u32) {
        self.write_raw(&format!("{}@{}@{}", func_name, filename, line));
    }

    /// Append `name\n` — single-field format (e.g. for ext_call_trait lists).
    pub fn write_name(&self, name: &str) {
        self.write_raw(name);
    }

    /// Append `func_name@filename\n` — two-field format used by the callback detector.
    pub fn write_func_file(&self, func_name: &str, filename: &str) {
        self.write_raw(&format!("{}@{}", func_name, filename));
    }

    /// Append `func_name@True\n` or `func_name@False\n`.
    /// Used by self_transfer, prepaid_gas and similar boolean detectors.
    pub fn write_bool(&self, func_name: &str, value: bool) {
        self.write_raw(&format!(
            "{}@{}",
            func_name,
            if value { "True" } else { "False" }
        ));
    }

    fn write_raw(&self, content: &str) {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .unwrap_or_else(|e| panic!("cannot open {:?}: {}", self.path, e));
        writeln!(file, "{}", content)
            .unwrap_or_else(|e| panic!("cannot write {:?}: {}", self.path, e));
    }
}
