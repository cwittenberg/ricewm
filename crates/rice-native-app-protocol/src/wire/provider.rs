use super::{
    errors::{DecodeError, EncodeError},
    primitives::{Reader, Writer},
};
use crate::{
    APPLICATION_CATALOG_VERSION, DataProviderOperation, DataProviderRequest, DataProviderResponse,
    DataProviderResult, DocumentOutlineEntry, DocumentSearchHit, HttpHeader, InstalledApplication,
    MAX_APPLICATION_ID_BYTES, MAX_APPLICATIONS, MAX_AUTH_INTERACTION_BYTES,
    MAX_AUTH_PROVIDER_ID_BYTES, MAX_DOCUMENT_OUTLINE_ENTRIES, MAX_DOCUMENT_PAGES,
    MAX_DOCUMENT_SEARCH_RESULTS, MAX_ENTROPY_BYTES, MAX_HTTP_BODY_BYTES,
    MAX_HTTP_HEADER_BLOCK_BYTES, MAX_HTTP_HEADER_NAME_BYTES, MAX_HTTP_HEADER_VALUE_BYTES,
    MAX_HTTP_HEADERS, MAX_HTTP_METADATA_BYTES, MAX_HTTP_REQUEST_BODY_BYTES,
    MAX_HTTP_TIMEOUT_MILLIS, MAX_MIME_TYPES, MAX_MOUNTS, MAX_PATH_BYTES, MAX_TEXT_BYTES,
    MountCapabilities, MountEntry, MountState,
    application::{
        validate_application_id, validate_catalog, validate_guest_paths, validate_mime_type,
    },
};

pub(super) fn encode_request(
    writer: &mut Writer,
    request: &DataProviderRequest,
) -> Result<(), EncodeError> {
    writer.id(request.request);
    writer.id(request.app);
    match &request.operation {
        DataProviderOperation::Entropy { byte_len } => {
            validate_entropy_length(*byte_len).map_err(EncodeError::InvalidValue)?;
            writer.u8(0);
            writer.u16(*byte_len);
        }
        DataProviderOperation::CurrencyRate {
            base_iso4217,
            quote_iso4217,
        } => {
            validate_iso(base_iso4217).map_err(EncodeError::InvalidValue)?;
            validate_iso(quote_iso4217).map_err(EncodeError::InvalidValue)?;
            writer.u8(1);
            writer.string(base_iso4217, 3, "base_iso4217")?;
            writer.string(quote_iso4217, 3, "quote_iso4217")?;
        }
        DataProviderOperation::MountCloudProviders => writer.u8(21),
        DataProviderOperation::MountConnectCloud { provider_id, account_id } => {
            validate_nonempty(provider_id).map_err(EncodeError::InvalidValue)?;
            writer.u8(22);
            writer.string(provider_id, 48, "cloud provider id")?;
            writer.string(account_id, 512, "cloud account id")?;
        }
        DataProviderOperation::MountConnectCloudConfigured { provider_id, account_id, config_json } => {
            validate_nonempty(provider_id).map_err(EncodeError::InvalidValue)?;
            writer.u8(23);
            writer.string(provider_id, 48, "cloud provider id")?;
            writer.string(account_id, 512, "cloud account id")?;
            writer.string(config_json, MAX_TEXT_BYTES, "cloud connection settings")?;
        }
        DataProviderOperation::MountList => writer.u8(2),
        DataProviderOperation::MountConnectLocal => writer.u8(3),
        DataProviderOperation::MountSync { mount_id } => {
            validate_nonempty(mount_id).map_err(EncodeError::InvalidValue)?;
            writer.u8(4);
            writer.string(mount_id, MAX_TEXT_BYTES, "mount id")?;
        }
        DataProviderOperation::MountDisconnect { mount_id } => {
            validate_nonempty(mount_id).map_err(EncodeError::InvalidValue)?;
            writer.u8(5);
            writer.string(mount_id, MAX_TEXT_BYTES, "mount id")?;
        }
        DataProviderOperation::MountReconnect { mount_id } => {
            validate_nonempty(mount_id).map_err(EncodeError::InvalidValue)?;
            writer.u8(6);
            writer.string(mount_id, MAX_TEXT_BYTES, "mount id")?;
        }
        DataProviderOperation::HttpGet { url } => {
            validate_http_url(url).map_err(EncodeError::InvalidValue)?;
            writer.u8(7);
            writer.string(url, MAX_TEXT_BYTES, "HTTP URL")?;
        }
        DataProviderOperation::HttpRequest {
            url,
            method,
            content_type,
            cookie,
            body,
        } => {
            validate_http_url(url).map_err(EncodeError::InvalidValue)?;
            if !matches!(method.as_str(), "GET" | "POST") {
                return Err(EncodeError::InvalidValue("HTTP method"));
            }
            writer.u8(12);
            writer.string(url, MAX_TEXT_BYTES, "HTTP URL")?;
            writer.string(method, 8, "HTTP method")?;
            writer.string(content_type, MAX_TEXT_BYTES, "HTTP content type")?;
            writer.string(cookie, MAX_TEXT_BYTES, "HTTP cookie")?;
            writer.bytes(body, MAX_HTTP_REQUEST_BODY_BYTES, "HTTP request body")?;
        }
        DataProviderOperation::HttpRequestV2 {
            url,
            method,
            headers,
            body,
            timeout_millis,
            response_body_limit,
        } => {
            validate_http_url(url).map_err(EncodeError::InvalidValue)?;
            validate_http_method(method).map_err(EncodeError::InvalidValue)?;
            validate_http_headers(headers).map_err(EncodeError::InvalidValue)?;
            validate_http_limits(*timeout_millis, *response_body_limit)
                .map_err(EncodeError::InvalidValue)?;
            writer.u8(13);
            writer.string(url, MAX_TEXT_BYTES, "HTTP URL")?;
            writer.string(method, 8, "HTTP method")?;
            writer.count(headers.len(), MAX_HTTP_HEADERS, "HTTP headers")?;
            for header in headers {
                writer.string(&header.name, MAX_HTTP_HEADER_NAME_BYTES, "HTTP header name")?;
                writer.string(
                    &header.value,
                    MAX_HTTP_HEADER_VALUE_BYTES,
                    "HTTP header value",
                )?;
            }
            writer.bytes(body, MAX_HTTP_REQUEST_BODY_BYTES, "HTTP request body")?;
            writer.u32(*timeout_millis);
            writer.u32(*response_body_limit);
        }
        DataProviderOperation::HttpRequestV3 {
            url,
            method,
            headers,
            cookie,
            body,
        } => {
            validate_http_url(url).map_err(EncodeError::InvalidValue)?;
            if !matches!(method.as_str(), "GET" | "POST") {
                return Err(EncodeError::InvalidValue("HTTP method"));
            }
            validate_http_headers(headers).map_err(EncodeError::InvalidValue)?;
            writer.u8(16);
            writer.string(url, MAX_TEXT_BYTES, "HTTP URL")?;
            writer.string(method, 8, "HTTP method")?;
            writer.count(headers.len(), MAX_HTTP_HEADERS, "HTTP headers")?;
            for header in headers {
                writer.string(&header.name, MAX_HTTP_HEADER_NAME_BYTES, "HTTP header name")?;
                writer.string(
                    &header.value,
                    MAX_HTTP_HEADER_VALUE_BYTES,
                    "HTTP header value",
                )?;
            }
            writer.string(cookie, MAX_TEXT_BYTES, "HTTP cookie")?;
            writer.bytes(body, MAX_HTTP_REQUEST_BODY_BYTES, "HTTP request body")?;
        }
        DataProviderOperation::Authorization {
            application_id,
            provider_id,
            entry_url,
            callback_mode,
            callback_url,
            interaction_kind,
            interaction_value,
        } => {
            validate_auth_application_id(application_id).map_err(EncodeError::InvalidValue)?;
            validate_auth_provider_id(provider_id).map_err(EncodeError::InvalidValue)?;
            validate_http_url(entry_url).map_err(EncodeError::InvalidValue)?;
            validate_auth_callback(callback_mode, callback_url)
                .map_err(EncodeError::InvalidValue)?;
            validate_auth_interaction(interaction_kind, interaction_value)
                .map_err(EncodeError::InvalidValue)?;
            writer.u8(17);
            writer.string(
                application_id,
                MAX_APPLICATION_ID_BYTES,
                "auth application id",
            )?;
            writer.string(provider_id, MAX_AUTH_PROVIDER_ID_BYTES, "auth provider id")?;
            writer.string(entry_url, MAX_TEXT_BYTES, "auth entry URL")?;
            writer.string(callback_mode, 32, "auth callback mode")?;
            writer.string(callback_url, MAX_TEXT_BYTES, "auth callback URL")?;
            writer.string(interaction_kind, 32, "auth interaction kind")?;
            writer.string(
                interaction_value,
                MAX_AUTH_INTERACTION_BYTES,
                "auth interaction value",
            )?;
        }
        DataProviderOperation::ApplicationCatalog { mime_type } => {
            validate_mime_type(mime_type).map_err(EncodeError::InvalidValue)?;
            writer.u8(8);
            writer.string(mime_type, MAX_TEXT_BYTES, "application MIME type")?;
        }
        DataProviderOperation::ApplicationLaunch {
            application_id,
            mime_type,
            paths,
            remember_default,
        } => {
            validate_application_id(application_id).map_err(EncodeError::InvalidValue)?;
            validate_mime_type(mime_type).map_err(EncodeError::InvalidValue)?;
            validate_guest_paths(paths).map_err(EncodeError::InvalidValue)?;
            writer.u8(9);
            writer.string(application_id, MAX_APPLICATION_ID_BYTES, "application id")?;
            writer.string(mime_type, MAX_TEXT_BYTES, "application MIME type")?;
            writer.count(paths.len(), crate::MAX_ARGUMENTS, "application paths")?;
            for path in paths {
                writer.string(path, MAX_PATH_BYTES, "application path")?;
            }
            writer.bool(*remember_default);
        }
        DataProviderOperation::MountConnectWebDav {
            url,
            username,
            password,
        } => {
            validate_webdav_url(url).map_err(EncodeError::InvalidValue)?;
            validate_credential(username).map_err(EncodeError::InvalidValue)?;
            validate_credential(password).map_err(EncodeError::InvalidValue)?;
            writer.u8(10);
            writer.string(url, MAX_TEXT_BYTES, "WebDAV URL")?;
            writer.string(username, MAX_TEXT_BYTES, "WebDAV username")?;
            writer.string(password, MAX_TEXT_BYTES, "WebDAV password")?;
        }
        DataProviderOperation::MountConnectNetwork { url, username, password, domain, bridge_url, bridge_token, uid, gid } => {
            validate_network_url(url).map_err(EncodeError::InvalidValue)?;
            writer.u8(18);
            for value in [url, username, password, domain, bridge_url, bridge_token] {
                validate_credential(value).map_err(EncodeError::InvalidValue)?;
                writer.string(value, MAX_TEXT_BYTES, "network connection")?;
            }
            writer.u32(*uid);
            writer.u32(*gid);
        }
        DataProviderOperation::MountDescribeNetwork { mount_id } | DataProviderOperation::MountForgetNetwork { mount_id } => {
            writer.u8(if matches!(&request.operation, DataProviderOperation::MountDescribeNetwork { .. }) {19} else {20});
            writer.string(mount_id, MAX_TEXT_BYTES, "mount id")?;
        }
        DataProviderOperation::MountCancelConnect { connect_request } => {
            writer.u8(11);
            writer.id(*connect_request);
        }
        DataProviderOperation::DocumentInfo { path } => {
            validate_document_path(path).map_err(EncodeError::InvalidValue)?;
            writer.u8(14);
            writer.string(path, MAX_PATH_BYTES, "document path")?;
        }
        DataProviderOperation::DocumentSearch {
            path,
            query,
            maximum_results,
        } => {
            validate_document_path(path).map_err(EncodeError::InvalidValue)?;
            validate_document_query(query).map_err(EncodeError::InvalidValue)?;
            if *maximum_results == 0 || usize::from(*maximum_results) > MAX_DOCUMENT_SEARCH_RESULTS
            {
                return Err(EncodeError::InvalidValue("document search result limit"));
            }
            writer.u8(15);
            writer.string(path, MAX_PATH_BYTES, "document path")?;
            writer.string(query, MAX_TEXT_BYTES, "document search query")?;
            writer.u16(*maximum_results);
        }
    }
    Ok(())
}

pub(super) fn decode_request(reader: &mut Reader<'_>) -> Result<DataProviderRequest, DecodeError> {
    let request = reader.id("request")?;
    let app = reader.id("app")?;
    let operation = match reader.u8()? {
        0 => {
            let byte_len = reader.u16()?;
            validate_entropy_length(byte_len).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::Entropy { byte_len }
        }
        1 => {
            let base_iso4217 = reader.string(3, "base_iso4217")?;
            let quote_iso4217 = reader.string(3, "quote_iso4217")?;
            validate_iso(&base_iso4217).map_err(DecodeError::InvalidValue)?;
            validate_iso(&quote_iso4217).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::CurrencyRate {
                base_iso4217,
                quote_iso4217,
            }
        }
        2 => DataProviderOperation::MountList,
        3 => DataProviderOperation::MountConnectLocal,
        4 => DataProviderOperation::MountSync {
            mount_id: nonempty_string(reader, MAX_TEXT_BYTES, "mount id")?,
        },
        5 => DataProviderOperation::MountDisconnect {
            mount_id: nonempty_string(reader, MAX_TEXT_BYTES, "mount id")?,
        },
        6 => DataProviderOperation::MountReconnect {
            mount_id: nonempty_string(reader, MAX_TEXT_BYTES, "mount id")?,
        },
        7 => {
            let url = nonempty_string(reader, MAX_TEXT_BYTES, "HTTP URL")?;
            validate_http_url(&url).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::HttpGet { url }
        }
        8 => {
            let mime_type = nonempty_string(reader, MAX_TEXT_BYTES, "application MIME type")?;
            validate_mime_type(&mime_type).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::ApplicationCatalog { mime_type }
        }
        9 => {
            let application_id =
                nonempty_string(reader, MAX_APPLICATION_ID_BYTES, "application id")?;
            validate_application_id(&application_id).map_err(DecodeError::InvalidValue)?;
            let mime_type = nonempty_string(reader, MAX_TEXT_BYTES, "application MIME type")?;
            validate_mime_type(&mime_type).map_err(DecodeError::InvalidValue)?;
            let count = reader.count(crate::MAX_ARGUMENTS, "application paths")?;
            let mut paths = Vec::with_capacity(count);
            for _ in 0..count {
                paths.push(nonempty_string(reader, MAX_PATH_BYTES, "application path")?);
            }
            validate_guest_paths(&paths).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::ApplicationLaunch {
                application_id,
                mime_type,
                paths,
                remember_default: reader.bool("remember application default")?,
            }
        }
        10 => {
            let url = nonempty_string(reader, MAX_TEXT_BYTES, "WebDAV URL")?;
            let username = reader.string(MAX_TEXT_BYTES, "WebDAV username")?;
            let password = reader.string(MAX_TEXT_BYTES, "WebDAV password")?;
            validate_webdav_url(&url).map_err(DecodeError::InvalidValue)?;
            validate_credential(&username).map_err(DecodeError::InvalidValue)?;
            validate_credential(&password).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::MountConnectWebDav {
                url,
                username,
                password,
            }
        }
        18 => {
            let url = reader.string(MAX_TEXT_BYTES, "network URL")?;
            let username = reader.string(MAX_TEXT_BYTES, "network username")?;
            let password = reader.string(MAX_TEXT_BYTES, "network password")?;
            let domain = reader.string(MAX_TEXT_BYTES, "network domain")?;
            let bridge_url = reader.string(MAX_TEXT_BYTES, "network bridge URL")?;
            let bridge_token = reader.string(MAX_TEXT_BYTES, "network bridge token")?;
            validate_network_url(&url).map_err(DecodeError::InvalidValue)?;
            for value in [&username, &password, &domain, &bridge_url, &bridge_token] {
                validate_credential(value).map_err(DecodeError::InvalidValue)?;
            }
            DataProviderOperation::MountConnectNetwork { url, username, password, domain, bridge_url, bridge_token, uid: reader.u32()?, gid: reader.u32()? }
        }
        21 => DataProviderOperation::MountCloudProviders,
        22 => DataProviderOperation::MountConnectCloud {
            provider_id: nonempty_string(reader, 48, "cloud provider id")?,
            account_id: reader.string(512, "cloud account id")?,
        },
        23 => DataProviderOperation::MountConnectCloudConfigured {
            provider_id:reader.string(48,"cloud provider id")?, account_id:reader.string(512,"cloud account id")?, config_json:reader.string(MAX_TEXT_BYTES,"cloud connection settings")?,
        },
        19 => DataProviderOperation::MountDescribeNetwork {mount_id:reader.string(MAX_TEXT_BYTES,"mount id")?},
        20 => DataProviderOperation::MountForgetNetwork {mount_id:reader.string(MAX_TEXT_BYTES,"mount id")?},
        11 => DataProviderOperation::MountCancelConnect {
            connect_request: reader.id("WebDAV connect request")?,
        },
        12 => {
            let url = nonempty_string(reader, MAX_TEXT_BYTES, "HTTP URL")?;
            validate_http_url(&url).map_err(DecodeError::InvalidValue)?;
            let method = nonempty_string(reader, 8, "HTTP method")?;
            if !matches!(method.as_str(), "GET" | "POST") {
                return Err(DecodeError::InvalidValue("HTTP method"));
            }
            DataProviderOperation::HttpRequest {
                url,
                method,
                content_type: reader.string(MAX_TEXT_BYTES, "HTTP content type")?,
                cookie: reader.string(MAX_TEXT_BYTES, "HTTP cookie")?,
                body: reader.bytes(MAX_HTTP_REQUEST_BODY_BYTES, "HTTP request body")?,
            }
        }
        13 => {
            let url = nonempty_string(reader, MAX_TEXT_BYTES, "HTTP URL")?;
            validate_http_url(&url).map_err(DecodeError::InvalidValue)?;
            let method = nonempty_string(reader, 8, "HTTP method")?;
            validate_http_method(&method).map_err(DecodeError::InvalidValue)?;
            let count = reader.count(MAX_HTTP_HEADERS, "HTTP headers")?;
            let mut headers = Vec::with_capacity(count);
            for _ in 0..count {
                headers.push(HttpHeader {
                    name: nonempty_string(reader, MAX_HTTP_HEADER_NAME_BYTES, "HTTP header name")?,
                    value: reader.string(MAX_HTTP_HEADER_VALUE_BYTES, "HTTP header value")?,
                });
            }
            validate_http_headers(&headers).map_err(DecodeError::InvalidValue)?;
            let body = reader.bytes(MAX_HTTP_REQUEST_BODY_BYTES, "HTTP request body")?;
            let timeout_millis = reader.u32()?;
            let response_body_limit = reader.u32()?;
            validate_http_limits(timeout_millis, response_body_limit)
                .map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::HttpRequestV2 {
                url,
                method,
                headers,
                body,
                timeout_millis,
                response_body_limit,
            }
        }
        14 => {
            let path = nonempty_string(reader, MAX_PATH_BYTES, "document path")?;
            validate_document_path(&path).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::DocumentInfo { path }
        }
        15 => {
            let path = nonempty_string(reader, MAX_PATH_BYTES, "document path")?;
            validate_document_path(&path).map_err(DecodeError::InvalidValue)?;
            let query = nonempty_string(reader, MAX_TEXT_BYTES, "document search query")?;
            validate_document_query(&query).map_err(DecodeError::InvalidValue)?;
            let maximum_results = reader.u16()?;
            if maximum_results == 0 || usize::from(maximum_results) > MAX_DOCUMENT_SEARCH_RESULTS {
                return Err(DecodeError::InvalidValue("document search result limit"));
            }
            DataProviderOperation::DocumentSearch {
                path,
                query,
                maximum_results,
            }
        }
        16 => {
            let url = nonempty_string(reader, MAX_TEXT_BYTES, "HTTP URL")?;
            validate_http_url(&url).map_err(DecodeError::InvalidValue)?;
            let method = nonempty_string(reader, 8, "HTTP method")?;
            if !matches!(method.as_str(), "GET" | "POST") {
                return Err(DecodeError::InvalidValue("HTTP method"));
            }
            let count = reader.count(MAX_HTTP_HEADERS, "HTTP headers")?;
            let mut headers = Vec::with_capacity(count);
            for _ in 0..count {
                headers.push(HttpHeader {
                    name: nonempty_string(reader, MAX_HTTP_HEADER_NAME_BYTES, "HTTP header name")?,
                    value: reader.string(MAX_HTTP_HEADER_VALUE_BYTES, "HTTP header value")?,
                });
            }
            validate_http_headers(&headers).map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::HttpRequestV3 {
                url,
                method,
                headers,
                cookie: reader.string(MAX_TEXT_BYTES, "HTTP cookie")?,
                body: reader.bytes(MAX_HTTP_REQUEST_BODY_BYTES, "HTTP request body")?,
            }
        }
        17 => {
            let application_id =
                nonempty_string(reader, MAX_APPLICATION_ID_BYTES, "auth application id")?;
            let provider_id =
                nonempty_string(reader, MAX_AUTH_PROVIDER_ID_BYTES, "auth provider id")?;
            let entry_url = nonempty_string(reader, MAX_TEXT_BYTES, "auth entry URL")?;
            let callback_mode = nonempty_string(reader, 32, "auth callback mode")?;
            let callback_url = nonempty_string(reader, MAX_TEXT_BYTES, "auth callback URL")?;
            let interaction_kind = reader.string(32, "auth interaction kind")?;
            let interaction_value =
                reader.string(MAX_AUTH_INTERACTION_BYTES, "auth interaction value")?;
            validate_auth_application_id(&application_id).map_err(DecodeError::InvalidValue)?;
            validate_auth_provider_id(&provider_id).map_err(DecodeError::InvalidValue)?;
            validate_http_url(&entry_url).map_err(DecodeError::InvalidValue)?;
            validate_auth_callback(&callback_mode, &callback_url)
                .map_err(DecodeError::InvalidValue)?;
            validate_auth_interaction(&interaction_kind, &interaction_value)
                .map_err(DecodeError::InvalidValue)?;
            DataProviderOperation::Authorization {
                application_id,
                provider_id,
                entry_url,
                callback_mode,
                callback_url,
                interaction_kind,
                interaction_value,
            }
        }
        _ => return Err(DecodeError::InvalidValue("data provider operation")),
    };
    Ok(DataProviderRequest {
        request,
        app,
        operation,
    })
}

pub(super) fn encode_response(
    writer: &mut Writer,
    response: &DataProviderResponse,
) -> Result<(), EncodeError> {
    writer.id(response.request);
    match &response.result {
        DataProviderResult::Entropy(bytes) => {
            if bytes.is_empty() || bytes.len() > MAX_ENTROPY_BYTES {
                return Err(EncodeError::InvalidValue("entropy length"));
            }
            writer.u8(0);
            writer.bytes(bytes, MAX_ENTROPY_BYTES, "entropy")?;
        }
        DataProviderResult::CurrencyRate {
            decimal_rate,
            observed_millis,
            source,
        } => {
            validate_positive_decimal(decimal_rate).map_err(EncodeError::InvalidValue)?;
            validate_nonempty(source).map_err(EncodeError::InvalidValue)?;
            writer.u8(1);
            writer.string(decimal_rate, MAX_TEXT_BYTES, "decimal_rate")?;
            writer.u64(*observed_millis);
            writer.string(source, MAX_TEXT_BYTES, "rate source")?;
        }
        DataProviderResult::Mounts {
            entries,
            can_connect_local,
        } => {
            validate_mounts(entries).map_err(EncodeError::InvalidValue)?;
            let icons = entries.iter().any(|entry| entry.icon.is_some());
            writer.u8(if icons { 13 } else { 3 });
            writer.bool(*can_connect_local);
            writer.count(entries.len(), MAX_MOUNTS, "mounts")?;
            for entry in entries {
                writer.string(&entry.id, MAX_TEXT_BYTES, "mount id")?;
                writer.string(&entry.label, MAX_TEXT_BYTES, "mount label")?;
                writer.string(&entry.path, MAX_PATH_BYTES, "mount path")?;
                writer.u8(entry.state as u8);
                writer.u8(entry.capabilities.bits());
                if icons { writer.u16(entry.icon.unwrap_or(0)); }
            }
        }
        DataProviderResult::HttpResponse {
            final_url,
            status,
            content_type,
            cookie,
            origin_cookies,
            redirect_trace,
            body,
        } => {
            validate_http_url(final_url).map_err(EncodeError::InvalidValue)?;
            if !(100..=599).contains(status) || body.len() > MAX_HTTP_BODY_BYTES {
                return Err(EncodeError::InvalidValue("HTTP response"));
            }
            writer.u8(4);
            writer.string(final_url, MAX_TEXT_BYTES, "final HTTP URL")?;
            writer.u16(*status);
            writer.string(content_type, MAX_TEXT_BYTES, "HTTP content type")?;
            writer.string(cookie, MAX_TEXT_BYTES, "HTTP cookie")?;
            writer.count(origin_cookies.len(), 64, "HTTP origin cookies")?;
            for entry in origin_cookies {
                validate_http_url(&entry.origin).map_err(EncodeError::InvalidValue)?;
                writer.string(&entry.origin, MAX_TEXT_BYTES, "HTTP cookie origin")?;
                writer.string(&entry.cookie, MAX_TEXT_BYTES, "HTTP origin cookie")?;
            }
            writer.string(
                redirect_trace,
                MAX_HTTP_METADATA_BYTES,
                "HTTP redirect trace and metadata",
            )?;
            writer.bytes(body, MAX_HTTP_BODY_BYTES, "HTTP body")?;
        }
        DataProviderResult::Applications {
            schema_version,
            mime_type,
            entries,
        } => {
            if *schema_version != APPLICATION_CATALOG_VERSION {
                return Err(EncodeError::InvalidValue("application catalog version"));
            }
            validate_mime_type(mime_type).map_err(EncodeError::InvalidValue)?;
            validate_catalog(entries).map_err(EncodeError::InvalidValue)?;
            writer.u8(5);
            writer.u16(*schema_version);
            writer.string(mime_type, MAX_TEXT_BYTES, "application MIME type")?;
            writer.count(entries.len(), MAX_APPLICATIONS, "applications")?;
            for entry in entries {
                encode_application(writer, entry)?;
            }
        }
        DataProviderResult::ApplicationLaunched {
            application_id,
            default_changed,
        } => {
            validate_application_id(application_id).map_err(EncodeError::InvalidValue)?;
            writer.u8(6);
            writer.string(application_id, MAX_APPLICATION_ID_BYTES, "application id")?;
            writer.bool(*default_changed);
        }
        DataProviderResult::MountConnected(entry) => {
            validate_mounts(std::slice::from_ref(entry)).map_err(EncodeError::InvalidValue)?;
            writer.u8(if entry.icon.is_some() { 14 } else { 7 });
            encode_mount(writer, entry)?;
            if let Some(icon) = entry.icon { writer.u16(icon); }
        }
        DataProviderResult::DocumentInfo {
            page_count,
            outline,
        } => {
            validate_document_info(*page_count, outline).map_err(EncodeError::InvalidValue)?;
            writer.u8(8);
            writer.u32(*page_count);
            writer.count(
                outline.len(),
                MAX_DOCUMENT_OUTLINE_ENTRIES,
                "document outline",
            )?;
            for entry in outline {
                writer.string(&entry.title, MAX_TEXT_BYTES, "document outline title")?;
                writer.u32(entry.page);
                writer.u8(entry.depth);
            }
        }
        DataProviderResult::DocumentSearch {
            query,
            hits,
            truncated,
        } => {
            validate_document_query(query).map_err(EncodeError::InvalidValue)?;
            validate_document_hits(hits).map_err(EncodeError::InvalidValue)?;
            writer.u8(9);
            writer.string(query, MAX_TEXT_BYTES, "document search query")?;
            writer.bool(*truncated);
            writer.count(
                hits.len(),
                MAX_DOCUMENT_SEARCH_RESULTS,
                "document search results",
            )?;
            for hit in hits {
                writer.u32(hit.page);
                writer.string(&hit.snippet, MAX_TEXT_BYTES, "document search snippet")?;
            }
        }
        DataProviderResult::Authorization { callback_url } => {
            validate_auth_result_url(callback_url).map_err(EncodeError::InvalidValue)?;
            writer.u8(10);
            writer.string(callback_url, MAX_TEXT_BYTES, "auth callback URL")?;
        }
        DataProviderResult::CloudProviders { json } => {writer.u8(12);writer.string(json, MAX_TEXT_BYTES, "cloud providers")?;}
        DataProviderResult::NetworkConnectionInfo { json } => {writer.u8(11);writer.string(json, MAX_TEXT_BYTES, "network connection info")?;}
        DataProviderResult::Unavailable { reason } => {
            validate_nonempty(reason).map_err(EncodeError::InvalidValue)?;
            writer.u8(2);
            writer.string(reason, MAX_TEXT_BYTES, "unavailable reason")?;
        }
    }
    Ok(())
}

pub(super) fn decode_response(
    reader: &mut Reader<'_>,
) -> Result<DataProviderResponse, DecodeError> {
    let request = reader.id("request")?;
    let result = match reader.u8()? {
        0 => {
            let bytes = reader.bytes(MAX_ENTROPY_BYTES, "entropy")?;
            if bytes.is_empty() {
                return Err(DecodeError::InvalidValue("entropy length"));
            }
            DataProviderResult::Entropy(bytes)
        }
        1 => {
            let decimal_rate = reader.string(MAX_TEXT_BYTES, "decimal_rate")?;
            validate_positive_decimal(&decimal_rate).map_err(DecodeError::InvalidValue)?;
            let observed_millis = reader.u64()?;
            let source = reader.string(MAX_TEXT_BYTES, "rate source")?;
            validate_nonempty(&source).map_err(DecodeError::InvalidValue)?;
            DataProviderResult::CurrencyRate {
                decimal_rate,
                observed_millis,
                source,
            }
        }
        2 => {
            let reason = reader.string(MAX_TEXT_BYTES, "unavailable reason")?;
            validate_nonempty(&reason).map_err(DecodeError::InvalidValue)?;
            DataProviderResult::Unavailable { reason }
        }
        tag @ (3 | 13) => {
            let can_connect_local = reader.bool("can connect local mount")?;
            let count = reader.count(MAX_MOUNTS, "mounts")?;
            let mut entries = Vec::with_capacity(count);
            for _ in 0..count {
                let id = nonempty_string(reader, MAX_TEXT_BYTES, "mount id")?;
                let label = nonempty_string(reader, MAX_TEXT_BYTES, "mount label")?;
                let path = reader.string(MAX_PATH_BYTES, "mount path")?;
                let state = match reader.u8()? {
                    0 => MountState::Connected,
                    1 => MountState::PermissionRequired,
                    2 => MountState::Syncing,
                    3 => MountState::Error,
                    _ => return Err(DecodeError::InvalidValue("mount state")),
                };
                let capabilities = MountCapabilities::from_bits(reader.u8()?)
                    .ok_or(DecodeError::InvalidValue("mount capabilities"))?;
                entries.push(MountEntry {
                    id,
                    label,
                    path,
                    state,
                    capabilities,
                    icon: if tag == 13 { match reader.u16()? { 0 => None, value => Some(value) } } else { None },
                });
            }
            validate_mounts(&entries).map_err(DecodeError::InvalidValue)?;
            DataProviderResult::Mounts {
                entries,
                can_connect_local,
            }
        }
        4 => {
            let final_url = nonempty_string(reader, MAX_TEXT_BYTES, "final HTTP URL")?;
            validate_http_url(&final_url).map_err(DecodeError::InvalidValue)?;
            let status = reader.u16()?;
            if !(100..=599).contains(&status) {
                return Err(DecodeError::InvalidValue("HTTP status"));
            }
            let content_type = reader.string(MAX_TEXT_BYTES, "HTTP content type")?;
            let cookie = reader.string(MAX_TEXT_BYTES, "HTTP cookie")?;
            let origin_cookie_count = reader.count(64, "HTTP origin cookies")?;
            let mut origin_cookies = Vec::with_capacity(origin_cookie_count);
            for _ in 0..origin_cookie_count {
                let origin = nonempty_string(reader, MAX_TEXT_BYTES, "HTTP cookie origin")?;
                validate_http_url(&origin).map_err(DecodeError::InvalidValue)?;
                let cookie = reader.string(MAX_TEXT_BYTES, "HTTP origin cookie")?;
                origin_cookies.push(crate::OriginCookie { origin, cookie });
            }
            let redirect_trace =
                reader.string(MAX_HTTP_METADATA_BYTES, "HTTP redirect trace and metadata")?;
            let body = reader.bytes(MAX_HTTP_BODY_BYTES, "HTTP body")?;
            DataProviderResult::HttpResponse {
                final_url,
                status,
                content_type,
                cookie,
                origin_cookies,
                redirect_trace,
                body,
            }
        }
        tag @ (5 | 6) => decode_application_result(reader, tag)?,
        7 => DataProviderResult::MountConnected(decode_mount(reader)?),
        14 => { let mut entry = decode_mount(reader)?; entry.icon = Some(reader.u16()?); DataProviderResult::MountConnected(entry) },
        8 => {
            let page_count = reader.u32()?;
            let count = reader.count(MAX_DOCUMENT_OUTLINE_ENTRIES, "document outline")?;
            let mut outline = Vec::with_capacity(count);
            for _ in 0..count {
                outline.push(DocumentOutlineEntry {
                    title: nonempty_string(reader, MAX_TEXT_BYTES, "document outline title")?,
                    page: reader.u32()?,
                    depth: reader.u8()?,
                });
            }
            validate_document_info(page_count, &outline).map_err(DecodeError::InvalidValue)?;
            DataProviderResult::DocumentInfo {
                page_count,
                outline,
            }
        }
        9 => {
            let query = nonempty_string(reader, MAX_TEXT_BYTES, "document search query")?;
            validate_document_query(&query).map_err(DecodeError::InvalidValue)?;
            let truncated = reader.bool("document search truncated")?;
            let count = reader.count(MAX_DOCUMENT_SEARCH_RESULTS, "document search results")?;
            let mut hits = Vec::with_capacity(count);
            for _ in 0..count {
                hits.push(DocumentSearchHit {
                    page: reader.u32()?,
                    snippet: nonempty_string(reader, MAX_TEXT_BYTES, "document search snippet")?,
                });
            }
            validate_document_hits(&hits).map_err(DecodeError::InvalidValue)?;
            DataProviderResult::DocumentSearch {
                query,
                hits,
                truncated,
            }
        }
        10 => {
            let callback_url = nonempty_string(reader, MAX_TEXT_BYTES, "auth callback URL")?;
            validate_auth_result_url(&callback_url).map_err(DecodeError::InvalidValue)?;
            DataProviderResult::Authorization { callback_url }
        }
        12 => DataProviderResult::CloudProviders {json:reader.string(MAX_TEXT_BYTES,"cloud providers")?},
        11 => DataProviderResult::NetworkConnectionInfo {json:reader.string(MAX_TEXT_BYTES,"network connection info")?},
        _ => return Err(DecodeError::InvalidValue("data provider result")),
    };
    Ok(DataProviderResponse { request, result })
}

fn validate_auth_application_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > MAX_APPLICATION_ID_BYTES
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit() && index > 0
                || byte == b'-' && index > 0
        })
    {
        return Err("auth application id");
    }
    Ok(())
}

fn validate_auth_provider_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > MAX_AUTH_PROVIDER_ID_BYTES
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit() && index > 0
                || byte == b'-' && index > 0
        })
    {
        return Err("auth provider id");
    }
    Ok(())
}

fn validate_auth_callback(mode: &str, url: &str) -> Result<(), &'static str> {
    if !matches!(
        mode,
        "https" | "loopback" | "custom-scheme" | "post-message"
    ) {
        return Err("auth callback mode");
    }
    if mode == "https" || mode == "post-message" {
        validate_http_url(url)?;
        if !url.starts_with("https://") {
            return Err("auth callback URL");
        }
    } else if url.is_empty()
        || url.len() > MAX_TEXT_BYTES
        || url
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
    {
        return Err("auth callback URL");
    }
    Ok(())
}

fn validate_auth_interaction(kind: &str, value: &str) -> Result<(), &'static str> {
    if kind.is_empty() {
        return if value.is_empty() {
            Ok(())
        } else {
            Err("auth interaction")
        };
    }
    if kind != "follow-link-once"
        || value.trim().is_empty()
        || value.len() > MAX_AUTH_INTERACTION_BYTES
        || value
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
    {
        return Err("auth interaction");
    }
    Ok(())
}

fn validate_auth_result_url(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
        || !value.contains("://")
    {
        return Err("auth callback URL");
    }
    Ok(())
}

fn validate_document_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty()
        || !path.starts_with('/')
        || path.contains('\0')
        || path.split('/').any(|part| matches!(part, "." | ".."))
        || !path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("pdf"))
    {
        return Err("document path");
    }
    Ok(())
}

fn validate_document_query(query: &str) -> Result<(), &'static str> {
    if query.trim().is_empty() || query.contains('\0') || query.len() > MAX_TEXT_BYTES {
        return Err("document search query");
    }
    Ok(())
}

fn validate_document_info(
    page_count: u32,
    outline: &[DocumentOutlineEntry],
) -> Result<(), &'static str> {
    if page_count == 0
        || page_count > MAX_DOCUMENT_PAGES
        || outline.len() > MAX_DOCUMENT_OUTLINE_ENTRIES
        || outline.iter().any(|entry| {
            entry.title.trim().is_empty()
                || entry.title.len() > MAX_TEXT_BYTES
                || entry.title.contains('\0')
                || entry.page == 0
                || entry.page > page_count
                || entry.depth > 32
        })
    {
        return Err("document info");
    }
    Ok(())
}

fn validate_document_hits(hits: &[DocumentSearchHit]) -> Result<(), &'static str> {
    if hits.len() > MAX_DOCUMENT_SEARCH_RESULTS
        || hits.iter().any(|hit| {
            hit.page == 0
                || hit.page > MAX_DOCUMENT_PAGES
                || hit.snippet.trim().is_empty()
                || hit.snippet.len() > MAX_TEXT_BYTES
                || hit.snippet.contains('\0')
        })
    {
        return Err("document search results");
    }
    Ok(())
}

fn decode_application_result(
    reader: &mut Reader<'_>,
    tag: u8,
) -> Result<DataProviderResult, DecodeError> {
    if tag == 6 {
        let application_id = nonempty_string(reader, MAX_APPLICATION_ID_BYTES, "application id")?;
        validate_application_id(&application_id).map_err(DecodeError::InvalidValue)?;
        return Ok(DataProviderResult::ApplicationLaunched {
            application_id,
            default_changed: reader.bool("application default changed")?,
        });
    }

    let schema_version = reader.u16()?;
    if schema_version != APPLICATION_CATALOG_VERSION {
        return Err(DecodeError::InvalidValue("application catalog version"));
    }
    let mime_type = nonempty_string(reader, MAX_TEXT_BYTES, "application MIME type")?;
    validate_mime_type(&mime_type).map_err(DecodeError::InvalidValue)?;
    let count = reader.count(MAX_APPLICATIONS, "applications")?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(decode_application(reader)?);
    }
    validate_catalog(&entries).map_err(DecodeError::InvalidValue)?;
    Ok(DataProviderResult::Applications {
        schema_version,
        mime_type,
        entries,
    })
}

fn encode_application(
    writer: &mut Writer,
    entry: &InstalledApplication,
) -> Result<(), EncodeError> {
    writer.string(&entry.id, MAX_APPLICATION_ID_BYTES, "application id")?;
    writer.string(&entry.name, MAX_TEXT_BYTES, "application name")?;
    writer.string(&entry.icon, MAX_TEXT_BYTES, "application icon")?;
    writer.count(
        entry.mime_types.len(),
        MAX_MIME_TYPES,
        "application MIME types",
    )?;
    for mime_type in &entry.mime_types {
        writer.string(mime_type, MAX_TEXT_BYTES, "application MIME type")?;
    }
    writer.bool(entry.matches_requested_type);
    writer.bool(entry.is_default);
    writer.bool(entry.is_native);
    Ok(())
}

fn encode_mount(writer: &mut Writer, entry: &MountEntry) -> Result<(), EncodeError> {
    writer.string(&entry.id, MAX_TEXT_BYTES, "mount id")?;
    writer.string(&entry.label, MAX_TEXT_BYTES, "mount label")?;
    writer.string(&entry.path, MAX_PATH_BYTES, "mount path")?;
    writer.u8(entry.state as u8);
    writer.u8(entry.capabilities.bits());
    Ok(())
}

fn decode_mount(reader: &mut Reader<'_>) -> Result<MountEntry, DecodeError> {
    let entry = MountEntry {
        icon: None,
        id: nonempty_string(reader, MAX_TEXT_BYTES, "mount id")?,
        label: nonempty_string(reader, MAX_TEXT_BYTES, "mount label")?,
        path: nonempty_string(reader, MAX_PATH_BYTES, "mount path")?,
        state: match reader.u8()? {
            0 => MountState::Connected,
            1 => MountState::PermissionRequired,
            2 => MountState::Syncing,
            3 => MountState::Error,
            _ => return Err(DecodeError::InvalidValue("mount state")),
        },
        capabilities: MountCapabilities::from_bits(reader.u8()?)
            .ok_or(DecodeError::InvalidValue("mount capabilities"))?,
    };
    validate_mounts(std::slice::from_ref(&entry)).map_err(DecodeError::InvalidValue)?;
    Ok(entry)
}

fn decode_application(reader: &mut Reader<'_>) -> Result<InstalledApplication, DecodeError> {
    let id = nonempty_string(reader, MAX_APPLICATION_ID_BYTES, "application id")?;
    let name = nonempty_string(reader, MAX_TEXT_BYTES, "application name")?;
    let icon = reader.string(MAX_TEXT_BYTES, "application icon")?;
    let count = reader.count(MAX_MIME_TYPES, "application MIME types")?;
    let mut mime_types = Vec::with_capacity(count);
    for _ in 0..count {
        mime_types.push(nonempty_string(
            reader,
            MAX_TEXT_BYTES,
            "application MIME type",
        )?);
    }
    Ok(InstalledApplication {
        id,
        name,
        icon,
        mime_types,
        matches_requested_type: reader.bool("application MIME match")?,
        is_default: reader.bool("default application")?,
        is_native: reader.bool("native application")?,
    })
}

fn validate_entropy_length(byte_len: u16) -> Result<(), &'static str> {
    if byte_len == 0 || usize::from(byte_len) > MAX_ENTROPY_BYTES {
        Err("entropy length")
    } else {
        Ok(())
    }
}

fn validate_iso(value: &str) -> Result<(), &'static str> {
    if value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err("ISO 4217 code")
    }
}

fn validate_nonempty(value: &str) -> Result<(), &'static str> {
    if value.trim().is_empty() {
        Err("provider text")
    } else {
        Ok(())
    }
}

fn validate_http_url(value: &str) -> Result<(), &'static str> {
    if (value.starts_with("http://") || value.starts_with("https://"))
        && !value.contains('\0')
        && value.len() <= MAX_TEXT_BYTES
    {
        Ok(())
    } else {
        Err("HTTP URL")
    }
}

fn validate_http_method(value: &str) -> Result<(), &'static str> {
    if matches!(value, "GET" | "POST" | "PUT" | "DELETE") {
        Ok(())
    } else {
        Err("HTTP method")
    }
}

fn validate_http_headers(headers: &[HttpHeader]) -> Result<(), &'static str> {
    if headers.len() > MAX_HTTP_HEADERS {
        return Err("HTTP headers");
    }
    let mut block_bytes = 0usize;
    for header in headers {
        let name = header.name.as_bytes();
        if name.is_empty()
            || name.len() > MAX_HTTP_HEADER_NAME_BYTES
            || !name.iter().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#'
                            | b'$'
                            | b'%'
                            | b'&'
                            | b'\''
                            | b'*'
                            | b'+'
                            | b'-'
                            | b'.'
                            | b'^'
                            | b'_'
                            | b'`'
                            | b'|'
                            | b'~'
                    )
            })
            || header.value.len() > MAX_HTTP_HEADER_VALUE_BYTES
            || header
                .value
                .bytes()
                .any(|byte| matches!(byte, b'\r' | b'\n' | 0))
        {
            return Err("HTTP header");
        }
        let folded = header.name.to_ascii_lowercase();
        if matches!(
            folded.as_str(),
            "connection"
                | "content-length"
                | "cookie"
                | "host"
                | "proxy-authorization"
                | "set-cookie"
                | "transfer-encoding"
                | "upgrade"
        ) || folded.starts_with("x-ricewm-")
            || folded.starts_with("x-bootup-")
        {
            return Err("forbidden HTTP header");
        }
        block_bytes = block_bytes
            .saturating_add(header.name.len())
            .saturating_add(header.value.len())
            .saturating_add(4);
        if block_bytes > MAX_HTTP_HEADER_BLOCK_BYTES {
            return Err("HTTP header block");
        }
    }
    Ok(())
}

fn validate_http_limits(timeout_millis: u32, response_body_limit: u32) -> Result<(), &'static str> {
    if !(1_000..=MAX_HTTP_TIMEOUT_MILLIS).contains(&timeout_millis) {
        return Err("HTTP timeout");
    }
    if response_body_limit == 0
        || usize::try_from(response_body_limit).map_or(true, |limit| limit > MAX_HTTP_BODY_BYTES)
    {
        return Err("HTTP response body limit");
    }
    Ok(())
}

fn validate_webdav_url(value: &str) -> Result<(), &'static str> {
    if value.starts_with("https://")
        || value.starts_with("http://localhost/")
        || value.starts_with("http://127.0.0.1/")
        || value.starts_with("http://[::1]/")
    {
        validate_http_url(value)
    } else {
        Err("WebDAV URL")
    }
}

fn validate_credential(value: &str) -> Result<(), &'static str> {
    if value.contains(['\0', '\r', '\n']) {
        Err("WebDAV credential")
    } else {
        Ok(())
    }
}

fn nonempty_string(
    reader: &mut Reader<'_>,
    maximum: usize,
    field: &'static str,
) -> Result<String, DecodeError> {
    let value = reader.string(maximum, field)?;
    validate_nonempty(&value).map_err(DecodeError::InvalidValue)?;
    Ok(value)
}

fn validate_mounts(entries: &[MountEntry]) -> Result<(), &'static str> {
    if entries.len() > MAX_MOUNTS {
        return Err("mount count");
    }
    for (index, entry) in entries.iter().enumerate() {
        if validate_nonempty(&entry.id).is_err()
            || validate_nonempty(&entry.label).is_err()
            || !entry.path.starts_with('/')
            || entry.path.contains('\0')
            || entry.path.len() > MAX_PATH_BYTES
            || entries[..index].iter().any(|prior| prior.id == entry.id)
        {
            return Err("mount entry");
        }
    }
    Ok(())
}

fn validate_positive_decimal(value: &str) -> Result<(), &'static str> {
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|digits| {
            digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        || !value.bytes().any(|byte| matches!(byte, b'1'..=b'9'))
    {
        Err("positive decimal rate")
    } else {
        Ok(())
    }
}

fn validate_network_url(value: &str) -> Result<(), &'static str> {
    if !(value.starts_with("smb://") || value.starts_with("nfs://")) || value.contains(['@', '\0', '\r', '\n']) {
        return Err("network URL");
    }
    Ok(())
}
