//! File type detection module.
//!
//! Automatically detects whether a file is text or binary based on content analysis.
//! Text files are stored using line-level COW for efficient diff storage.

use std::str;

/// Thresholds for text file detection.
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Maximum file size to be considered for text storage (bytes).
    /// Files larger than this are stored as binary.
    pub max_text_file_size: usize,

    /// Maximum line length (bytes) for text files.
    /// Files with lines longer than this are stored as binary.
    pub max_line_length: usize,

    /// Maximum percentage of non-printable characters allowed for text.
    pub max_non_printable_ratio: f64,

    /// Minimum ratio of newline characters for text detection.
    pub min_newline_ratio: f64,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_text_file_size: 10 * 1024 * 1024, // 10 MB
            max_line_length: 10 * 1024,           // 10 KB per line
            max_non_printable_ratio: 0.05,        // 5% non-printable allowed
            min_newline_ratio: 0.0001,            // At least some newlines
        }
    }
}

/// Information about detected file type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTypeInfo {
    /// Text file with detected encoding and line ending style.
    Text { encoding: TextEncoding, line_ending: LineEnding, line_count: usize },
    /// Binary file.
    Binary,
}

impl FileTypeInfo {
    /// Returns true if this is a text file.
    pub fn is_text(&self) -> bool {
        matches!(self, FileTypeInfo::Text { .. })
    }

    /// Returns true if this is a binary file.
    pub fn is_binary(&self) -> bool {
        matches!(self, FileTypeInfo::Binary)
    }
}

/// Supported text encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Ascii,
    Latin1,
    Unknown,
}

impl std::fmt::Display for TextEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextEncoding::Utf8 => write!(f, "utf-8"),
            TextEncoding::Ascii => write!(f, "ascii"),
            TextEncoding::Latin1 => write!(f, "latin-1"),
            TextEncoding::Unknown => write!(f, "unknown"),
        }
    }
}

/// Line ending styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style: \n
    Lf,
    /// Windows-style: \r\n
    CrLf,
    /// Old Mac-style: \r
    Cr,
    /// Mixed line endings
    Mixed,
    /// No line endings (single line)
    None,
}

impl std::fmt::Display for LineEnding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineEnding::Lf => write!(f, "lf"),
            LineEnding::CrLf => write!(f, "crlf"),
            LineEnding::Cr => write!(f, "cr"),
            LineEnding::Mixed => write!(f, "mixed"),
            LineEnding::None => write!(f, "none"),
        }
    }
}

/// File type detector.
pub struct FileTypeDetector {
    config: DetectionConfig,
}

impl FileTypeDetector {
    /// Create a new detector with default configuration.
    pub fn new() -> Self {
        Self::with_config(DetectionConfig::default())
    }

    /// Create a new detector with custom configuration.
    pub fn with_config(config: DetectionConfig) -> Self {
        Self { config }
    }

    /// Detect the file type from content.
    pub fn detect(&self, data: &[u8]) -> FileTypeInfo {
        // Empty files are considered text
        if data.is_empty() {
            return FileTypeInfo::Text {
                encoding: TextEncoding::Utf8,
                line_ending: LineEnding::None,
                line_count: 0,
            };
        }

        // Check size limit
        if data.len() > self.config.max_text_file_size {
            return FileTypeInfo::Binary;
        }

        // Check for null bytes (strong indicator of binary)
        if data.contains(&0) {
            return FileTypeInfo::Binary;
        }

        // Try to detect encoding
        let (encoding, text_result) = self.detect_encoding(data);

        let text = match text_result {
            Some(t) => t,
            None => return FileTypeInfo::Binary,
        };

        // Check for non-printable characters
        let non_printable_count = self.count_non_printable(data);
        let non_printable_ratio = non_printable_count as f64 / data.len() as f64;
        if non_printable_ratio > self.config.max_non_printable_ratio {
            return FileTypeInfo::Binary;
        }

        // Detect line endings and count lines
        let (line_ending, line_count, max_line_len) = self.analyze_lines(&text);

        // Check max line length
        if max_line_len > self.config.max_line_length {
            return FileTypeInfo::Binary;
        }

        FileTypeInfo::Text { encoding, line_ending, line_count }
    }

    /// Detect encoding and try to decode as text.
    fn detect_encoding(&self, data: &[u8]) -> (TextEncoding, Option<String>) {
        // Check for UTF-8 BOM
        let data_without_bom =
            if data.starts_with(&[0xEF, 0xBB, 0xBF]) { &data[3..] } else { data };

        // Try UTF-8 first
        if let Ok(text) = str::from_utf8(data_without_bom) {
            // Check if it's pure ASCII
            let is_ascii = data_without_bom.iter().all(|&b| b < 128);
            let encoding = if is_ascii { TextEncoding::Ascii } else { TextEncoding::Utf8 };
            return (encoding, Some(text.to_string()));
        }

        // Try Latin-1 (ISO-8859-1) - always succeeds for any byte sequence
        // but we only use it if it looks reasonable
        let text: String = data_without_bom.iter().map(|&b| b as char).collect();
        let printable_ratio = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .count() as f64
            / text.len() as f64;

        if printable_ratio > 0.9 {
            return (TextEncoding::Latin1, Some(text));
        }

        (TextEncoding::Unknown, None)
    }

    /// Count non-printable characters (excluding common whitespace).
    fn count_non_printable(&self, data: &[u8]) -> usize {
        data.iter()
            .filter(|&&b| {
                // Allow: tab, newline, carriage return, and printable ASCII
                !(b == b'\t' || b == b'\n' || b == b'\r' || (0x20..0x7F).contains(&b) || b >= 0x80)
            })
            .count()
    }

    /// Analyze lines in the text.
    fn analyze_lines(&self, text: &str) -> (LineEnding, usize, usize) {
        let mut lf_count = 0;
        let mut crlf_count = 0;
        let mut cr_only_count = 0;
        let mut max_line_len = 0;
        let mut current_line_len = 0;
        let mut line_count = 1;

        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '\r' {
                if i + 1 < chars.len() && chars[i + 1] == '\n' {
                    crlf_count += 1;
                    i += 2;
                } else {
                    cr_only_count += 1;
                    i += 1;
                }
                max_line_len = max_line_len.max(current_line_len);
                current_line_len = 0;
                line_count += 1;
            } else if chars[i] == '\n' {
                lf_count += 1;
                i += 1;
                max_line_len = max_line_len.max(current_line_len);
                current_line_len = 0;
                line_count += 1;
            } else {
                current_line_len += chars[i].len_utf8();
                i += 1;
            }
        }

        // Account for last line if it doesn't end with newline
        max_line_len = max_line_len.max(current_line_len);

        // Determine predominant line ending
        let total_endings = lf_count + crlf_count + cr_only_count;
        let line_ending = if total_endings == 0 {
            LineEnding::None
        } else if crlf_count > 0 && lf_count == 0 && cr_only_count == 0 {
            LineEnding::CrLf
        } else if lf_count > 0 && crlf_count == 0 && cr_only_count == 0 {
            LineEnding::Lf
        } else if cr_only_count > 0 && lf_count == 0 && crlf_count == 0 {
            LineEnding::Cr
        } else {
            LineEnding::Mixed
        };

        (line_ending, line_count, max_line_len)
    }
}

impl Default for FileTypeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_empty_file() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"");
        assert!(info.is_text());
    }

    #[test]
    fn test_detect_ascii_text() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"Hello, World!\nThis is a test.\n");
        match info {
            FileTypeInfo::Text { encoding, line_ending, line_count } => {
                assert_eq!(encoding, TextEncoding::Ascii);
                assert_eq!(line_ending, LineEnding::Lf);
                assert_eq!(line_count, 3);
            }
            _ => panic!("Expected text file"),
        }
    }

    #[test]
    fn test_detect_utf8_text() {
        let detector = FileTypeDetector::new();
        let info = detector.detect("Hello, 世界!\n你好\n".as_bytes());
        match info {
            FileTypeInfo::Text { encoding, line_ending, .. } => {
                assert_eq!(encoding, TextEncoding::Utf8);
                assert_eq!(line_ending, LineEnding::Lf);
            }
            _ => panic!("Expected text file"),
        }
    }

    #[test]
    fn test_detect_crlf_text() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"Line 1\r\nLine 2\r\nLine 3\r\n");
        match info {
            FileTypeInfo::Text { line_ending, line_count, .. } => {
                assert_eq!(line_ending, LineEnding::CrLf);
                assert_eq!(line_count, 4);
            }
            _ => panic!("Expected text file"),
        }
    }

    #[test]
    fn test_detect_binary_with_null() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"Hello\x00World");
        assert!(info.is_binary());
    }

    #[test]
    fn test_detect_binary_large_file() {
        let config = DetectionConfig { max_text_file_size: 100, ..Default::default() };
        let detector = FileTypeDetector::with_config(config);
        let large_content = vec![b'a'; 200];
        let info = detector.detect(&large_content);
        assert!(info.is_binary());
    }

    #[test]
    fn test_detect_single_line() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"No newline at the end");
        match info {
            FileTypeInfo::Text { line_ending, line_count, .. } => {
                assert_eq!(line_ending, LineEnding::None);
                assert_eq!(line_count, 1);
            }
            _ => panic!("Expected text file"),
        }
    }

    #[test]
    fn test_detect_mixed_line_endings() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"Line 1\nLine 2\r\nLine 3\n");
        match info {
            FileTypeInfo::Text { line_ending, .. } => {
                assert_eq!(line_ending, LineEnding::Mixed);
            }
            _ => panic!("Expected text file"),
        }
    }

    // Additional tests for better coverage

    #[test]
    fn test_detection_config_default() {
        let config = DetectionConfig::default();
        assert_eq!(config.max_text_file_size, 10 * 1024 * 1024);
        assert_eq!(config.max_line_length, 10 * 1024);
        assert!(config.max_non_printable_ratio > 0.0);
    }

    #[test]
    fn test_detection_config_custom() {
        let config = DetectionConfig {
            max_text_file_size: 1000,
            max_line_length: 100,
            max_non_printable_ratio: 0.1,
            min_newline_ratio: 0.001,
        };
        assert_eq!(config.max_text_file_size, 1000);
        assert_eq!(config.max_line_length, 100);
        assert_eq!(config.max_non_printable_ratio, 0.1);
    }

    #[test]
    fn test_file_type_info_is_text() {
        let text_info = FileTypeInfo::Text {
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::Lf,
            line_count: 10,
        };
        assert!(text_info.is_text());
        assert!(!text_info.is_binary());
    }

    #[test]
    fn test_file_type_info_is_binary() {
        let binary_info = FileTypeInfo::Binary;
        assert!(binary_info.is_binary());
        assert!(!binary_info.is_text());
    }

    #[test]
    fn test_text_encoding_variants() {
        assert_eq!(TextEncoding::Ascii, TextEncoding::Ascii);
        assert_eq!(TextEncoding::Utf8, TextEncoding::Utf8);
        assert_eq!(TextEncoding::Unknown, TextEncoding::Unknown);
        assert_ne!(TextEncoding::Ascii, TextEncoding::Utf8);
    }

    #[test]
    fn test_line_ending_variants() {
        assert_eq!(LineEnding::Lf, LineEnding::Lf);
        assert_eq!(LineEnding::CrLf, LineEnding::CrLf);
        assert_eq!(LineEnding::Cr, LineEnding::Cr);
        assert_eq!(LineEnding::Mixed, LineEnding::Mixed);
        assert_eq!(LineEnding::None, LineEnding::None);
        assert_ne!(LineEnding::Lf, LineEnding::CrLf);
    }

    #[test]
    fn test_detect_cr_only_line_ending() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"Line 1\rLine 2\rLine 3\r");
        match info {
            FileTypeInfo::Text { line_ending, .. } => {
                assert_eq!(line_ending, LineEnding::Cr);
            }
            _ => panic!("Expected text file"),
        }
    }

    #[test]
    fn test_detect_long_line_as_binary() {
        let config = DetectionConfig { max_line_length: 10, ..Default::default() };
        let detector = FileTypeDetector::with_config(config);
        let info = detector.detect(b"This is a very long line that exceeds the maximum");
        assert!(info.is_binary());
    }

    #[test]
    fn test_detect_tabs_and_spaces() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"Hello\tWorld\n    Indented\n");
        assert!(info.is_text());
    }

    #[test]
    fn test_detect_utf8_bom() {
        let detector = FileTypeDetector::new();
        // UTF-8 BOM: EF BB BF followed by non-ASCII content
        let mut content = vec![0xEF, 0xBB, 0xBF];
        content.extend_from_slice("Hello 世界\n".as_bytes());
        let info = detector.detect(&content);
        match info {
            FileTypeInfo::Text { encoding, .. } => {
                // BOM alone doesn't force UTF-8 detection, need actual UTF-8 content
                assert_eq!(encoding, TextEncoding::Utf8);
            }
            _ => panic!("Expected text file with UTF-8"),
        }
    }

    #[test]
    fn test_detect_high_non_printable_as_binary() {
        let config = DetectionConfig { max_non_printable_ratio: 0.01, ..Default::default() };
        let detector = FileTypeDetector::with_config(config);
        // Content with many control characters
        let mut content = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        content.extend_from_slice(b"Hello");
        let info = detector.detect(&content);
        assert!(info.is_binary());
    }

    #[test]
    fn test_detect_valid_utf8_multibyte() {
        let detector = FileTypeDetector::new();
        // Various UTF-8 characters
        let info = detector.detect("日本語テスト\n한국어\nРусский\n".as_bytes());
        match info {
            FileTypeInfo::Text { encoding, line_count, .. } => {
                assert_eq!(encoding, TextEncoding::Utf8);
                assert_eq!(line_count, 4);
            }
            _ => panic!("Expected UTF-8 text file"),
        }
    }

    #[test]
    fn test_detect_invalid_utf8_as_binary() {
        let detector = FileTypeDetector::new();
        // Invalid UTF-8 sequence
        let content = vec![0x80, 0x81, 0x82, 0x83];
        let info = detector.detect(&content);
        // Should be detected as binary due to invalid UTF-8
        assert!(info.is_binary());
    }

    #[test]
    fn test_detector_default_impl() {
        let detector = FileTypeDetector::default();
        let info = detector.detect(b"test");
        assert!(info.is_text());
    }

    #[test]
    fn test_detect_only_newlines() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"\n\n\n\n");
        match info {
            FileTypeInfo::Text { line_ending, line_count, .. } => {
                assert_eq!(line_ending, LineEnding::Lf);
                assert_eq!(line_count, 5);
            }
            _ => panic!("Expected text file"),
        }
    }

    #[test]
    fn test_detect_whitespace_only() {
        let detector = FileTypeDetector::new();
        let info = detector.detect(b"   \t\t   \n   \t\n");
        assert!(info.is_text());
    }

    #[test]
    fn test_binary_simple() {
        let info = FileTypeInfo::Binary;
        assert!(info.is_binary());
        assert!(!info.is_text());
    }
}
