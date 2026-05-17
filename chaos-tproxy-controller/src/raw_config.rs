// Type aliases / re-exports so the rest of the controller can use familiar names.
// Note: Rule/Selector/Target/Actions keep their openapi names to avoid clashing
// with identically-named types in chaos-tproxy-proxy.
pub use chaos_tproxy_openapi::{
    Actions, ChaosTproxyConfig as RawConfig, PatchAction, PatchBody, PatchBodyContents,
    PatchBodyContentsJson, RawFile, RawFileContents, RawFilePath, ReplaceAction, ReplaceBody,
    ReplaceBodyContents, ReplaceBodyContentsBase64, ReplaceBodyContentsText, Role, RoleClient,
    RoleServer, Rule, Selector, Target, TlsConfig as TLSRawConfig,
};
