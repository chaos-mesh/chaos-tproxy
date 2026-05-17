// @generated — do NOT edit by hand.
// Re-run `cargo build -p chaos-tproxy-openapi` after editing
// openapi/chaos-tproxy.openapi.yaml.
//
// Only the `types` module from progenitor's output is kept;
// the (empty) HTTP client wrapper is intentionally dropped
// so chaos-tproxy-proxy does not depend on reqwest /
// progenitor_client.

/// Error types.
pub mod error {
    /// Error from a `TryFrom` or `FromStr` implementation.
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(
            &self,
            f: &mut ::std::fmt::Formatter<'_>,
        ) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(
            &self,
            f: &mut ::std::fmt::Formatter<'_>,
        ) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
///`Actions`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "abort": {
///      "description": "If true, terminate the request/response with a 502.",
///      "type": [
///        "boolean",
///        "null"
///      ]
///    },
///    "delay": {
///      "description": "Wait this long before continuing. Format is the `humantime`\ncrate's flavor: `\"100ms\"`, `\"3s\"`, `\"1m30s\"`, etc.\n",
///      "type": [
///        "string",
///        "null"
///      ]
///    },
///    "patch": {
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/PatchAction"
///        }
///      ]
///    },
///    "replace": {
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/ReplaceAction"
///        }
///      ]
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct Actions {
    ///If true, terminate the request/response with a 502.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub abort: ::std::option::Option<bool>,
    /**Wait this long before continuing. Format is the `humantime`
crate's flavor: `"100ms"`, `"3s"`, `"1m30s"`, etc.
*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub delay: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub patch: ::std::option::Option<PatchAction>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub replace: ::std::option::Option<ReplaceAction>,
}
impl ::std::default::Default for Actions {
    fn default() -> Self {
        Self {
            abort: Default::default(),
            delay: Default::default(),
            patch: Default::default(),
            replace: Default::default(),
        }
    }
}
///Top-level config for chaos-tproxy.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Top-level config for chaos-tproxy.",
///  "type": "object",
///  "properties": {
///    "ignore_mark": {
///      "description": "Packet mark to ignore in iptables rules. Currently unused.",
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "minimum": 0.0
///    },
///    "interface": {
///      "description": "Network interface. Currently unused.",
///      "deprecated": true,
///      "type": [
///        "string",
///        "null"
///      ]
///    },
///    "listen_port": {
///      "description": "Proxy listen port. Kept for backwards compatibility; currently unused.",
///      "deprecated": true,
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "maximum": 65535.0,
///      "minimum": 1.0
///    },
///    "proxy_mark": {
///      "description": "`SO_MARK` set on upstream sockets opened by the proxy.\nUsed by the eBPF egress program to skip the proxy's own\nforward traffic and avoid redirect loops.\n",
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "minimum": 0.0
///    },
///    "proxy_ports": {
///      "description": "TCP destination ports the chaos-tproxy will proxy.",
///      "type": [
///        "array",
///        "null"
///      ],
///      "items": {
///        "type": "integer",
///        "format": "int32",
///        "maximum": 65535.0,
///        "minimum": 1.0
///      }
///    },
///    "role": {
///      "description": "If set, rules only fire when the connection's\n(src_ip, dst_ip) matches the configured side.\n",
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/Role"
///        }
///      ]
///    },
///    "route_table": {
///      "description": "Routing table ID. Currently unused.",
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "maximum": 255.0,
///      "minimum": 0.0
///    },
///    "rules": {
///      "description": "Chaos rules, evaluated in declaration order.",
///      "default": [],
///      "type": [
///        "array",
///        "null"
///      ],
///      "items": {
///        "$ref": "#/components/schemas/Rule"
///      }
///    },
///    "safe_mode": {
///      "description": "Deprecated.",
///      "type": [
///        "boolean",
///        "null"
///      ]
///    },
///    "tls": {
///      "description": "TLS configuration for the proxy.",
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/TLSConfig"
///        }
///      ]
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ChaosTproxyConfig {
    ///Packet mark to ignore in iptables rules. Currently unused.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub ignore_mark: ::std::option::Option<i32>,
    ///Network interface. Currently unused.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub interface: ::std::option::Option<::std::string::String>,
    ///Proxy listen port. Kept for backwards compatibility; currently unused.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub listen_port: ::std::option::Option<::std::num::NonZeroU32>,
    /**`SO_MARK` set on upstream sockets opened by the proxy.
Used by the eBPF egress program to skip the proxy's own
forward traffic and avoid redirect loops.
*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub proxy_mark: ::std::option::Option<i32>,
    ///TCP destination ports the chaos-tproxy will proxy.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub proxy_ports: ::std::option::Option<::std::vec::Vec<::std::num::NonZeroU32>>,
    /**If set, rules only fire when the connection's
(src_ip, dst_ip) matches the configured side.
*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub role: ::std::option::Option<Role>,
    ///Routing table ID. Currently unused.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub route_table: ::std::option::Option<i32>,
    ///Chaos rules, evaluated in declaration order.
    #[serde(default = "defaults::chaos_tproxy_config_rules")]
    pub rules: ::std::option::Option<::std::vec::Vec<Rule>>,
    ///Deprecated.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub safe_mode: ::std::option::Option<bool>,
    ///TLS configuration for the proxy.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub tls: ::std::option::Option<TlsConfig>,
}
impl ::std::default::Default for ChaosTproxyConfig {
    fn default() -> Self {
        Self {
            ignore_mark: Default::default(),
            interface: Default::default(),
            listen_port: Default::default(),
            proxy_mark: Default::default(),
            proxy_ports: Default::default(),
            role: Default::default(),
            route_table: Default::default(),
            rules: defaults::chaos_tproxy_config_rules(),
            safe_mode: Default::default(),
            tls: Default::default(),
        }
    }
}
///`PatchAction`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "body": {
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/PatchBody"
///        }
///      ]
///    },
///    "headers": {
///      "description": "Headers to append. Each entry is a `[name, value]` pair;\nduplicate names are allowed for headers like Cookie.\n",
///      "type": [
///        "array",
///        "null"
///      ],
///      "items": {
///        "type": "array",
///        "items": {
///          "type": "string"
///        },
///        "maxItems": 2,
///        "minItems": 2
///      }
///    },
///    "queries": {
///      "description": "Query parameters to append. Each entry is a `[key, value]`\npair; duplicate keys are intentionally allowed (e.g.\n`foo=1&foo=2`), which is why this is an array of pairs\nrather than a map.\n",
///      "type": [
///        "array",
///        "null"
///      ],
///      "items": {
///        "type": "array",
///        "items": {
///          "type": "string"
///        },
///        "maxItems": 2,
///        "minItems": 2
///      }
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PatchAction {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub body: ::std::option::Option<PatchBody>,
    /**Headers to append. Each entry is a `[name, value]` pair;
duplicate names are allowed for headers like Cookie.
*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub headers: ::std::option::Option<::std::vec::Vec<[::std::string::String; 2usize]>>,
    /**Query parameters to append. Each entry is a `[key, value]`
pair; duplicate keys are intentionally allowed (e.g.
`foo=1&foo=2`), which is why this is an array of pairs
rather than a map.
*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub queries: ::std::option::Option<::std::vec::Vec<[::std::string::String; 2usize]>>,
}
impl ::std::default::Default for PatchAction {
    fn default() -> Self {
        Self {
            body: Default::default(),
            headers: Default::default(),
            queries: Default::default(),
        }
    }
}
///`PatchBody`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "contents"
///  ],
///  "properties": {
///    "contents": {
///      "$ref": "#/components/schemas/PatchBodyContents"
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PatchBody {
    pub contents: PatchBodyContents,
}
///`PatchBodyContents`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "oneOf": [
///    {
///      "$ref": "#/components/schemas/PatchBodyContentsJson"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct PatchBodyContents(pub PatchBodyContentsJson);
impl ::std::ops::Deref for PatchBodyContents {
    type Target = PatchBodyContentsJson;
    fn deref(&self) -> &PatchBodyContentsJson {
        &self.0
    }
}
impl ::std::convert::From<PatchBodyContents> for PatchBodyContentsJson {
    fn from(value: PatchBodyContents) -> Self {
        value.0
    }
}
impl ::std::convert::From<PatchBodyContentsJson> for PatchBodyContents {
    fn from(value: PatchBodyContentsJson) -> Self {
        Self(value)
    }
}
///`PatchBodyContentsJson`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "type",
///    "value"
///  ],
///  "properties": {
///    "type": {
///      "type": "string",
///      "enum": [
///        "JSON"
///      ]
///    },
///    "value": {
///      "description": "JSON value (as a string). The proxy parses this and merges\nit into the request/response body using RFC 7396 JSON\nMerge Patch semantics.\n",
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PatchBodyContentsJson {
    #[serde(rename = "type")]
    pub type_: PatchBodyContentsJsonType,
    /**JSON value (as a string). The proxy parses this and merges
it into the request/response body using RFC 7396 JSON
Merge Patch semantics.
*/
    pub value: ::std::string::String,
}
///`PatchBodyContentsJsonType`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "JSON"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum PatchBodyContentsJsonType {
    #[serde(rename = "JSON")]
    Json,
}
impl ::std::fmt::Display for PatchBodyContentsJsonType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Json => f.write_str("JSON"),
        }
    }
}
impl ::std::str::FromStr for PatchBodyContentsJsonType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "JSON" => Ok(Self::Json),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for PatchBodyContentsJsonType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for PatchBodyContentsJsonType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for PatchBodyContentsJsonType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`RawFile`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "oneOf": [
///    {
///      "$ref": "#/components/schemas/RawFilePath"
///    },
///    {
///      "$ref": "#/components/schemas/RawFileContents"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum RawFile {
    Path(RawFilePath),
    Contents(RawFileContents),
}
impl ::std::convert::From<RawFilePath> for RawFile {
    fn from(value: RawFilePath) -> Self {
        Self::Path(value)
    }
}
impl ::std::convert::From<RawFileContents> for RawFile {
    fn from(value: RawFileContents) -> Self {
        Self::Contents(value)
    }
}
///`RawFileContents`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "type",
///    "value"
///  ],
///  "properties": {
///    "type": {
///      "type": "string",
///      "enum": [
///        "Contents"
///      ]
///    },
///    "value": {
///      "description": "Raw file contents (base64-encoded in JSON/YAML).",
///      "type": "string",
///      "format": "byte"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RawFileContents {
    #[serde(rename = "type")]
    pub type_: RawFileContentsType,
    ///Raw file contents (base64-encoded in JSON/YAML).
    pub value: ::std::string::String,
}
///`RawFileContentsType`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "Contents"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum RawFileContentsType {
    Contents,
}
impl ::std::fmt::Display for RawFileContentsType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Contents => f.write_str("Contents"),
        }
    }
}
impl ::std::str::FromStr for RawFileContentsType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Contents" => Ok(Self::Contents),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for RawFileContentsType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for RawFileContentsType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RawFileContentsType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`RawFilePath`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "type",
///    "value"
///  ],
///  "properties": {
///    "type": {
///      "type": "string",
///      "enum": [
///        "Path"
///      ]
///    },
///    "value": {
///      "description": "Absolute or relative path to the file on disk.",
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RawFilePath {
    #[serde(rename = "type")]
    pub type_: RawFilePathType,
    ///Absolute or relative path to the file on disk.
    pub value: ::std::string::String,
}
///`RawFilePathType`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "Path"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum RawFilePathType {
    Path,
}
impl ::std::fmt::Display for RawFilePathType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Path => f.write_str("Path"),
        }
    }
}
impl ::std::str::FromStr for RawFilePathType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Path" => Ok(Self::Path),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for RawFilePathType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for RawFilePathType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for RawFilePathType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`ReplaceAction`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "body": {
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/ReplaceBody"
///        }
///      ]
///    },
///    "code": {
///      "description": "Replace response status code. Ignored on Request rules.",
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "maximum": 599.0,
///      "minimum": 100.0
///    },
///    "headers": {
///      "description": "Replace (set) request/response header values.",
///      "type": [
///        "object",
///        "null"
///      ],
///      "additionalProperties": {
///        "type": "string"
///      }
///    },
///    "method": {
///      "description": "Replace request method. Ignored on Response rules.",
///      "type": [
///        "string",
///        "null"
///      ]
///    },
///    "path": {
///      "description": "Replace request path. Ignored on Response rules.",
///      "type": [
///        "string",
///        "null"
///      ]
///    },
///    "queries": {
///      "description": "Replace request query parameters with this map. Ignored on Response rules.",
///      "type": [
///        "object",
///        "null"
///      ],
///      "additionalProperties": {
///        "type": "string"
///      }
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ReplaceAction {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub body: ::std::option::Option<ReplaceBody>,
    ///Replace response status code. Ignored on Request rules.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub code: ::std::option::Option<i32>,
    ///Replace (set) request/response header values.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub headers: ::std::option::Option<
        ::std::collections::HashMap<::std::string::String, ::std::string::String>,
    >,
    ///Replace request method. Ignored on Response rules.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub method: ::std::option::Option<::std::string::String>,
    ///Replace request path. Ignored on Response rules.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub path: ::std::option::Option<::std::string::String>,
    ///Replace request query parameters with this map. Ignored on Response rules.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub queries: ::std::option::Option<
        ::std::collections::HashMap<::std::string::String, ::std::string::String>,
    >,
}
impl ::std::default::Default for ReplaceAction {
    fn default() -> Self {
        Self {
            body: Default::default(),
            code: Default::default(),
            headers: Default::default(),
            method: Default::default(),
            path: Default::default(),
            queries: Default::default(),
        }
    }
}
///`ReplaceBody`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "contents"
///  ],
///  "properties": {
///    "contents": {
///      "$ref": "#/components/schemas/ReplaceBodyContents"
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ReplaceBody {
    pub contents: ReplaceBodyContents,
}
///`ReplaceBodyContents`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "oneOf": [
///    {
///      "$ref": "#/components/schemas/ReplaceBodyContentsText"
///    },
///    {
///      "$ref": "#/components/schemas/ReplaceBodyContentsBase64"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ReplaceBodyContents {
    Text(ReplaceBodyContentsText),
    Base64(ReplaceBodyContentsBase64),
}
impl ::std::convert::From<ReplaceBodyContentsText> for ReplaceBodyContents {
    fn from(value: ReplaceBodyContentsText) -> Self {
        Self::Text(value)
    }
}
impl ::std::convert::From<ReplaceBodyContentsBase64> for ReplaceBodyContents {
    fn from(value: ReplaceBodyContentsBase64) -> Self {
        Self::Base64(value)
    }
}
///`ReplaceBodyContentsBase64`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "type",
///    "value"
///  ],
///  "properties": {
///    "type": {
///      "type": "string",
///      "enum": [
///        "BASE64"
///      ]
///    },
///    "value": {
///      "description": "Base64-encoded replacement body (decoded by the proxy at apply time).",
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ReplaceBodyContentsBase64 {
    #[serde(rename = "type")]
    pub type_: ReplaceBodyContentsBase64Type,
    ///Base64-encoded replacement body (decoded by the proxy at apply time).
    pub value: ::std::string::String,
}
///`ReplaceBodyContentsBase64Type`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "BASE64"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum ReplaceBodyContentsBase64Type {
    #[serde(rename = "BASE64")]
    Base64,
}
impl ::std::fmt::Display for ReplaceBodyContentsBase64Type {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Base64 => f.write_str("BASE64"),
        }
    }
}
impl ::std::str::FromStr for ReplaceBodyContentsBase64Type {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "BASE64" => Ok(Self::Base64),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ReplaceBodyContentsBase64Type {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ReplaceBodyContentsBase64Type {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ReplaceBodyContentsBase64Type {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`ReplaceBodyContentsText`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "type",
///    "value"
///  ],
///  "properties": {
///    "type": {
///      "type": "string",
///      "enum": [
///        "TEXT"
///      ]
///    },
///    "value": {
///      "description": "Literal replacement body.",
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ReplaceBodyContentsText {
    #[serde(rename = "type")]
    pub type_: ReplaceBodyContentsTextType,
    ///Literal replacement body.
    pub value: ::std::string::String,
}
///`ReplaceBodyContentsTextType`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "TEXT"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum ReplaceBodyContentsTextType {
    #[serde(rename = "TEXT")]
    Text,
}
impl ::std::fmt::Display for ReplaceBodyContentsTextType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Text => f.write_str("TEXT"),
        }
    }
}
impl ::std::str::FromStr for ReplaceBodyContentsTextType {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "TEXT" => Ok(Self::Text),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ReplaceBodyContentsTextType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ReplaceBodyContentsTextType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ReplaceBodyContentsTextType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`Role`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "oneOf": [
///    {
///      "$ref": "#/components/schemas/RoleClient"
///    },
///    {
///      "$ref": "#/components/schemas/RoleServer"
///    }
///  ]
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Role {
    Client(RoleClient),
    Server(RoleServer),
}
impl ::std::convert::From<RoleClient> for Role {
    fn from(value: RoleClient) -> Self {
        Self::Client(value)
    }
}
impl ::std::convert::From<RoleServer> for Role {
    fn from(value: RoleServer) -> Self {
        Self::Server(value)
    }
}
///`RoleClient`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "Client"
///  ],
///  "properties": {
///    "Client": {
///      "description": "Rule fires when the connection's source ip is in this list.",
///      "type": "array",
///      "items": {
///        "type": "string",
///        "format": "ipv4"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoleClient {
    ///Rule fires when the connection's source ip is in this list.
    #[serde(rename = "Client")]
    pub client: ::std::vec::Vec<::std::net::Ipv4Addr>,
}
///`RoleServer`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "Server"
///  ],
///  "properties": {
///    "Server": {
///      "description": "Rule fires when the connection's destination ip is in this list.",
///      "type": "array",
///      "items": {
///        "type": "string",
///        "format": "ipv4"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoleServer {
    ///Rule fires when the connection's destination ip is in this list.
    #[serde(rename = "Server")]
    pub server: ::std::vec::Vec<::std::net::Ipv4Addr>,
}
///`Rule`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "actions",
///    "selector",
///    "target"
///  ],
///  "properties": {
///    "actions": {
///      "$ref": "#/components/schemas/Actions"
///    },
///    "selector": {
///      "$ref": "#/components/schemas/Selector"
///    },
///    "target": {
///      "$ref": "#/components/schemas/Target"
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct Rule {
    pub actions: Actions,
    pub selector: Selector,
    pub target: Target,
}
///`Selector`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "properties": {
///    "code": {
///      "description": "Match only when the response status equals this. Ignored on Request rules.",
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "maximum": 599.0,
///      "minimum": 100.0
///    },
///    "method": {
///      "description": "HTTP method (GET / POST / …).",
///      "type": [
///        "string",
///        "null"
///      ]
///    },
///    "path": {
///      "description": "Wildcard glob against the request URI path. Syntax is the\n`wildmatch` crate's flavor: `?` matches one char, `*`\nmatches zero-or-more chars. Case-sensitive.\n",
///      "type": [
///        "string",
///        "null"
///      ]
///    },
///    "port": {
///      "description": "Match only when the original-dst port equals this.",
///      "type": [
///        "integer",
///        "null"
///      ],
///      "format": "int32",
///      "maximum": 65535.0,
///      "minimum": 1.0
///    },
///    "request_headers": {
///      "description": "Match only when every (name, value) pair appears in the request headers.",
///      "type": [
///        "object",
///        "null"
///      ],
///      "additionalProperties": {
///        "type": "string"
///      }
///    },
///    "response_headers": {
///      "description": "Match only when every (name, value) pair appears in the response headers. Ignored on Request rules.",
///      "type": [
///        "object",
///        "null"
///      ],
///      "additionalProperties": {
///        "type": "string"
///      }
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct Selector {
    ///Match only when the response status equals this. Ignored on Request rules.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub code: ::std::option::Option<i32>,
    ///HTTP method (GET / POST / …).
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub method: ::std::option::Option<::std::string::String>,
    /**Wildcard glob against the request URI path. Syntax is the
`wildmatch` crate's flavor: `?` matches one char, `*`
matches zero-or-more chars. Case-sensitive.
*/
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub path: ::std::option::Option<::std::string::String>,
    ///Match only when the original-dst port equals this.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub port: ::std::option::Option<::std::num::NonZeroU32>,
    ///Match only when every (name, value) pair appears in the request headers.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub request_headers: ::std::option::Option<
        ::std::collections::HashMap<::std::string::String, ::std::string::String>,
    >,
    ///Match only when every (name, value) pair appears in the response headers. Ignored on Request rules.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub response_headers: ::std::option::Option<
        ::std::collections::HashMap<::std::string::String, ::std::string::String>,
    >,
}
impl ::std::default::Default for Selector {
    fn default() -> Self {
        Self {
            code: Default::default(),
            method: Default::default(),
            path: Default::default(),
            port: Default::default(),
            request_headers: Default::default(),
            response_headers: Default::default(),
        }
    }
}
///Whether this rule matches against the HTTP request or the response.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Whether this rule matches against the HTTP request or the response.",
///  "type": "string",
///  "enum": [
///    "Request",
///    "Response"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub enum Target {
    Request,
    Response,
}
impl ::std::fmt::Display for Target {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Request => f.write_str("Request"),
            Self::Response => f.write_str("Response"),
        }
    }
}
impl ::std::str::FromStr for Target {
    type Err = self::error::ConversionError;
    fn from_str(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "Request" => Ok(Self::Request),
            "Response" => Ok(Self::Response),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for Target {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &str,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for Target {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for Target {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`TlsConfig`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "cert_file",
///    "key_file"
///  ],
///  "properties": {
///    "ca_file": {
///      "description": "CA certificate. If absent, system roots are used.",
///      "type": [
///        "object",
///        "null"
///      ],
///      "allOf": [
///        {
///          "$ref": "#/components/schemas/RawFile"
///        }
///      ]
///    },
///    "cert_file": {
///      "$ref": "#/components/schemas/RawFile"
///    },
///    "key_file": {
///      "$ref": "#/components/schemas/RawFile"
///    }
///  }
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct TlsConfig {
    ///CA certificate. If absent, system roots are used.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub ca_file: ::std::option::Option<RawFile>,
    pub cert_file: RawFile,
    pub key_file: RawFile,
}
/// Generation of default values for serde.
pub mod defaults {
    pub(super) fn chaos_tproxy_config_rules() -> ::std::option::Option<
        ::std::vec::Vec<super::Rule>,
    > {
        ::std::option::Option::Some(vec![])
    }
}
