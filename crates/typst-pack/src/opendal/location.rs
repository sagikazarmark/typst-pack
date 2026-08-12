use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};

/// A caller-defined name for an OpenDAL Operator.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperatorBinding(String);

impl OperatorBinding {
    /// Constructs a lowercase RFC scheme-style Operator binding.
    pub fn new(value: impl AsRef<str>) -> Result<Self, OperatorBindingError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(OperatorBindingError::Empty);
        }

        for (index, character) in value.char_indices() {
            if character.is_ascii_uppercase() {
                return Err(OperatorBindingError::NonLowercaseCharacter { index, character });
            }
            if index == 0 && !character.is_ascii_lowercase() {
                return Err(OperatorBindingError::InvalidInitialCharacter { index, character });
            }
            if index > 0
                && !(character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '+' | '.' | '-'))
            {
                return Err(OperatorBindingError::InvalidCharacter { index, character });
            }
        }

        Ok(Self(value.to_owned()))
    }

    /// Returns this binding's canonical spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for OperatorBinding {
    type Err = OperatorBindingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl fmt::Display for OperatorBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for OperatorBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// A reason an Operator binding is not canonical.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OperatorBindingError {
    #[error("an Operator binding cannot be empty")]
    Empty,
    #[error("invalid initial character {character:?} at byte {index}")]
    InvalidInitialCharacter { index: usize, character: char },
    #[error("invalid character {character:?} at byte {index}")]
    InvalidCharacter { index: usize, character: char },
    #[error("uppercase character {character:?} at byte {index}")]
    NonLowercaseCharacter { index: usize, character: char },
}

/// A canonical location addressed through a caller-supplied Operator binding.
///
/// Exact objects are non-root paths without a trailing slash. Prefixes are the
/// root or non-root paths with a trailing slash.
///
/// Import this module instead of the type when [`std::panic::Location`] is also
/// in scope:
///
/// ```
/// use typst_pack::opendal::location;
///
/// let object: location::Location = "archive:/packs/document.typk".parse()?;
/// # Ok::<(), location::LocationError>(())
/// ```
///
/// ```
/// use typst_pack::opendal::{Location, OperatorBinding};
///
/// let object: Location = "archive:/packs/document.typk".parse()?;
/// assert_eq!(object.operation_path(), "packs/document.typk");
/// assert_eq!(object.to_string(), "archive:/packs/document.typk");
///
/// let binding = OperatorBinding::new("archive")?;
/// let prefix = Location::from_operation_path(binding, "packages/café/")?;
/// assert_eq!(prefix.to_string(), "archive:/packages/caf%C3%A9/");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Location {
    binding: OperatorBinding,
    operation_path: String,
}

impl Location {
    /// Parses a canonical `binding:/path` location.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, LocationError> {
        let value = value.as_ref();
        let Some(binding_end) = value.find(':') else {
            return Err(LocationError::MissingBindingSeparator);
        };
        let binding = OperatorBinding::new(&value[..binding_end])
            .map_err(|source| LocationError::InvalidBinding { source })?;
        let suffix_offset = binding_end + 1;
        let suffix = &value[suffix_offset..];

        if let Some(authority) = suffix.strip_prefix("//") {
            let authority_end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
            if let Some(index) = authority[..authority_end].find('@') {
                return Err(LocationError::UserInfoNotAllowed {
                    index: suffix_offset + 2 + index,
                });
            }
            return Err(LocationError::AuthorityNotAllowed {
                index: suffix_offset + 1,
            });
        }

        let Some(path) = suffix.strip_prefix('/') else {
            return Err(LocationError::MissingAbsolutePath {
                index: suffix_offset,
            });
        };
        let operation_path = decode_uri_path(path, suffix_offset + 1)?;

        Ok(Self {
            binding,
            operation_path,
        })
    }

    /// Constructs a location from a decoded root-relative OpenDAL operation path.
    pub fn from_operation_path(
        binding: OperatorBinding,
        operation_path: impl AsRef<str>,
    ) -> Result<Self, LocationError> {
        let operation_path = operation_path.as_ref();
        if operation_path.is_empty() || operation_path == "/" {
            return Ok(Self {
                binding,
                operation_path: String::new(),
            });
        }
        if operation_path.starts_with('/') {
            return Err(LocationError::MissingAbsolutePath { index: 0 });
        }
        validate_decoded_operation_path(operation_path)?;

        Ok(Self {
            binding,
            operation_path: operation_path.to_owned(),
        })
    }

    /// Returns the Operator binding used by this location.
    pub fn binding(&self) -> &OperatorBinding {
        &self.binding
    }

    /// Returns the decoded root-relative operation path.
    pub fn operation_path(&self) -> &str {
        &self.operation_path
    }

    /// Reports whether this location names the root.
    pub fn is_root(&self) -> bool {
        self.operation_path.is_empty()
    }

    /// Reports whether this location names a prefix form.
    pub fn has_trailing_slash(&self) -> bool {
        self.is_root() || self.operation_path.ends_with('/')
    }

    #[allow(dead_code)]
    pub(crate) fn dispatch_path(&self) -> &str {
        if self.is_root() {
            "/"
        } else {
            self.operation_path()
        }
    }

    #[allow(dead_code)]
    pub(crate) fn require_object(&self) -> Result<(), LocationRoleError> {
        if self.is_root() {
            Err(LocationRoleError::ObjectAtRoot)
        } else if self.has_trailing_slash() {
            Err(LocationRoleError::ObjectHasTrailingSlash)
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub(crate) fn require_prefix(&self) -> Result<(), LocationRoleError> {
        if self.has_trailing_slash() {
            Ok(())
        } else {
            Err(LocationRoleError::PrefixMissingTrailingSlash)
        }
    }

    #[allow(dead_code)]
    pub(crate) fn compose(&self, child: &str) -> Result<Self, LocationError> {
        let mut operation_path = String::with_capacity(self.operation_path.len() + child.len());
        operation_path.push_str(&self.operation_path);
        operation_path.push_str(child);
        Self::from_operation_path(self.binding.clone(), operation_path)
    }

    #[allow(dead_code)]
    pub(crate) fn relative_file_path<'a>(
        &self,
        candidate: &'a str,
    ) -> Result<&'a str, PrefixConfinementError> {
        debug_assert!(self.require_prefix().is_ok());

        if candidate.is_empty() {
            return Err(PrefixConfinementError::EmptyPath);
        }
        if self.is_root() {
            if candidate == "/" {
                return Err(PrefixConfinementError::PrefixMarker);
            }
            if candidate.starts_with('/') {
                return Err(PrefixConfinementError::OutsidePrefix);
            }
            return Ok(candidate);
        }
        if candidate == self.operation_path {
            return Err(PrefixConfinementError::PrefixMarker);
        }
        let Some(relative) = candidate.strip_prefix(&self.operation_path) else {
            return Err(PrefixConfinementError::OutsidePrefix);
        };
        if relative.is_empty() {
            return Err(PrefixConfinementError::EmptyPath);
        }
        Ok(relative)
    }

    #[cfg(fuzzing)]
    #[doc(hidden)]
    pub fn fuzz_role_checks(
        &self,
    ) -> (Result<(), LocationRoleError>, Result<(), LocationRoleError>) {
        (self.require_object(), self.require_prefix())
    }

    #[cfg(fuzzing)]
    #[doc(hidden)]
    pub fn fuzz_compose(&self, child: &str) -> Result<Self, LocationError> {
        self.compose(child)
    }
}

impl FromStr for Location {
    type Err = LocationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:/", self.binding)?;
        for byte in self.operation_path.bytes() {
            if byte == b'/' || is_pchar(byte) {
                formatter.write_str(char::from(byte).encode_utf8(&mut [0; 4]))?;
            } else {
                write!(formatter, "%{byte:02X}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// A reason a location is unsafe or not canonically spelled.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum LocationError {
    #[error("the location is missing its Operator binding separator")]
    MissingBindingSeparator,
    #[error("the location has an invalid Operator binding: {source}")]
    InvalidBinding { source: OperatorBindingError },
    #[error("the location is missing its absolute path slash at byte {index}")]
    MissingAbsolutePath { index: usize },
    #[error("an authority is not allowed at byte {index}")]
    AuthorityNotAllowed { index: usize },
    #[error("userinfo is not allowed at byte {index}")]
    UserInfoNotAllowed { index: usize },
    #[error("a query is not allowed at byte {index}")]
    QueryNotAllowed { index: usize },
    #[error("a fragment is not allowed at byte {index}")]
    FragmentNotAllowed { index: usize },
    #[error("raw non-ASCII input is not allowed at byte {index}")]
    RawNonAscii { index: usize },
    #[error("a control character is not allowed at byte {index}")]
    ControlCharacter { index: usize },
    #[error("a backslash is not allowed at byte {index}")]
    Backslash { index: usize },
    #[error("a malformed percent escape starts at byte {index}")]
    MalformedPercentEscape { index: usize },
    #[error("a noncanonical percent escape starts at byte {index}")]
    NoncanonicalPercentEscape { index: usize },
    #[error("an encoded path character starts at byte {index}")]
    EncodedPchar { index: usize },
    #[error("an encoded path separator starts at byte {index}")]
    EncodedSeparator { index: usize },
    #[error("an encoded backslash starts at byte {index}")]
    EncodedBackslash { index: usize },
    #[error("percent escapes produce invalid UTF-8 at byte {index}")]
    InvalidUtf8 { index: usize },
    #[error("a repeated path separator occurs at byte {index}")]
    RepeatedSeparator { index: usize },
    #[error("a dot segment starts at byte {index}")]
    DotSegment { index: usize },
    #[error("the path aliases another operation path at byte {index}")]
    NormalizationAlias { index: usize },
    #[error("path character {character:?} must be percent-encoded at byte {index}")]
    NoncanonicalPathCharacter { index: usize, character: char },
}

/// A reason a location cannot serve an exact-object or prefix role.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum LocationRoleError {
    #[error("an exact object cannot be located at root")]
    ObjectAtRoot,
    #[error("an exact object location cannot have a trailing slash")]
    ObjectHasTrailingSlash,
    #[error("a non-root prefix location must have a trailing slash")]
    PrefixMissingTrailingSlash,
}

/// Resolves an Operator binding without rewriting operation paths.
pub trait OperatorResolver {
    type Error: Error + 'static;

    fn resolve(&self, binding: &OperatorBinding) -> Result<::opendal::Operator, Self::Error>;
}

/// An immutable lexical map of caller-supplied Operators.
///
/// The caller constructs each Operator and may bind clones of one Operator to
/// distinct names. Consumers can accept the map through [`OperatorResolver`]
/// without knowing how any backend was configured.
///
/// ```
/// use typst_pack::opendal::{
///     OperatorBinding, OperatorBindings, OperatorResolver,
/// };
///
/// fn resolve_for_consumer<R: OperatorResolver>(
///     resolver: &R,
///     binding: &OperatorBinding,
/// ) -> Result<opendal::Operator, R::Error> {
///     resolver.resolve(binding)
/// }
///
/// let operator = opendal::Operator::new(opendal::services::Memory::default())?;
/// let archive = OperatorBinding::new("archive")?;
/// let project = OperatorBinding::new("project")?;
/// let bindings = OperatorBindings::new([
///     (project, operator.clone()),
///     (archive.clone(), operator),
/// ])?;
///
/// assert_eq!(
///     bindings
///         .bindings()
///         .map(OperatorBinding::as_str)
///         .collect::<Vec<_>>(),
///     ["archive", "project"]
/// );
/// let _direct_operator = bindings.operator(&archive).expect("archive is configured");
/// let _resolved_operator = resolve_for_consumer(&bindings, &archive)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct OperatorBindings {
    operators: BTreeMap<OperatorBinding, ::opendal::Operator>,
}

impl OperatorBindings {
    /// Builds bindings and rejects duplicate names.
    pub fn new(
        entries: impl IntoIterator<Item = (OperatorBinding, ::opendal::Operator)>,
    ) -> Result<Self, OperatorBindingsError> {
        let mut operators = BTreeMap::new();
        for (binding, operator) in entries {
            if operators.insert(binding.clone(), operator).is_some() {
                return Err(OperatorBindingsError::DuplicateBinding { binding });
            }
        }
        Ok(Self { operators })
    }

    /// Lists binding names in lexical order.
    pub fn bindings(&self) -> impl ExactSizeIterator<Item = &OperatorBinding> {
        self.operators.keys()
    }

    /// Returns a cheap clone of the Operator for `binding`.
    pub fn operator(&self, binding: &OperatorBinding) -> Option<::opendal::Operator> {
        self.operators.get(binding).cloned()
    }
}

impl fmt::Debug for OperatorBindings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperatorBindings")
            .field("bindings", &DisplayBindings(self.operators.keys()))
            .finish()
    }
}

impl OperatorResolver for OperatorBindings {
    type Error = OperatorBindingsResolveError;

    fn resolve(&self, binding: &OperatorBinding) -> Result<::opendal::Operator, Self::Error> {
        self.operator(binding)
            .ok_or_else(|| OperatorBindingsResolveError::UnknownBinding {
                binding: binding.clone(),
            })
    }
}

/// A reason an immutable Operator binding map cannot be built.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OperatorBindingsError {
    #[error("duplicate Operator binding {binding}")]
    DuplicateBinding { binding: OperatorBinding },
}

/// A reason an Operator binding cannot be resolved.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OperatorBindingsResolveError {
    #[error("unknown Operator binding {binding}")]
    UnknownBinding { binding: OperatorBinding },
}

struct DisplayBindings<'a>(
    std::collections::btree_map::Keys<'a, OperatorBinding, ::opendal::Operator>,
);

impl fmt::Debug for DisplayBindings<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.0.clone()).finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum PrefixConfinementError {
    OutsidePrefix,
    PrefixMarker,
    EmptyPath,
}

struct DecodedPath {
    value: String,
    source_offsets: Vec<usize>,
    encoded_bytes: Vec<bool>,
}

fn decode_uri_path(path: &str, path_offset: usize) -> Result<String, LocationError> {
    if let Some(index) = path.find('?') {
        return Err(LocationError::QueryNotAllowed {
            index: path_offset + index,
        });
    }
    if let Some(index) = path.find('#') {
        return Err(LocationError::FragmentNotAllowed {
            index: path_offset + index,
        });
    }
    if let Some((index, _)) = path
        .char_indices()
        .find(|(_, character)| !character.is_ascii())
    {
        return Err(LocationError::RawNonAscii {
            index: path_offset + index,
        });
    }
    if let Some((index, _)) = path
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(LocationError::ControlCharacter {
            index: path_offset + index,
        });
    }
    if let Some(index) = path.find('\\') {
        return Err(LocationError::Backslash {
            index: path_offset + index,
        });
    }

    for (index, escape) in percent_escapes(path) {
        let Some([high, low]) = escape else {
            return Err(LocationError::MalformedPercentEscape {
                index: path_offset + index,
            });
        };
        if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
            return Err(LocationError::MalformedPercentEscape {
                index: path_offset + index,
            });
        }
    }
    for (index, escape) in percent_escapes(path) {
        let [high, low] = escape.expect("escapes were validated above");
        if high.is_ascii_lowercase() || low.is_ascii_lowercase() {
            return Err(LocationError::NoncanonicalPercentEscape {
                index: path_offset + index,
            });
        }
    }
    for (index, escape) in percent_escapes(path) {
        let [high, low] = escape.expect("escapes were validated above");
        let byte = decode_hex(high, low);
        if byte == b'/' {
            return Err(LocationError::EncodedSeparator {
                index: path_offset + index,
            });
        }
    }
    for (index, escape) in percent_escapes(path) {
        let [high, low] = escape.expect("escapes were validated above");
        let byte = decode_hex(high, low);
        if byte == b'\\' {
            return Err(LocationError::EncodedBackslash {
                index: path_offset + index,
            });
        }
    }
    for (index, escape) in percent_escapes(path) {
        let [high, low] = escape.expect("escapes were validated above");
        if is_pchar(decode_hex(high, low)) {
            return Err(LocationError::EncodedPchar {
                index: path_offset + index,
            });
        }
    }
    if let Some(index) = path.as_bytes().windows(2).position(|pair| pair == b"//") {
        return Err(LocationError::RepeatedSeparator {
            index: path_offset + index + 1,
        });
    }

    let decoded = decode_percent_bytes(path, path_offset)?;
    validate_decoded_controls(&decoded)?;
    validate_dot_segments(&decoded.value, &decoded.source_offsets)?;
    validate_normalization(&decoded.value, &decoded.source_offsets)?;
    validate_raw_path_characters(&decoded)?;
    Ok(decoded.value)
}

fn validate_decoded_operation_path(path: &str) -> Result<(), LocationError> {
    if let Some((index, _)) = path
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(LocationError::ControlCharacter { index });
    }
    if let Some(index) = path.find('\\') {
        return Err(LocationError::Backslash { index });
    }
    if let Some(index) = path.as_bytes().windows(2).position(|pair| pair == b"//") {
        return Err(LocationError::RepeatedSeparator { index: index + 1 });
    }

    let source_offsets = path
        .char_indices()
        .flat_map(|(index, character)| std::iter::repeat_n(index, character.len_utf8()))
        .collect::<Vec<_>>();
    validate_dot_segments(path, &source_offsets)?;
    validate_normalization(path, &source_offsets)
}

fn decode_percent_bytes(path: &str, path_offset: usize) -> Result<DecodedPath, LocationError> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut source_offsets = Vec::with_capacity(bytes.len());
    let mut encoded_bytes = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            decoded.push(decode_hex(bytes[index + 1], bytes[index + 2]));
            source_offsets.push(path_offset + index);
            encoded_bytes.push(true);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            source_offsets.push(path_offset + index);
            encoded_bytes.push(false);
            index += 1;
        }
    }
    let value = String::from_utf8(decoded).map_err(|error| LocationError::InvalidUtf8 {
        index: source_offsets[error.utf8_error().valid_up_to()],
    })?;
    Ok(DecodedPath {
        value,
        source_offsets,
        encoded_bytes,
    })
}

fn validate_decoded_controls(decoded: &DecodedPath) -> Result<(), LocationError> {
    if let Some((index, _)) = decoded
        .value
        .char_indices()
        .find(|(_, character)| character.is_control())
    {
        return Err(LocationError::ControlCharacter {
            index: decoded.source_offsets[index],
        });
    }
    Ok(())
}

fn validate_dot_segments(path: &str, source_offsets: &[usize]) -> Result<(), LocationError> {
    let mut start = 0;
    for segment in path.split('/') {
        if segment == "." || segment == ".." {
            return Err(LocationError::DotSegment {
                index: source_offsets[start],
            });
        }
        start += segment.len() + 1;
    }
    Ok(())
}

fn validate_normalization(path: &str, source_offsets: &[usize]) -> Result<(), LocationError> {
    if path.is_empty() || vendored_normalize_path(path) == path {
        return Ok(());
    }

    let trimmed_start = path.len() - path.trim_start().len();
    let trimmed_end = path.trim_end().len();
    let decoded_index = if trimmed_start > 0 {
        0
    } else if trimmed_end < path.len() {
        trimmed_end
    } else {
        0
    };
    Err(LocationError::NormalizationAlias {
        index: source_offsets[decoded_index],
    })
}

fn validate_raw_path_characters(decoded: &DecodedPath) -> Result<(), LocationError> {
    for (index, character) in decoded.value.char_indices() {
        if !decoded.encoded_bytes[index]
            && character != '/'
            && !(character.is_ascii() && is_pchar(character as u8))
        {
            return Err(LocationError::NoncanonicalPathCharacter {
                index: decoded.source_offsets[index],
                character,
            });
        }
    }
    Ok(())
}

fn percent_escapes(path: &str) -> impl Iterator<Item = (usize, Option<[u8; 2]>)> + '_ {
    let bytes = path.as_bytes();
    bytes
        .iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'%')
        .map(move |(index, _)| {
            (
                index,
                (index + 2 < bytes.len()).then(|| [bytes[index + 1], bytes[index + 2]]),
            )
        })
}

fn decode_hex(high: u8, low: u8) -> u8 {
    (hex_value(high) << 4) | hex_value(low)
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("hex input was validated"),
    }
}

fn is_pchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        )
}

pub(crate) fn vendored_normalize_path(path: &str) -> String {
    let path = path.trim().trim_start_matches('/');
    if path.is_empty() {
        return "/".to_owned();
    }
    let has_trailing_slash = path.ends_with('/');
    let mut normalized = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if has_trailing_slash {
        normalized.push('/');
    }
    normalized
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn location_roles_enforce_exact_object_and_prefix_shapes() {
        let root = Location::parse("store:/").unwrap();
        let object = Location::parse("store:/archive.typk").unwrap();
        let prefix = Location::parse("store:/packages/").unwrap();

        assert_eq!(root.require_object(), Err(LocationRoleError::ObjectAtRoot));
        assert_eq!(root.require_prefix(), Ok(()));
        assert_eq!(object.require_object(), Ok(()));
        assert_eq!(
            object.require_prefix(),
            Err(LocationRoleError::PrefixMissingTrailingSlash)
        );
        assert_eq!(
            prefix.require_object(),
            Err(LocationRoleError::ObjectHasTrailingSlash)
        );
        assert_eq!(prefix.require_prefix(), Ok(()));
    }

    #[test]
    fn root_projection_composition_and_prefix_confinement_are_byte_exact() {
        let root = Location::parse("store:/").unwrap();
        let prefix = Location::parse("store:/base/").unwrap();

        assert_eq!(root.operation_path(), "");
        assert_eq!(root.dispatch_path(), "/");
        assert_eq!(root.compose("child").unwrap().operation_path(), "child");
        assert_eq!(
            prefix.compose("nested/child").unwrap().operation_path(),
            "base/nested/child"
        );
        assert_eq!(
            prefix.compose("/child"),
            Err(LocationError::RepeatedSeparator { index: 5 })
        );

        assert_eq!(root.relative_file_path("child"), Ok("child"));
        assert_eq!(
            root.relative_file_path("/"),
            Err(PrefixConfinementError::PrefixMarker)
        );
        assert_eq!(prefix.relative_file_path("base/child"), Ok("child"));
        assert_eq!(
            prefix.relative_file_path("base/nested/child"),
            Ok("nested/child")
        );
        assert_eq!(
            prefix.relative_file_path("base/"),
            Err(PrefixConfinementError::PrefixMarker)
        );
        assert_eq!(
            prefix.relative_file_path("base"),
            Err(PrefixConfinementError::OutsidePrefix)
        );
        assert_eq!(
            prefix.relative_file_path("base-sibling/child"),
            Err(PrefixConfinementError::OutsidePrefix)
        );
        assert_eq!(
            prefix.relative_file_path("base2/child"),
            Err(PrefixConfinementError::OutsidePrefix)
        );
        assert_eq!(
            prefix.relative_file_path(""),
            Err(PrefixConfinementError::EmptyPath)
        );
    }

    #[test]
    fn vendored_normalization_preserves_dot_segments_and_non_whitespace_format_chars() {
        for path in [
            "",
            "/",
            "///",
            "abc",
            "abc/",
            "/abc/def",
            "abc///def///",
            " abc/def ",
            "a/./b",
            "a/../b",
            "a\u{feff}",
            "a\u{200b}",
        ] {
            assert_eq!(
                vendored_normalize_path(path),
                ::opendal::raw::normalize_path(path),
                "normalization drift for {path:?}"
            );
        }
        assert_eq!(vendored_normalize_path("a/./b"), "a/./b");
        assert_eq!(vendored_normalize_path("a\u{feff}"), "a\u{feff}");
    }

    proptest! {
        #[test]
        fn vendored_normalization_agrees_with_opendal_for_arbitrary_utf8(path in any::<String>()) {
            prop_assert_eq!(
                vendored_normalize_path(&path),
                ::opendal::raw::normalize_path(&path),
                "OpenDAL normalization drift for {:?}",
                path,
            );
        }
    }
}
