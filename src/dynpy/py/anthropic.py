# Brevi's anthropic: the Python SDK's API (anthropic 2.6) for what agent programs use: Anthropic and AsyncAnthropic,
# messages.create (also stream=True), messages.stream with its MessageStream helper (text_stream, the TextEvent /
# InputJsonEvent / ... events, get_final_message, get_final_text), messages.count_tokens, models.list (pages)
# and models.retrieve, with_options / copy, the SDK's errors, retries and request headers. Response objects are
# the SDK's own models (src/dynpy/py/_brevi_anthropic_types.py, written from the SDK by tools/gen_llm_types.py).
# It runs over Brevi's httpx. caps.rs lists what it has (OWN_ANTHROPIC) and what sends a program to CPython's
# anthropic instead (ANTHROPIC_LACKS: beta, batches, files, tool runners, raw responses, ...). messages.parse and
# output_format (structured outputs) build on Brevi's pydantic TypeAdapter.
# Its behaviour follows anthropic-sdk-python (Copyright Anthropic, PBC; Apache-2.0 and MIT licensed), whose error messages
# it repeats.
import json

import _brevi_http
import _brevi_sdk
import httpx
from _brevi_sdk import (
    NOT_GIVEN,
    NotGiven,
    Omit,
    omit,
    not_given,
    BaseModel,
    Stream,
    AsyncStream,
    request_options,
    strip_not_given,
    is_given,
    build,
    page_class,
)
import anthropic.types as types
from _brevi_anthropic_types import (
    Message,
    MessageTokensCount,
    ModelInfo,
    ParsedMessage,
    TextEvent,
    CitationEvent,
    ThinkingEvent,
    SignatureEvent,
    InputJsonEvent,
    ToolUseBlock,
    ServerToolUseBlock,
)
import _brevi_anthropic_types as _ty
import pydantic as _pyd

__title__ = "anthropic"
__version__ = "2.6.0"
ANTHROPIC_API_VERSION = "2026-10-21"
DEFAULT_MAX_RETRIES = 2
DEFAULT_TIMEOUT = httpx.Timeout(timeout=10 * 60, connect=5.0)
DEFAULT_CONNECTION_LIMITS = httpx.Limits(max_connections=1000, max_keepalive_connections=100)
Timeout = httpx.Timeout

HUMAN_PROMPT = "\n\nHuman:"
AI_PROMPT = "\n\nAssistant:"

MODEL_NONSTREAMING_TOKENS = {
    "claude-opus-4-20250514": 8192,
    "claude-opus-4-0": 8192,
    "claude-4-opus-20250514": 8192,
    "anthropic.claude-opus-4-20250514-v1:0": 8192,
    "claude-opus-4@20250514": 8192,
    "anthropic.claude-opus-4-1-20250805-v1:0": 8192,
    "claude-opus-4-1@20250805": 8192,
}


# ---------- errors ----------

class AnthropicError(Exception):
    pass


class APIError(AnthropicError):
    def __init__(self, message, request, *, body):
        Exception.__init__(self, message)
        self.request = request
        self.message = message
        self.body = body


class APIResponseValidationError(APIError):
    def __init__(self, response, body, *, message=None):
        APIError.__init__(self, message or "Data returned by API invalid for expected schema.", response.request, body=body)
        self.response = response
        self.status_code = response.status_code


class APIWebhookValidationError(APIError):
    pass


def _prop(body, *keys):
    v = body
    for k in keys:
        if not isinstance(v, dict) or k not in v:
            return None
        v = v[k]
    return v


class APIStatusError(APIError):
    def __init__(self, message, *, response, body):
        APIError.__init__(self, message, response.request, body=body)
        self.response = response
        self.status_code = response.status_code
        self.request_id = response.headers.get("request-id")
        self.workspace_id = response.headers.get("anthropic-workspace-id")
        self.api_message = _prop(body, "error", "message")
        self.type = _prop(body, "error", "type")
        code = _prop(body, "error", "details", "error_code")
        self.error_code = code if isinstance(code, str) else None
        details = _prop(body, "error", "details")
        try:
            self.details = _brevi_sdk.construct_type(details, _ERR_DETAILS) if details is not None else None
        except Exception:
            self.details = None


class APIConnectionError(APIError):
    def __init__(self, *, message="Connection error.", request):
        APIError.__init__(self, message, request, body=None)


class APITimeoutError(APIConnectionError):
    def __init__(self, request):
        APIConnectionError.__init__(
            self,
            message="Request timed out or interrupted. This could be due to a network timeout, dropped connection, or request cancellation. See https://docs.anthropic.com/en/api/errors#long-requests for more details.",
            request=request,
        )


class RetryableError(AnthropicError):
    pass


class CredentialsError(AnthropicError):
    pass


class IdentityTokenFileError(CredentialsError):
    def __init__(self, message, *, path=None):
        Exception.__init__(self, message)
        self.path = path


class BadRequestError(APIStatusError):
    status_code = 400


class AuthenticationError(APIStatusError):
    status_code = 401


class PermissionDeniedError(APIStatusError):
    status_code = 403


class NotFoundError(APIStatusError):
    status_code = 404


class RequestTimeoutError(APIStatusError):
    status_code = 408


class ConflictError(APIStatusError):
    status_code = 409


class PreconditionFailedError(APIStatusError):
    status_code = 412


class ContentTooLargeError(APIStatusError):
    status_code = 413


RequestTooLargeError = ContentTooLargeError


class UnprocessableEntityError(APIStatusError):
    status_code = 422


class RateLimitError(APIStatusError):
    status_code = 429


class RequestHeaderFieldsTooLargeError(APIStatusError):
    status_code = 431


class ClientClosedRequestError(APIStatusError):
    status_code = 499


class InternalServerError(APIStatusError):
    pass


class HTTPNotImplementedError(InternalServerError):
    status_code = 501


class ServiceUnavailableError(InternalServerError):
    status_code = 503


class GatewayTimeoutError(InternalServerError):
    status_code = 504


DeadlineExceededError = GatewayTimeoutError


class OverloadedError(InternalServerError):
    status_code = 529


_STATUS = {
    400: BadRequestError, 401: AuthenticationError, 403: PermissionDeniedError, 404: NotFoundError, 408: RequestTimeoutError,
    409: ConflictError, 412: PreconditionFailedError, 413: ContentTooLargeError, 422: UnprocessableEntityError, 429: RateLimitError,
    431: RequestHeaderFieldsTooLargeError, 499: ClientClosedRequestError, 501: HTTPNotImplementedError, 503: ServiceUnavailableError,
    504: GatewayTimeoutError, 529: OverloadedError,
}
_BASES = {}
for _c in _STATUS.values():
    _BASES[_c.__name__] = _c
_BASES["InternalServerError"] = InternalServerError
_BASES["APIStatusError"] = APIStatusError
_CODED = {}
_BY_NAME = {}
for _st, _code, _name, _base, _mod in json.loads(_ty._ERRORS):
    _cls = _BY_NAME.get(_name)
    if _cls is None:
        _cls = type(_name, (_BASES[_base],), {"__module__": _mod})
        _BY_NAME[_name] = _cls
    _CODED[(_st, _code)] = _cls

CMEKKeyDisabledError = _BY_NAME["CMEKKeyDisabledError"]
RampLimitExceededError = _BY_NAME["RampLimitExceededError"]
CMEKKeyUnreachableError = _BY_NAME["CMEKKeyUnreachableError"]
OrganizationOnHoldError = _BY_NAME["OrganizationOnHoldError"]
CMEKKeyNetworkBlockedError = _BY_NAME["CMEKKeyNetworkBlockedError"]
CMEKContextUnavailableError = _BY_NAME["CMEKContextUnavailableError"]
ClaudeCodeVersionTooOldError = _BY_NAME["ClaudeCodeVersionTooOldError"]
EnforcedSpendLimitReachedError = _BY_NAME["EnforcedSpendLimitReachedError"]
RequestBodyEncodingInvalidError = _BY_NAME["RequestBodyEncodingInvalidError"]
OrganizationTypeNotSupportedError = _BY_NAME["OrganizationTypeNotSupportedError"]
ClaudeCodeKeyCreatorNotMemberError = _BY_NAME["ClaudeCodeKeyCreatorNotMemberError"]
RequestBodyEncodingUnsupportedError = _BY_NAME["RequestBodyEncodingUnsupportedError"]
RequestBodyTooLargeAfterDecodingError = _BY_NAME["RequestBodyTooLargeAfterDecodingError"]

_ERR_DETAILS = _ty._UNIONS.get("ErrorDetails")


def make_status_error(err_msg, *, body, response):
    code = _prop(body, "error", "details", "error_code")
    cls = _CODED.get((response.status_code, code if isinstance(code, str) else None)) or _STATUS.get(response.status_code)
    if cls is None:
        cls = InternalServerError if response.status_code >= 500 else APIStatusError
    return cls(err_msg, response=response, body=body)


# ---------- pages ----------

class SyncPage(_brevi_sdk.BasePage):
    def has_next_page(self):
        if self.has_more is not None and self.has_more is False:
            return False
        return _brevi_sdk.BasePage.has_next_page(self)

    def next_page_info(self):
        if self._bv_options.params.get("before_id"):
            if not self.first_id:
                return None
            return {"before_id": self.first_id}
        if not self.last_id:
            return None
        return {"after_id": self.last_id}


class AsyncPage(_brevi_sdk.AsyncBasePage):
    def has_next_page(self):
        if self.has_more is not None and self.has_more is False:
            return False
        return _brevi_sdk.BasePage.has_next_page(self)

    def next_page_info(self):
        return SyncPage.next_page_info(self)


_PAGE_FIELDS = [("has_more", ("U", ("bool", "none"), None), ("v", None)), ("first_id", ("U", ("str", "none"), None), ("v", None)),
                ("last_id", ("U", ("str", "none"), None), ("v", None))]
_ModelsPage = page_class(SyncPage, "SyncPage[ModelInfo]", "ModelInfo", _PAGE_FIELDS, "anthropic.pagination")
_AsyncModelsPage = page_class(AsyncPage, "AsyncPage[ModelInfo]", "ModelInfo", _PAGE_FIELDS, "anthropic.pagination")

_EVENT = ("U", (("M", "RawMessageStartEvent"), ("M", "RawMessageDeltaEvent"), ("M", "RawMessageStopEvent"), ("M", "RawContentBlockStartEvent"),
                ("M", "RawContentBlockDeltaEvent"), ("M", "RawContentBlockStopEvent")),
          ("type", {"message_start": "RawMessageStartEvent", "message_delta": "RawMessageDeltaEvent", "message_stop": "RawMessageStopEvent",
                    "content_block_start": "RawContentBlockStartEvent", "content_block_delta": "RawContentBlockDeltaEvent",
                    "content_block_stop": "RawContentBlockStopEvent"}))
_PARSED_BLOCK = ParsedMessage._FN["content"][2][1]


# ---------- clients ----------

_SKIP = object()

def _env(name):
    return _brevi_http.env_exact(name)


class _Common:
    _sdk_error = AnthropicError
    _array_format = "brackets"
    _none_retries_msg = "max_retries cannot be None. If you want to disable retries, pass `0`; if you want unlimited retries, pass a very high number; if you want the default behavior, pass `anthropic.DEFAULT_MAX_RETRIES`"

    def _init(self, api_key, auth_token, base_url, timeout, max_retries, default_headers, default_query, http_client, webhook_key):
        given = [n for n, v in (("`api_key=`", api_key), ("`auth_token=`", auth_token)) if v is not None]
        if len(given) > 1:
            raise ValueError("Pass at most one of `api_key=`, `auth_token=`, `credentials=`, `config=`, or `profile=`. Received " + ", ".join(given) + ".")
        if not given:
            api_key = _env("ANTHROPIC_API_KEY") or None
            auth_token = _env("ANTHROPIC_AUTH_TOKEN") or None
            if api_key is not None:
                auth_token = None
        self.api_key = api_key
        self.auth_token = auth_token
        if webhook_key is None:
            webhook_key = _env("ANTHROPIC_WEBHOOK_SIGNING_KEY")
        self.webhook_key = webhook_key
        self.credentials = None
        if base_url is None:
            base_url = _env("ANTHROPIC_BASE_URL")
        if base_url is None:
            base_url = "https://api.anthropic.com"
        hv = _env("ANTHROPIC_CUSTOM_HEADERS")
        if hv is not None:
            parsed = {}
            for line in hv.split("\n"):
                colon = line.find(":")
                if colon >= 0:
                    parsed[line[:colon].strip()] = line[colon + 1:].strip()
            for k, v in (default_headers or {}).items():
                parsed[k] = v
            default_headers = parsed
        self._setup(version=__version__, base_url=base_url, max_retries=max_retries, timeout=timeout, custom_headers=default_headers,
                    custom_query=default_query, default_timeout=DEFAULT_TIMEOUT)
        self._init_http(http_client)
        self.messages = (AsyncMessages if self._is_async else Messages)(self)
        self.models = (AsyncModels if self._is_async else Models)(self)

    @property
    def auth_headers(self):
        if self.api_key is not None:
            return {"Authorization": "Bearer " + self.api_key}
        if self.auth_token is not None:
            return {"Authorization": "Bearer " + self.auth_token}
        return {}

    @property
    def default_headers(self):
        h = self._base_headers()
        h["X-Stainless-Async"] = "async:asyncio" if self._is_async else "false"
        h["anthropic-version"] = ANTHROPIC_API_VERSION
        for k, v in self._custom_headers.items():
            h[k] = v
        return h

    def _validate_headers(self, headers):
        for k in headers:
            if k.lower() in ("authorization", "x-api-key"):
                return
        raise TypeError('"Could not resolve authentication method. Expected one of api_key, auth_token, or credentials to be set. Or for the `Authorization` header to be explicitly omitted"')

    def _make_status_error(self, err_msg, *, body, response):
        return make_status_error(err_msg, body=body, response=response)

    def _sse_item(self, sse, stream):
        response = stream.response
        if sse.event == "ping" or sse.event is None:
            return _SKIP
        if sse.event == "error":
            body = sse.data
            try:
                body = sse.json()
                err_msg = "%s" % (body,)
            except Exception:
                err_msg = sse.data or "Error code: %d" % response.status_code
            response.is_closed or response.close() if not self._is_async else None
            raise self._make_status_error(err_msg, body=body, response=response)
        if not sse.data:
            return _SKIP
        return self._process_response_data(data=sse.json(), cast_to=stream._cast_to, response=response)

    def _timeout_error(self, request):
        return APITimeoutError(request=request)

    def _connection_error(self, request):
        return APIConnectionError(request=request)

    def _add_ids(self, v, response):
        v._request_id = response.headers.get("request-id")
        v._workspace_id = response.headers.get("anthropic-workspace-id")

    def _calculate_nonstreaming_timeout(self, max_tokens, max_nonstreaming_tokens):
        expected = 60 * 60 * max_tokens / 128000
        if expected > 60 * 10 or (max_nonstreaming_tokens and max_tokens > max_nonstreaming_tokens):
            raise ValueError(
                "Streaming is required for operations that may take longer than 10 minutes. "
                + "See https://platform.claude.com/docs/en/cli-sdks-libraries/sdks/python#long-requests for more details"
            )
        return httpx.Timeout(60 * 10, connect=5.0)

    def copy(self, *, api_key=None, auth_token=None, credentials=NOT_GIVEN, config=None, profile=None, webhook_key=None, base_url=None,
             timeout=NOT_GIVEN, http_client=None, max_retries=NOT_GIVEN, open_telemetry=None, default_headers=None, set_default_headers=None,
             default_query=None, set_default_query=None, middleware=NOT_GIVEN, _extra_kwargs={}):
        if default_headers is not None and set_default_headers is not None:
            raise ValueError("The `default_headers` and `set_default_headers` arguments are mutually exclusive")
        if default_query is not None and set_default_query is not None:
            raise ValueError("The `default_query` and `set_default_query` arguments are mutually exclusive")
        headers = self._custom_headers
        if default_headers is not None:
            headers = _brevi_sdk._merge_headers(headers, default_headers)
        elif set_default_headers is not None:
            headers = set_default_headers
        params = self._custom_query
        if default_query is not None:
            params = dict(params)
            for k, v in default_query.items():
                params[k] = v
        elif set_default_query is not None:
            params = set_default_query
        if not api_key and not auth_token:
            api_key = self.api_key
            auth_token = self.auth_token if api_key is None else None
        return type(self)(
            api_key=api_key,
            auth_token=auth_token,
            webhook_key=webhook_key or self.webhook_key,
            base_url=base_url or self.base_url,
            timeout=self.timeout if isinstance(timeout, NotGiven) else timeout,
            http_client=http_client or self._client,
            max_retries=max_retries if is_given(max_retries) else self.max_retries,
            default_headers=headers,
            default_query=params,
        )

    with_options = copy


class Anthropic(_Common, _brevi_sdk.SyncClient):
    _is_async = False

    def __init__(self, *, api_key=None, auth_token=None, credentials=None, config=None, profile=None, webhook_key=None, base_url=None,
                 timeout=not_given, max_retries=DEFAULT_MAX_RETRIES, default_headers=None, default_query=None, open_telemetry=None, http_client=None,
                 middleware=None, _strict_response_validation=False, _token_cache=not_given):
        self._init(api_key, auth_token, base_url, timeout, max_retries, default_headers, default_query, http_client, webhook_key)

    def _stream_items(self, stream):
        response = stream.response
        for sse in stream._iter_events():
            item = self._sse_item(sse, stream)
            if item is not _SKIP:
                yield item
        response.close()


class AsyncAnthropic(_Common, _brevi_sdk.AsyncClientBase):
    _is_async = True

    def __init__(self, *, api_key=None, auth_token=None, credentials=None, config=None, profile=None, webhook_key=None, base_url=None,
                 timeout=not_given, max_retries=DEFAULT_MAX_RETRIES, default_headers=None, default_query=None, open_telemetry=None, http_client=None,
                 middleware=None, _strict_response_validation=False, _token_cache=not_given):
        self._init(api_key, auth_token, base_url, timeout, max_retries, default_headers, default_query, http_client, webhook_key)

    async def _stream_items(self, stream):
        response = stream.response
        async for sse in stream._iter_events():
            item = self._sse_item(sse, stream)
            if item is not _SKIP:
                yield item
        await response.aclose()


Client = Anthropic
AsyncClient = AsyncAnthropic


# ---------- resources ----------

def _missing(max_tokens, messages, model):
    if max_tokens is _NO or messages is _NO or model is _NO:
        raise TypeError("Missing required arguments; Expected either ('max_tokens', 'messages' and 'model') or ('max_tokens', 'messages', 'model' and 'stream') arguments to be given")


_NO = NotGiven()


def _headers(user_profile_id, workspace_id, extra_headers):
    h = strip_not_given({"anthropic-user-profile-id": user_profile_id, "anthropic-workspace-id": workspace_id})
    for k, v in (extra_headers or {}).items():
        h[k] = v
    return h


def _create_body(max_tokens, messages, model, cache_control, container, diagnostics, inference_geo, metadata, output_config, service_tier, speed,
                 stop_sequences, stream, system, thinking, tool_choice, tools):
    return {
        "max_tokens": max_tokens, "messages": messages, "model": model, "cache_control": cache_control, "container": container,
        "diagnostics": diagnostics, "inference_geo": inference_geo, "metadata": metadata, "output_config": output_config,
        "service_tier": service_tier, "speed": speed, "stop_sequences": stop_sequences, "stream": stream, "system": system,
        "thinking": thinking, "tool_choice": tool_choice, "tools": tools,
    }


def _stream_body(max_tokens, messages, model, cache_control, diagnostics, inference_geo, metadata, output_config, container, service_tier, speed,
                 stop_sequences, system, tools, thinking, tool_choice):
    return {
        "max_tokens": max_tokens, "messages": messages, "model": model, "cache_control": cache_control, "diagnostics": diagnostics,
        "inference_geo": inference_geo, "metadata": metadata, "output_config": output_config, "container": container,
        "service_tier": service_tier, "speed": speed, "stop_sequences": stop_sequences, "system": system, "tools": tools,
        "thinking": thinking, "tool_choice": tool_choice, "stream": True,
    }


def _count_body(messages, model, cache_control, output_config, speed, system, thinking, tool_choice, tools):
    return {"messages": messages, "model": model, "cache_control": cache_control, "output_config": output_config, "speed": speed,
            "system": system, "thinking": thinking, "tool_choice": tool_choice, "tools": tools}


# ---------- structured outputs (messages.parse, output_format) ----------

_SUPPORTED_STRING_FORMATS = {"date-time", "time", "date", "duration", "email", "hostname", "uri", "ipv4", "ipv6", "uuid"}


def transform_schema(json_schema):
    # the SDK's lib/_parse/_transform.py: a JSON schema as the API takes it
    if isinstance(json_schema, type) and issubclass(json_schema, _pyd.BaseModel):
        json_schema = json_schema.model_json_schema()
    strict_schema = {}
    json_schema = dict(json_schema)
    defs = json_schema.pop("$defs", None)
    if defs is not None:
        strict_defs = {}
        strict_schema["$defs"] = strict_defs
        for name, schema in defs.items():
            strict_defs[name] = transform_schema(schema)
    ref = json_schema.pop("$ref", None)
    if ref is not None:
        strict_schema["$ref"] = ref
        return strict_schema
    type_ = json_schema.pop("type", None)
    any_of = json_schema.pop("anyOf", None)
    one_of = json_schema.pop("oneOf", None)
    all_of = json_schema.pop("allOf", None)
    if isinstance(any_of, list):
        strict_schema["anyOf"] = [transform_schema(v) for v in any_of]
    elif isinstance(one_of, list):
        strict_schema["anyOf"] = [transform_schema(v) for v in one_of]
    elif isinstance(all_of, list):
        strict_schema["allOf"] = [transform_schema(v) for v in all_of]
    else:
        if type_ is None:
            raise ValueError("Schema must have a 'type', 'anyOf', 'oneOf', or 'allOf' field.")
        strict_schema["type"] = type_
    enum = json_schema.pop("enum", None)
    if isinstance(enum, list):
        strict_schema["enum"] = enum
    description = json_schema.pop("description", None)
    if description is not None:
        strict_schema["description"] = description
    title = json_schema.pop("title", None)
    if title is not None:
        strict_schema["title"] = title
    if type_ == "object":
        strict_schema["properties"] = {key: transform_schema(ps) for key, ps in json_schema.pop("properties", {}).items()}
        json_schema.pop("additionalProperties", None)
        strict_schema["additionalProperties"] = False
        required = json_schema.pop("required", None)
        if required is not None:
            strict_schema["required"] = required
    elif type_ == "string":
        fmt = json_schema.pop("format", None)
        if fmt and fmt in _SUPPORTED_STRING_FORMATS:
            strict_schema["format"] = fmt
        elif fmt:
            json_schema["format"] = fmt
    elif type_ == "array":
        items = json_schema.pop("items", None)
        if items is not None:
            strict_schema["items"] = transform_schema(items)
        min_items = json_schema.pop("minItems", None)
        if min_items is not None and min_items == 0 or min_items == 1:
            strict_schema["minItems"] = min_items
        elif min_items is not None:
            json_schema["minItems"] = min_items
    if json_schema:
        description = strict_schema.get("description")
        strict_schema["description"] = ((description + "\n\n" if description is not None else "") + "{"
                                        + ", ".join("%s: %s" % (key, value) for key, value in json_schema.items()) + "}")
    return strict_schema


def _json_format(output_format):
    # output_format=T as the request's output_config.format, or not_given
    if not is_given(output_format) or output_format is None:
        return not_given
    if isinstance(output_format, dict):
        raise TypeError("`output_format` must be a type; pass a schema dict as `output_config={'format': ...}` instead")
    try:
        schema = _pyd.TypeAdapter(output_format).json_schema()
    except _pyd.PydanticSchemaGenerationError:
        raise TypeError("Could not generate JSON schema for the given `output_format` type. Use a type that works with `pydantic.TypeAdapter`")
    return {"schema": transform_schema(schema), "type": "json_schema"}


def _merge_config(output_config, fmt):
    if is_given(fmt):
        if is_given(output_config):
            d = dict(output_config)
            d["format"] = fmt
            return d
        return {"format": fmt}
    return output_config


def _parse_text(text, output_format):
    if is_given(output_format) and output_format is not None:
        return _pyd.TypeAdapter(output_format).validate_json(text)
    return None


def _parsed_output(self):
    for content in self.content:
        if content.type == "text" and content.parsed_output:
            return content.parsed_output
    return None


_ty.ParsedTextBlock._EX = ("parsed_output",)
ParsedMessage.parsed_output = property(_parsed_output)
# what parse() returns: pydantic's names for the SDK's generic models
_GTextBlock = type("ParsedTextBlock[TypeVar]", (_ty.ParsedTextBlock,), {"__module__": "anthropic.types.parsed_message"})
_GMessage = type("ParsedMessage[TypeVar]", (ParsedMessage,), {"__module__": "anthropic.types.parsed_message"})


def _parse_response(response, output_format):
    content = []
    for block in response.content:
        if block.type == "text":
            d = block.to_dict()
            d["parsed_output"] = _parse_text(block.text, output_format)
            content.append(_GTextBlock.construct(**d))
        else:
            content.append(block)
    d = response.to_dict()
    d["content"] = []
    m = _GMessage.construct(**d)
    m.content = content
    rid = getattr(response, "_request_id", None)
    if rid is not None:
        m._request_id = rid
    return m


def _parse_headers(user_profile_id, workspace_id, extra_headers):
    h = {"x-stainless-helper": "messages.parse"}
    for k, v in _headers(user_profile_id, workspace_id, extra_headers).items():
        h[k] = v
    return h


class Messages:
    def __init__(self, client):
        self._client = client

    def create(self, *, max_tokens=_NO, messages=_NO, model=_NO, cache_control=omit, container=omit, diagnostics=omit, inference_geo=omit,
               metadata=omit, output_config=omit, service_tier=omit, speed=omit, stop_sequences=omit, stream=omit, system=omit, thinking=omit,
               tool_choice=omit, tools=omit, user_profile_id=omit, workspace_id=omit, extra_headers=None, extra_query=None, extra_body=None,
               timeout=not_given):
        _missing(max_tokens, messages, model)
        c = self._client
        if not stream and not is_given(timeout) and c.timeout == DEFAULT_TIMEOUT:
            timeout = c._calculate_nonstreaming_timeout(max_tokens, MODEL_NONSTREAMING_TOKENS.get(model, None))
        body = _create_body(max_tokens, messages, model, cache_control, container, diagnostics, inference_geo, metadata, output_config,
                            service_tier, speed, stop_sequences, stream, system, thinking, tool_choice, tools)
        opts = request_options(extra_headers=_headers(user_profile_id, workspace_id, extra_headers), extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)
        return c.post("/v1/messages", body=body, options=opts, cast_to=Message, stream=bool(stream), stream_cls=(Stream, _EVENT))

    def parse(self, *, max_tokens, messages, model, diagnostics=omit, metadata=omit, output_config=omit, output_format=omit, service_tier=omit,
              speed=omit, stop_sequences=omit, system=omit, thinking=omit, tool_choice=omit, tools=omit, extra_headers=None, extra_query=None,
              extra_body=None, timeout=not_given):
        c = self._client
        if not is_given(timeout) and c.timeout == DEFAULT_TIMEOUT:
            timeout = c._calculate_nonstreaming_timeout(max_tokens, MODEL_NONSTREAMING_TOKENS.get(model, None))
        output_config = _merge_config(output_config, _json_format(output_format))
        body = {"max_tokens": max_tokens, "messages": messages, "model": model, "diagnostics": diagnostics, "metadata": metadata,
                "output_config": output_config, "service_tier": service_tier, "speed": speed, "stop_sequences": stop_sequences, "system": system,
                "thinking": thinking, "tool_choice": tool_choice, "tools": tools}
        opts = request_options(extra_headers=_parse_headers(omit, omit, extra_headers), extra_query=extra_query, extra_body=extra_body,
                               timeout=timeout)
        return _parse_response(c.post("/v1/messages", body=body, options=opts, cast_to=Message), output_format)

    def stream(self, *, max_tokens, messages, model, cache_control=omit, diagnostics=omit, inference_geo=omit, metadata=omit, output_config=omit,
               output_format=omit, container=omit, service_tier=omit, speed=omit, stop_sequences=omit, system=omit, thinking=omit,
               tool_choice=omit, tools=omit, user_profile_id=omit, workspace_id=omit, extra_headers=None, extra_query=None, extra_body=None,
               timeout=not_given):
        output_config = _merge_config(output_config, _json_format(output_format))
        body = _stream_body(max_tokens, messages, model, cache_control, diagnostics, inference_geo, metadata, output_config, container,
                            service_tier, speed, stop_sequences, system, tools, thinking, tool_choice)
        opts = request_options(extra_headers=_headers(user_profile_id, workspace_id, extra_headers), extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)
        c = self._client

        def make_request():
            return c.post("/v1/messages", body=body, options=opts, cast_to=Message, stream=True, stream_cls=(Stream, _EVENT))

        return MessageStreamManager(make_request, output_format)

    def count_tokens(self, *, messages, model, cache_control=omit, output_config=omit, output_format=omit, speed=omit, system=omit, thinking=omit,
                     tool_choice=omit, tools=omit, user_profile_id=omit, workspace_id=omit, extra_headers=None, extra_query=None, extra_body=None,
                     timeout=not_given):
        output_config = _merge_config(output_config, _json_format(output_format))
        body = _count_body(messages, model, cache_control, output_config, speed, system, thinking, tool_choice, tools)
        opts = request_options(extra_headers=_headers(user_profile_id, workspace_id, extra_headers), extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)
        return self._client.post("/v1/messages/count_tokens", body=body, options=opts, cast_to=MessageTokensCount)


class AsyncMessages:
    def __init__(self, client):
        self._client = client

    async def create(self, *, max_tokens=_NO, messages=_NO, model=_NO, cache_control=omit, container=omit, diagnostics=omit, inference_geo=omit,
                     metadata=omit, output_config=omit, service_tier=omit, speed=omit, stop_sequences=omit, stream=omit, system=omit,
                     thinking=omit, tool_choice=omit, tools=omit, user_profile_id=omit, workspace_id=omit, extra_headers=None, extra_query=None,
                     extra_body=None, timeout=not_given):
        _missing(max_tokens, messages, model)
        c = self._client
        if not stream and not is_given(timeout) and c.timeout == DEFAULT_TIMEOUT:
            timeout = c._calculate_nonstreaming_timeout(max_tokens, MODEL_NONSTREAMING_TOKENS.get(model, None))
        body = _create_body(max_tokens, messages, model, cache_control, container, diagnostics, inference_geo, metadata, output_config,
                            service_tier, speed, stop_sequences, stream, system, thinking, tool_choice, tools)
        opts = request_options(extra_headers=_headers(user_profile_id, workspace_id, extra_headers), extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)
        return await c.post("/v1/messages", body=body, options=opts, cast_to=Message, stream=bool(stream), stream_cls=(AsyncStream, _EVENT))

    async def parse(self, *, max_tokens, messages, model, diagnostics=omit, metadata=omit, output_config=omit, output_format=omit, service_tier=omit,
              speed=omit, stop_sequences=omit, system=omit, thinking=omit, tool_choice=omit, tools=omit, extra_headers=None, extra_query=None,
              extra_body=None, timeout=not_given):
        c = self._client
        if not is_given(timeout) and c.timeout == DEFAULT_TIMEOUT:
            timeout = c._calculate_nonstreaming_timeout(max_tokens, MODEL_NONSTREAMING_TOKENS.get(model, None))
        output_config = _merge_config(output_config, _json_format(output_format))
        body = {"max_tokens": max_tokens, "messages": messages, "model": model, "diagnostics": diagnostics, "metadata": metadata,
                "output_config": output_config, "service_tier": service_tier, "speed": speed, "stop_sequences": stop_sequences, "system": system,
                "thinking": thinking, "tool_choice": tool_choice, "tools": tools}
        opts = request_options(extra_headers=_parse_headers(omit, omit, extra_headers), extra_query=extra_query, extra_body=extra_body,
                               timeout=timeout)
        return _parse_response(await c.post("/v1/messages", body=body, options=opts, cast_to=Message), output_format)

    def stream(self, *, max_tokens, messages, model, cache_control=omit, diagnostics=omit, inference_geo=omit, metadata=omit, output_config=omit,
               output_format=omit, container=omit, service_tier=omit, speed=omit, stop_sequences=omit, system=omit, thinking=omit,
               tool_choice=omit, tools=omit, user_profile_id=omit, workspace_id=omit, extra_headers=None, extra_query=None, extra_body=None,
               timeout=not_given):
        output_config = _merge_config(output_config, _json_format(output_format))
        body = _stream_body(max_tokens, messages, model, cache_control, diagnostics, inference_geo, metadata, output_config, container,
                            service_tier, speed, stop_sequences, system, tools, thinking, tool_choice)
        opts = request_options(extra_headers=_headers(user_profile_id, workspace_id, extra_headers), extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)
        return AsyncMessageStreamManager(self._client.post("/v1/messages", body=body, options=opts, cast_to=Message, stream=True,
                                                           stream_cls=(AsyncStream, _EVENT)), output_format)

    async def count_tokens(self, *, messages, model, cache_control=omit, output_config=omit, output_format=omit, speed=omit, system=omit,
                           thinking=omit, tool_choice=omit, tools=omit, user_profile_id=omit, workspace_id=omit, extra_headers=None,
                           extra_query=None, extra_body=None, timeout=not_given):
        output_config = _merge_config(output_config, _json_format(output_format))
        body = _count_body(messages, model, cache_control, output_config, speed, system, thinking, tool_choice, tools)
        opts = request_options(extra_headers=_headers(user_profile_id, workspace_id, extra_headers), extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)
        return await self._client.post("/v1/messages/count_tokens", body=body, options=opts, cast_to=MessageTokensCount)


def _list_query(after_id, before_id, lifecycle, limit):
    return {"after_id": after_id, "before_id": before_id, "lifecycle": lifecycle, "limit": limit}


class Models:
    def __init__(self, client):
        self._client = client

    def retrieve(self, model_id, *, workspace_id=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not model_id:
            raise ValueError("Expected a non-empty value for `model_id` but received %r" % (model_id,))
        opts = request_options(extra_headers=_headers(omit, workspace_id, extra_headers), extra_query=extra_query, extra_body=extra_body, timeout=timeout)
        return self._client.get("/v1/models/" + _brevi_sdk.quote_segment(model_id), options=opts, cast_to=ModelInfo)

    def list(self, *, after_id=omit, before_id=omit, lifecycle=omit, limit=omit, workspace_id=omit, extra_headers=None, extra_query=None,
             extra_body=None, timeout=not_given):
        opts = request_options(query=_list_query(after_id, before_id, lifecycle, limit), extra_headers=_headers(omit, workspace_id, extra_headers),
                               extra_query=extra_query, extra_body=extra_body, timeout=timeout)
        return self._client.get_api_list("/v1/models", page=_ModelsPage, model=ModelInfo, options=opts)


class AsyncModels:
    def __init__(self, client):
        self._client = client

    async def retrieve(self, model_id, *, workspace_id=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not model_id:
            raise ValueError("Expected a non-empty value for `model_id` but received %r" % (model_id,))
        opts = request_options(extra_headers=_headers(omit, workspace_id, extra_headers), extra_query=extra_query, extra_body=extra_body, timeout=timeout)
        return await self._client.get("/v1/models/" + _brevi_sdk.quote_segment(model_id), options=opts, cast_to=ModelInfo)

    def list(self, *, after_id=omit, before_id=omit, lifecycle=omit, limit=omit, workspace_id=omit, extra_headers=None, extra_query=None,
             extra_body=None, timeout=not_given):
        opts = request_options(query=_list_query(after_id, before_id, lifecycle, limit), extra_headers=_headers(omit, workspace_id, extra_headers),
                               extra_query=extra_query, extra_body=extra_body, timeout=timeout)
        return self._client.get_api_list("/v1/models", page=_AsyncModelsPage, model=ModelInfo, options=opts)


# ---------- message streams ----------

def _partial_json(b):
    # jiter.from_json(b, partial_mode=True): the value the complete part of b gives, dropping an unfinished string
    s = b.decode("utf-8")
    p = _PJ(s)
    p.ws()
    v = p.value()
    if v is _PJ.CUT:
        raise ValueError("EOF while parsing a value at line 1 column %d" % len(s))
    p.ws()
    if p.i < len(s):
        raise ValueError("trailing characters at line 1 column %d" % (p.i + 1))
    return v


class _PJ:
    CUT = object()

    def __init__(self, s):
        self.s = s
        self.i = 0
        self.n = len(s)

    def ws(self):
        while self.i < self.n and self.s[self.i] in " \t\r\n":
            self.i += 1

    def value(self):
        self.ws()
        if self.i >= self.n:
            return _PJ.CUT
        c = self.s[self.i]
        if c == "{":
            return self.obj()
        if c == "[":
            return self.arr()
        if c == '"':
            return self.string()
        if c in "-0123456789":
            return self.number()
        for w, v in (("true", True), ("false", False), ("null", None)):
            if self.s.startswith(w, self.i):
                self.i += len(w)
                return v
            if w.startswith(self.s[self.i:]):
                self.i = self.n
                return _PJ.CUT
        raise ValueError("expected value at line 1 column %d" % (self.i + 1))

    def obj(self):
        self.i += 1
        out = {}
        self.ws()
        if self.i < self.n and self.s[self.i] == "}":
            self.i += 1
            return out
        while True:
            self.ws()
            if self.i >= self.n:
                return out
            if self.s[self.i] != '"':
                raise ValueError("key must be a string at line 1 column %d" % (self.i + 1))
            k = self.string()
            if k is _PJ.CUT:
                return out
            self.ws()
            if self.i >= self.n:
                return out
            if self.s[self.i] != ":":
                raise ValueError("expected `:` at line 1 column %d" % (self.i + 1))
            self.i += 1
            v = self.value()
            if v is _PJ.CUT:
                return out
            out[k] = v
            self.ws()
            if self.i >= self.n:
                return out
            c = self.s[self.i]
            self.i += 1
            if c == "}":
                return out
            if c != ",":
                raise ValueError("expected `,` or `}` at line 1 column %d" % self.i)

    def arr(self):
        self.i += 1
        out = []
        self.ws()
        if self.i < self.n and self.s[self.i] == "]":
            self.i += 1
            return out
        while True:
            v = self.value()
            if v is _PJ.CUT:
                return out
            out.append(v)
            self.ws()
            if self.i >= self.n:
                return out
            c = self.s[self.i]
            self.i += 1
            if c == "]":
                return out
            if c != ",":
                raise ValueError("expected `,` or `]` at line 1 column %d" % self.i)

    def string(self):
        j = self.i + 1
        s = self.s
        while j < self.n:
            c = s[j]
            if c == "\\":
                j += 2
                continue
            if c == '"':
                v = json.loads(s[self.i:j + 1])
                self.i = j + 1
                return v
            j += 1
        self.i = self.n
        return _PJ.CUT

    def number(self):
        j = self.i
        s = self.s
        while j < self.n and s[j] in "+-0123456789.eE":
            j += 1
        t = s[self.i:j]
        self.i = j
        try:
            if "." in t or "e" in t or "E" in t:
                return float(t)
            return int(t)
        except ValueError:
            if j >= self.n:
                return _PJ.CUT
            raise ValueError("invalid number at line 1 column %d" % j)


def accumulate_event(event, current_snapshot, json_bufs):
    if current_snapshot is None:
        if event.type == "message_start":
            return build(ParsedMessage, **event.message.to_dict())
        raise RuntimeError('Unexpected event order, got %s before "message_start"' % event.type)
    t = event.type
    if t == "content_block_start":
        current_snapshot.content.append(_brevi_sdk.construct_type(event.content_block.to_dict(), _PARSED_BLOCK))
    elif t == "content_block_delta":
        content = current_snapshot.content[event.index]
        dt = event.delta.type
        if dt == "text_delta":
            if content.type == "text":
                content.text += event.delta.text
        elif dt == "input_json_delta":
            if isinstance(content, (ToolUseBlock, ServerToolUseBlock)):
                buf = json_bufs.get(event.index, b"")
                buf += event.delta.partial_json.encode("utf-8")
                if buf:
                    try:
                        content.input = _partial_json(buf)
                    except ValueError as e:
                        raise ValueError(
                            "Unable to parse tool parameter JSON from model. Please retry your request or adjust your prompt. Error: %s. JSON: %s"
                            % (e, buf.decode("utf-8")))
                json_bufs[event.index] = buf
        elif dt == "citations_delta":
            if content.type == "text":
                if not content.citations:
                    content.citations = [event.delta.citation]
                else:
                    content.citations.append(event.delta.citation)
        elif dt == "thinking_delta":
            if content.type == "thinking":
                content.thinking += event.delta.thinking
        elif dt == "signature_delta":
            if content.type == "thinking":
                content.signature = event.delta.signature
    elif t == "message_delta":
        s = current_snapshot
        s.stop_reason = event.delta.stop_reason
        s.stop_sequence = event.delta.stop_sequence
        s.stop_details = event.delta.stop_details
        if event.delta.container is not None:
            s.container = event.delta.container
        u = event.usage
        s.usage.output_tokens = u.output_tokens
        if u.input_tokens is not None:
            s.usage.input_tokens = u.input_tokens
        if u.cache_creation_input_tokens is not None:
            s.usage.cache_creation_input_tokens = u.cache_creation_input_tokens
        if u.cache_read_input_tokens is not None:
            s.usage.cache_read_input_tokens = u.cache_read_input_tokens
        if u.server_tool_use is not None:
            s.usage.server_tool_use = u.server_tool_use
        if u.output_tokens_details is not None:
            s.usage.output_tokens_details = u.output_tokens_details
    return current_snapshot


def build_events(event, snapshot):
    out = []
    t = event.type
    if t == "message_start" or t == "message_delta" or t == "content_block_start":
        out.append(event)
    elif t == "message_stop":
        out.append(build(_ty.ParsedMessageStopEvent, type="message_stop", message=snapshot))
    elif t == "content_block_delta":
        out.append(event)
        block = snapshot.content[event.index]
        dt = event.delta.type
        if dt == "text_delta":
            if block.type == "text":
                out.append(build(TextEvent, type="text", text=event.delta.text, snapshot=block.text))
        elif dt == "input_json_delta":
            if isinstance(block, (ToolUseBlock, ServerToolUseBlock)):
                out.append(build(InputJsonEvent, type="input_json", partial_json=event.delta.partial_json, snapshot=block.input))
        elif dt == "citations_delta":
            if block.type == "text":
                out.append(build(CitationEvent, type="citation", citation=event.delta.citation, snapshot=block.citations or []))
        elif dt == "thinking_delta":
            if block.type == "thinking":
                out.append(build(ThinkingEvent, type="thinking", thinking=event.delta.thinking, snapshot=block.thinking))
        elif dt == "signature_delta":
            if block.type == "thinking":
                out.append(build(SignatureEvent, type="signature", signature=block.signature))
    elif t == "content_block_stop":
        out.append(build(_ty.ParsedContentBlockStopEvent, type="content_block_stop", index=event.index, content_block=snapshot.content[event.index]))
    else:
        out.append(event)
    return out


def _final_text(message):
    texts = []
    for block in message.content:
        if block.type == "text":
            texts.append(block.text)
    if not texts:
        raise RuntimeError(
            ".get_final_text() can only be called when the API returns a `text` content block.\nThe API returned %s content block type(s) that you can access by calling get_final_message().content"
            % ",".join([b.type for b in message.content]))
    return "".join(texts)


class MessageStream:
    def __init__(self, raw_stream, output_format=not_given):
        self._raw_stream = raw_stream
        self._output_format = output_format
        self._snapshot = None
        self._json_bufs = {}
        self._iterator = self._stream()
        self.text_stream = self._stream_text()

    @property
    def response(self):
        return self._raw_stream.response

    @property
    def request_id(self):
        return self.response.headers.get("request-id")

    @property
    def workspace_id(self):
        return self.response.headers.get("anthropic-workspace-id")

    def __next__(self):
        return next(self._iterator)

    def __iter__(self):
        for item in self._iterator:
            yield item

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def close(self):
        self._raw_stream.close()

    def get_final_message(self):
        self.until_done()
        return self._snapshot

    def get_final_text(self):
        return _final_text(self.get_final_message())

    def until_done(self):
        for _ in self:
            pass

    @property
    def current_message_snapshot(self):
        return self._snapshot

    def _stream(self):
        for ev in self._raw_stream:
            if self._snapshot is None and ev.type != "message_start":
                yield ev
                continue
            self._snapshot = accumulate_event(ev, self._snapshot, self._json_bufs)
            if ev.type == "content_block_stop" and is_given(self._output_format):
                block = self._snapshot.content[ev.index]
                if block.type == "text":
                    block.parsed_output = _parse_text(block.text, self._output_format)
            for e in build_events(ev, self._snapshot):
                yield e

    def _stream_text(self):
        for chunk in self:
            if chunk.type == "content_block_delta" and chunk.delta.type == "text_delta":
                yield chunk.delta.text


class MessageStreamManager:
    def __init__(self, api_request, output_format=not_given):
        self._stream = None
        self._api_request = api_request
        self._output_format = output_format

    def __enter__(self):
        self._stream = MessageStream(self._api_request(), self._output_format)
        return self._stream

    def __exit__(self, exc_type, exc, tb):
        if self._stream is not None:
            self._stream.close()


class AsyncMessageStream:
    def __init__(self, raw_stream, output_format=not_given):
        self._raw_stream = raw_stream
        self._output_format = output_format
        self._snapshot = None
        self._json_bufs = {}
        self._iterator = self._stream()
        self.text_stream = self._stream_text()

    @property
    def response(self):
        return self._raw_stream.response

    @property
    def request_id(self):
        return self.response.headers.get("request-id")

    @property
    def workspace_id(self):
        return self.response.headers.get("anthropic-workspace-id")

    async def __anext__(self):
        return await self._iterator.__anext__()

    async def __aiter__(self):
        async for item in self._iterator:
            yield item

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        await self.close()

    async def close(self):
        await self._raw_stream.close()

    async def get_final_message(self):
        await self.until_done()
        return self._snapshot

    async def get_final_text(self):
        return _final_text(await self.get_final_message())

    async def until_done(self):
        async for _ in self:
            pass

    @property
    def current_message_snapshot(self):
        return self._snapshot

    async def _stream(self):
        async for ev in self._raw_stream:
            if self._snapshot is None and ev.type != "message_start":
                yield ev
                continue
            self._snapshot = accumulate_event(ev, self._snapshot, self._json_bufs)
            if ev.type == "content_block_stop" and is_given(self._output_format):
                block = self._snapshot.content[ev.index]
                if block.type == "text":
                    block.parsed_output = _parse_text(block.text, self._output_format)
            for e in build_events(ev, self._snapshot):
                yield e

    async def _stream_text(self):
        async for chunk in self:
            if chunk.type == "content_block_delta" and chunk.delta.type == "text_delta":
                yield chunk.delta.text


class AsyncMessageStreamManager:
    def __init__(self, api_request, output_format=not_given):
        self._stream = None
        self._api_request = api_request
        self._output_format = output_format

    async def __aenter__(self):
        self._stream = AsyncMessageStream(await self._api_request, self._output_format)
        return self._stream

    async def __aexit__(self, exc_type, exc, tb):
        if self._stream is not None:
            await self._stream.close()
