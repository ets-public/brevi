# Brevi's openai: the Python SDK's API (openai 3.46) for what agent programs use: OpenAI and AsyncOpenAI,
# chat.completions.create (also stream=True, tools), embeddings.create (the SDK's base64 default decoded as float32),
# models.list / retrieve, responses.create (also stream=True) with Response.output_text, with_options / copy, the
# SDK's errors, retries and request headers, and structured outputs: chat.completions.parse / .stream and
# responses.parse / .stream with pydantic_function_tool, built on Brevi's pydantic. Response objects are the SDK's own models
# (src/dynpy/py/_brevi_openai_types.py, written from the SDK by tools/gen_llm_types.py). It runs over Brevi's httpx.
# caps.rs lists what it has (OWN_OPENAI) and what sends a program to CPython's openai instead (OPENAI_LACKS: beta,
# audio, images, files, realtime, providers, raw responses, ...).
# Its behaviour follows openai-python (Copyright OpenAI; Apache-2.0 licensed, as Brevi is), whose error messages it repeats.
import base64 as _b64
import json as _json

import _brevi_http
import _brevi_sdk
import httpx
import pydantic as _pyd
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
    is_given,
    page_class,
)
import openai.types as types
from _brevi_openai_types import ChatCompletion, ChatCompletionChunk, CreateEmbeddingResponse, Model, Response
import _brevi_openai_types as _ty

__title__ = "openai"
__version__ = "3.46.0"
VERSION = __version__
DEFAULT_MAX_RETRIES = 2
DEFAULT_TIMEOUT = httpx.Timeout(timeout=600, connect=5.0)
DEFAULT_CONNECTION_LIMITS = httpx.Limits(max_connections=1000, max_keepalive_connections=100)
Timeout = httpx.Timeout


def _output_text(self):
    texts = []
    for output in self.output:
        if output.type == "message":
            for content in output.content:
                if content.type == "output_text":
                    if content.text is not None:
                        texts.append(content.text)
    return "".join(texts)


Response.output_text = property(_output_text)


def _output_parsed(self):
    for output in self.output:
        if output.type == "message":
            for content in output.content:
                if content.type == "output_text" and content.parsed:
                    return content.parsed
    return None


_ty.ParsedResponse.output_parsed = property(_output_parsed)


# ---------- errors ----------

class OpenAIError(Exception):
    pass


class APIError(OpenAIError):
    code = None
    param = None

    def __init__(self, message, request, *, body):
        Exception.__init__(self, message)
        self.request = request
        self.message = message
        self.body = body
        if isinstance(body, dict):
            code = body.get("code")
            self.code = str(code) if code is not None else None
            self.param = body.get("param")
            self.type = body.get("type")
        else:
            self.code = None
            self.param = None
            self.type = None


class APIResponseValidationError(APIError):
    def __init__(self, response, body, *, message=None):
        APIError.__init__(self, message or "Data returned by API invalid for expected schema.", response.request, body=body)
        self.response = response
        self.status_code = response.status_code


class APIStatusError(APIError):
    def __init__(self, message, *, response, body):
        APIError.__init__(self, message, response.request, body=body)
        self.response = response
        self.status_code = response.status_code
        self.request_id = response.headers.get("x-request-id")


class APIConnectionError(APIError):
    def __init__(self, *, message="Connection error.", request):
        APIError.__init__(self, message, request, body=None)


class APITimeoutError(APIConnectionError):
    def __init__(self, request):
        APIConnectionError.__init__(self, message="Request timed out.", request=request)


class BadRequestError(APIStatusError):
    status_code = 400


class AuthenticationError(APIStatusError):
    status_code = 401


class PermissionDeniedError(APIStatusError):
    status_code = 403


class NotFoundError(APIStatusError):
    status_code = 404


class ConflictError(APIStatusError):
    status_code = 409


class UnprocessableEntityError(APIStatusError):
    status_code = 422


class RateLimitError(APIStatusError):
    status_code = 429


class InternalServerError(APIStatusError):
    pass


class LengthFinishReasonError(OpenAIError):
    def __init__(self, *, completion):
        msg = "Could not parse response content as the length limit was reached"
        if completion.usage:
            msg += " - %s" % (completion.usage,)
        Exception.__init__(self, msg)
        self.completion = completion


class ContentFilterFinishReasonError(OpenAIError):
    def __init__(self, *, completion=None):
        Exception.__init__(self, "Could not parse response content as the request was rejected by the content filter")
        self.completion = completion


class InvalidWebhookSignatureError(ValueError):
    pass


_STATUS = {400: BadRequestError, 401: AuthenticationError, 403: PermissionDeniedError, 404: NotFoundError, 409: ConflictError,
           422: UnprocessableEntityError, 429: RateLimitError}


# ---------- pages ----------

class SyncPage(_brevi_sdk.BasePage):
    def next_page_info(self):
        return None


class AsyncPage(_brevi_sdk.AsyncBasePage):
    def next_page_info(self):
        return None


_ModelsPage = page_class(SyncPage, "SyncPage[Model]", "Model", [("object", "str", ("r",))], "openai.pagination")
_AsyncModelsPage = page_class(AsyncPage, "AsyncPage[Model]", "Model", [("object", "str", ("r",))], "openai.pagination")

_CHUNK = ("M", "ChatCompletionChunk")
_EVENT = _ty._UNIONS["ResponseStreamEvent"]


# ---------- clients ----------

def _env(name):
    return _brevi_http.env_exact(name)


def _stream_error(data):
    message = None
    error = data.get("error")
    if isinstance(error, dict):
        message = error.get("message")
    if not message or not isinstance(message, str):
        message = "An error occurred during streaming"
    return message


class _Common:
    _sdk_error = OpenAIError
    _array_format = "brackets"
    _none_retries_msg = "max_retries cannot be None"

    def _init(self, api_key, admin_api_key, organization, project, webhook_secret, base_url, websocket_base_url, timeout, max_retries,
              default_headers, default_query, http_client):
        if api_key is None:
            api_key = _env("OPENAI_API_KEY")
        if callable(api_key):
            raise TypeError("Brevi's openai has no api_key callables")
        self.api_key = api_key or ""
        if admin_api_key is None:
            admin_api_key = _env("OPENAI_ADMIN_KEY")
        self.admin_api_key = admin_api_key
        if not self.api_key and self.admin_api_key is None:
            raise OpenAIError(
                "Missing credentials. Please pass an `api_key`, `workload_identity`, `admin_api_key`, or set the `OPENAI_API_KEY` or `OPENAI_ADMIN_KEY` environment variable."
            )
        self.workload_identity = None
        if organization is None:
            organization = _env("OPENAI_ORG_ID")
        self.organization = organization
        if project is None:
            project = _env("OPENAI_PROJECT_ID")
        self.project = project
        if webhook_secret is None:
            webhook_secret = _env("OPENAI_WEBHOOK_SECRET")
        self.webhook_secret = webhook_secret
        self.websocket_base_url = websocket_base_url
        if base_url is None or isinstance(base_url, NotGiven):
            base_url = _env("OPENAI_BASE_URL")
        if base_url is None:
            base_url = "https://api.openai.com/v1"
        hv = _env("OPENAI_CUSTOM_HEADERS")
        if hv is not None:
            parsed = {}
            for line in hv.split("\n"):
                colon = line.find(":")
                if colon >= 0:
                    parsed[line[:colon].strip()] = line[colon + 1:].strip()
            explicit = default_headers or {}
            for k in explicit:
                if k.lower() == "authorization":
                    parsed = {n: v for n, v in parsed.items() if n.lower() != "authorization"}
            for k, v in explicit.items():
                parsed[k] = v
            default_headers = parsed
        self._setup(version=__version__, base_url=base_url, max_retries=max_retries, timeout=timeout, custom_headers=default_headers,
                    custom_query=default_query, default_timeout=DEFAULT_TIMEOUT)
        self._init_http(http_client)
        a = self._is_async
        self.chat = _Chat(self, a)
        self.embeddings = (AsyncEmbeddings if a else Embeddings)(self)
        self.models = (AsyncModels if a else Models)(self)
        self.responses = (AsyncResponses if a else Responses)(self)

    @property
    def auth_headers(self):
        if not self.api_key:
            return {}
        return {"Authorization": "Bearer " + self.api_key}

    @property
    def default_headers(self):
        h = self._base_headers()
        h["X-Stainless-Async"] = "async:asyncio" if self._is_async else "false"
        if self.organization is not None:
            h["OpenAI-Organization"] = self.organization
        if self.project is not None:
            h["OpenAI-Project"] = self.project
        for k, v in self._custom_headers.items():
            h[k] = v
        return h

    def _validate_headers(self, headers):
        for k in headers:
            if k.lower() == "authorization":
                return
        for k, v in self._custom_headers.items():
            if k.lower() == "authorization" and isinstance(v, Omit):
                return
        raise TypeError(
            "Could not resolve authentication method. Expected either api_key or admin_api_key to be set. Or for the `Authorization` header to be explicitly omitted."
        )

    def _status_error_from_response(self, response):
        err_msg = "Error code: %d" % response.status_code
        if response.is_closed and not response.is_stream_consumed:
            body = None
        else:
            text = response.text.strip()
            body = text
            try:
                body = _json.loads(text)
            except Exception:
                pass
            else:
                error = body.get("error", body) if isinstance(body, dict) else None
                message = error.get("message") if isinstance(error, dict) else None
                if isinstance(message, str) and message:
                    err_msg += " - " + message
        return self._make_status_error(err_msg, body=body, response=response)

    def _make_status_error(self, err_msg, *, body, response):
        data = body.get("error", body) if isinstance(body, dict) else body
        cls = _STATUS.get(response.status_code)
        if cls is None:
            cls = InternalServerError if response.status_code >= 500 else APIStatusError
        return cls(err_msg, response=response, body=data)

    def _sse_item(self, sse, stream):
        # None at the end of the stream ([DONE])
        if sse.data.startswith("[DONE]"):
            return None
        data = sse.json()
        if isinstance(data, dict) and data.get("error"):
            raise APIError(message=_stream_error(data), request=stream.response.request, body=data["error"])
        return self._process_response_data(data=data, cast_to=stream._cast_to, response=stream.response)

    def _timeout_error(self, request):
        return APITimeoutError(request=request)

    def _connection_error(self, request):
        return APIConnectionError(request=request)

    def copy(self, *, api_key=None, admin_api_key=None, workload_identity=None, provider=NOT_GIVEN, organization=None, project=None,
             webhook_secret=None, websocket_base_url=None, base_url=NOT_GIVEN, data_residency=None, timeout=NOT_GIVEN, http_client=None,
             max_retries=NOT_GIVEN, default_headers=None, set_default_headers=None, default_query=None, set_default_query=None,
             _enforce_credentials=None, _extra_kwargs={}):
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
        return type(self)(
            api_key=api_key or self.api_key or None,
            admin_api_key=admin_api_key or self.admin_api_key,
            organization=organization or self.organization,
            project=project or self.project,
            webhook_secret=webhook_secret or self.webhook_secret,
            websocket_base_url=websocket_base_url or self.websocket_base_url,
            base_url=self.base_url if not is_given(base_url) or base_url is None else base_url,
            timeout=self.timeout if isinstance(timeout, NotGiven) else timeout,
            http_client=http_client or self._client,
            max_retries=max_retries if is_given(max_retries) else self.max_retries,
            default_headers=headers,
            default_query=params,
        )

    with_options = copy


class OpenAI(_Common, _brevi_sdk.SyncClient):
    _is_async = False

    def __init__(self, *, api_key=None, admin_api_key=None, workload_identity=None, organization=None, project=None, webhook_secret=None,
                 provider=None, base_url=not_given, data_residency=None, websocket_base_url=None, timeout=not_given,
                 max_retries=DEFAULT_MAX_RETRIES, default_headers=None, default_query=None, http_client=None, _strict_response_validation=False,
                 _enforce_credentials=True):
        if workload_identity is not None or provider is not None or data_residency is not None:
            raise TypeError("Brevi's openai has no workload_identity, provider or data_residency")
        self._init(api_key, admin_api_key, organization, project, webhook_secret, base_url, websocket_base_url, timeout, max_retries,
                   default_headers, default_query, http_client)

    def _stream_items(self, stream):
        response = stream.response
        for sse in stream._iter_events():
            item = self._sse_item(sse, stream)
            if item is None:
                break
            yield item
        response.close()


class AsyncOpenAI(_Common, _brevi_sdk.AsyncClientBase):
    _is_async = True

    def __init__(self, *, api_key=None, admin_api_key=None, workload_identity=None, organization=None, project=None, webhook_secret=None,
                 provider=None, base_url=not_given, data_residency=None, websocket_base_url=None, timeout=not_given,
                 max_retries=DEFAULT_MAX_RETRIES, default_headers=None, default_query=None, http_client=None, _strict_response_validation=False,
                 _enforce_credentials=True):
        if workload_identity is not None or provider is not None or data_residency is not None:
            raise TypeError("Brevi's openai has no workload_identity, provider or data_residency")
        self._init(api_key, admin_api_key, organization, project, webhook_secret, base_url, websocket_base_url, timeout, max_retries,
                   default_headers, default_query, http_client)

    async def _stream_items(self, stream):
        response = stream.response
        async for sse in stream._iter_events():
            item = self._sse_item(sse, stream)
            if item is None:
                break
            yield item
        await response.aclose()


Client = OpenAI
AsyncClient = AsyncOpenAI


# ---------- resources ----------

def _chat_body(model, stream, service_tier, messages, audio, frequency_penalty, function_call, functions, logit_bias, logprobs,
               max_completion_tokens, max_tokens, metadata, modalities, moderation, n, parallel_tool_calls, prediction, presence_penalty,
               prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning_effort, response_format, safety_identifier, seed, stop,
               store, stream_options, temperature, tool_choice, tools, top_logprobs, top_p, user, verbosity, web_search_options):
    if is_given(response_format) and isinstance(response_format, type):
        raise TypeError(
            "You tried to pass a `BaseModel` class to `chat.completions.create()`; You must use `chat.completions.parse()` instead"
        )
    return {
        "model": model, "stream": stream, "service_tier": service_tier, "messages": messages, "audio": audio,
        "frequency_penalty": frequency_penalty, "function_call": function_call, "functions": functions, "logit_bias": logit_bias,
        "logprobs": logprobs, "max_completion_tokens": max_completion_tokens, "max_tokens": max_tokens, "metadata": metadata,
        "modalities": modalities, "moderation": moderation, "n": n, "parallel_tool_calls": parallel_tool_calls, "prediction": prediction,
        "presence_penalty": presence_penalty, "prompt_cache_key": prompt_cache_key, "prompt_cache_options": prompt_cache_options,
        "prompt_cache_retention": prompt_cache_retention, "reasoning_effort": reasoning_effort, "response_format": response_format,
        "safety_identifier": safety_identifier, "seed": seed, "stop": stop, "store": store, "stream_options": stream_options,
        "temperature": temperature, "tool_choice": tool_choice, "tools": tools, "top_logprobs": top_logprobs, "top_p": top_p, "user": user,
        "verbosity": verbosity, "web_search_options": web_search_options,
    }


def _opts(extra_headers, extra_query, extra_body, timeout, post_parser=None):
    return request_options(extra_headers=extra_headers, extra_query=extra_query, extra_body=extra_body, timeout=timeout, post_parser=post_parser)


class _Chat:
    def __init__(self, client, is_async):
        self.completions = (AsyncCompletions if is_async else Completions)(client)


class Completions:
    def __init__(self, client):
        self._client = client

    def create(self, *, messages, model, audio=omit, frequency_penalty=omit, function_call=omit, functions=omit, logit_bias=omit, logprobs=omit,
               max_completion_tokens=omit, max_tokens=omit, metadata=omit, modalities=omit, moderation=omit, n=omit, parallel_tool_calls=omit,
               prediction=omit, presence_penalty=omit, prompt_cache_key=omit, prompt_cache_options=omit, prompt_cache_retention=omit,
               reasoning_effort=omit, response_format=omit, safety_identifier=omit, seed=omit, service_tier=omit, stop=omit, store=omit,
               stream=omit, stream_options=omit, temperature=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit, user=omit,
               verbosity=omit, web_search_options=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        body = _chat_body(model, stream, service_tier, messages, audio, frequency_penalty, function_call, functions, logit_bias, logprobs,
                          max_completion_tokens, max_tokens, metadata, modalities, moderation, n, parallel_tool_calls, prediction,
                          presence_penalty, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning_effort, response_format,
                          safety_identifier, seed, stop, store, stream_options, temperature, tool_choice, tools, top_logprobs, top_p, user,
                          verbosity, web_search_options)
        return self._client.post("/chat/completions", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout),
                                 cast_to=ChatCompletion, stream=bool(stream), stream_cls=(Stream, _CHUNK))

    def parse(self, *, messages, model, audio=omit, response_format=omit, frequency_penalty=omit, function_call=omit, functions=omit,
              logit_bias=omit, logprobs=omit, max_completion_tokens=omit, max_tokens=omit, metadata=omit, modalities=omit, moderation=omit,
              n=omit, parallel_tool_calls=omit, prediction=omit, presence_penalty=omit, prompt_cache_key=omit, prompt_cache_options=omit,
              prompt_cache_retention=omit, reasoning_effort=omit, safety_identifier=omit, seed=omit, service_tier=omit, stop=omit,
              store=omit, stream_options=omit, temperature=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit, user=omit,
              verbosity=omit, web_search_options=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        chat_completion_tools = _validate_input_tools(tools)
        headers = {"X-Stainless-Helper-Method": "chat.completions.parse"}
        headers.update(extra_headers or {})

        def parser(raw_completion):
            return _parse_chat_completion(response_format, chat_completion_tools, raw_completion)

        body = _chat_body(model, False, service_tier, messages, audio, frequency_penalty, function_call, functions, logit_bias, logprobs,
                          max_completion_tokens, max_tokens, metadata, modalities, moderation, n, parallel_tool_calls, prediction,
                          presence_penalty, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning_effort,
                          _type_to_response_format(response_format), safety_identifier, seed, stop, store, stream_options, temperature,
                          tool_choice, chat_completion_tools, top_logprobs, top_p, user, verbosity, web_search_options)
        return self._client.post("/chat/completions", body=body, options=_opts(headers, extra_query, extra_body, timeout, parser),
                                 cast_to=ChatCompletion)

    def stream(self, *, messages, model, audio=omit, response_format=omit, frequency_penalty=omit, function_call=omit, functions=omit,
               logit_bias=omit, logprobs=omit, max_completion_tokens=omit, max_tokens=omit, metadata=omit, modalities=omit, moderation=omit,
               n=omit, parallel_tool_calls=omit, prediction=omit, presence_penalty=omit, prompt_cache_key=omit, prompt_cache_options=omit,
               prompt_cache_retention=omit, reasoning_effort=omit, safety_identifier=omit, seed=omit, service_tier=omit, stop=omit,
               store=omit, stream_options=omit, temperature=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit, user=omit,
               verbosity=omit, web_search_options=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        chat_completion_tools = _materialize_input_tools(tools)
        headers = {"X-Stainless-Helper-Method": "chat.completions.stream"}
        headers.update(extra_headers or {})

        def api_request():
            return self.create(stream=True, messages=messages, model=model, audio=audio,
                               response_format=_type_to_response_format(response_format), frequency_penalty=frequency_penalty,
                               function_call=function_call, functions=functions, logit_bias=logit_bias, logprobs=logprobs,
                               max_completion_tokens=max_completion_tokens, max_tokens=max_tokens, metadata=metadata, modalities=modalities,
                               moderation=moderation, n=n, parallel_tool_calls=parallel_tool_calls, prediction=prediction,
                               presence_penalty=presence_penalty, prompt_cache_key=prompt_cache_key,
                               prompt_cache_options=prompt_cache_options, prompt_cache_retention=prompt_cache_retention,
                               reasoning_effort=reasoning_effort, safety_identifier=safety_identifier, seed=seed, service_tier=service_tier,
                               stop=stop, store=store, stream_options=stream_options, temperature=temperature, tool_choice=tool_choice,
                               tools=chat_completion_tools, top_logprobs=top_logprobs, top_p=top_p, user=user, verbosity=verbosity,
                               web_search_options=web_search_options, extra_headers=headers, extra_query=extra_query, extra_body=extra_body,
                               timeout=timeout)

        return ChatCompletionStreamManager(api_request, response_format=response_format, input_tools=chat_completion_tools)

    def retrieve(self, completion_id, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not completion_id:
            raise ValueError("Expected a non-empty value for `completion_id` but received %r" % (completion_id,))
        return self._client.get("/chat/completions/" + _brevi_sdk.quote_segment(completion_id),
                                options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=ChatCompletion)


class AsyncCompletions:
    def __init__(self, client):
        self._client = client

    async def create(self, *, messages, model, audio=omit, frequency_penalty=omit, function_call=omit, functions=omit, logit_bias=omit,
                     logprobs=omit, max_completion_tokens=omit, max_tokens=omit, metadata=omit, modalities=omit, moderation=omit, n=omit,
                     parallel_tool_calls=omit, prediction=omit, presence_penalty=omit, prompt_cache_key=omit, prompt_cache_options=omit,
                     prompt_cache_retention=omit, reasoning_effort=omit, response_format=omit, safety_identifier=omit, seed=omit,
                     service_tier=omit, stop=omit, store=omit, stream=omit, stream_options=omit, temperature=omit, tool_choice=omit, tools=omit,
                     top_logprobs=omit, top_p=omit, user=omit, verbosity=omit, web_search_options=omit, extra_headers=None, extra_query=None,
                     extra_body=None, timeout=not_given):
        body = _chat_body(model, stream, service_tier, messages, audio, frequency_penalty, function_call, functions, logit_bias, logprobs,
                          max_completion_tokens, max_tokens, metadata, modalities, moderation, n, parallel_tool_calls, prediction,
                          presence_penalty, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning_effort, response_format,
                          safety_identifier, seed, stop, store, stream_options, temperature, tool_choice, tools, top_logprobs, top_p, user,
                          verbosity, web_search_options)
        return await self._client.post("/chat/completions", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout),
                                       cast_to=ChatCompletion, stream=bool(stream), stream_cls=(AsyncStream, _CHUNK))

    async def parse(self, *, messages, model, audio=omit, response_format=omit, frequency_penalty=omit, function_call=omit, functions=omit,
                    logit_bias=omit, logprobs=omit, max_completion_tokens=omit, max_tokens=omit, metadata=omit, modalities=omit,
                    moderation=omit, n=omit, parallel_tool_calls=omit, prediction=omit, presence_penalty=omit, prompt_cache_key=omit,
                    prompt_cache_options=omit, prompt_cache_retention=omit, reasoning_effort=omit, safety_identifier=omit, seed=omit,
                    service_tier=omit, stop=omit, store=omit, stream_options=omit, temperature=omit, tool_choice=omit, tools=omit,
                    top_logprobs=omit, top_p=omit, user=omit, verbosity=omit, web_search_options=omit, extra_headers=None, extra_query=None,
                    extra_body=None, timeout=not_given):
        chat_completion_tools = _validate_input_tools(tools)
        headers = {"X-Stainless-Helper-Method": "chat.completions.parse"}
        headers.update(extra_headers or {})

        def parser(raw_completion):
            return _parse_chat_completion(response_format, chat_completion_tools, raw_completion)

        body = _chat_body(model, False, service_tier, messages, audio, frequency_penalty, function_call, functions, logit_bias, logprobs,
                          max_completion_tokens, max_tokens, metadata, modalities, moderation, n, parallel_tool_calls, prediction,
                          presence_penalty, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning_effort,
                          _type_to_response_format(response_format), safety_identifier, seed, stop, store, stream_options, temperature,
                          tool_choice, chat_completion_tools, top_logprobs, top_p, user, verbosity, web_search_options)
        return await self._client.post("/chat/completions", body=body, options=_opts(headers, extra_query, extra_body, timeout, parser),
                                 cast_to=ChatCompletion)

    def stream(self, *, messages, model, audio=omit, response_format=omit, frequency_penalty=omit, function_call=omit, functions=omit,
               logit_bias=omit, logprobs=omit, max_completion_tokens=omit, max_tokens=omit, metadata=omit, modalities=omit, moderation=omit,
               n=omit, parallel_tool_calls=omit, prediction=omit, presence_penalty=omit, prompt_cache_key=omit, prompt_cache_options=omit,
               prompt_cache_retention=omit, reasoning_effort=omit, safety_identifier=omit, seed=omit, service_tier=omit, stop=omit,
               store=omit, stream_options=omit, temperature=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit, user=omit,
               verbosity=omit, web_search_options=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        chat_completion_tools = _materialize_input_tools(tools)
        headers = {"X-Stainless-Helper-Method": "chat.completions.stream"}
        headers.update(extra_headers or {})
        api_request = self.create(stream=True, messages=messages, model=model, audio=audio,
                                  response_format=_type_to_response_format(response_format), frequency_penalty=frequency_penalty,
                                  function_call=function_call, functions=functions, logit_bias=logit_bias, logprobs=logprobs,
                                  max_completion_tokens=max_completion_tokens, max_tokens=max_tokens, metadata=metadata,
                                  modalities=modalities, moderation=moderation, n=n, parallel_tool_calls=parallel_tool_calls,
                                  prediction=prediction, presence_penalty=presence_penalty, prompt_cache_key=prompt_cache_key,
                                  prompt_cache_options=prompt_cache_options, prompt_cache_retention=prompt_cache_retention,
                                  reasoning_effort=reasoning_effort, safety_identifier=safety_identifier, seed=seed,
                                  service_tier=service_tier, stop=stop, store=store, stream_options=stream_options, temperature=temperature,
                                  tool_choice=tool_choice, tools=chat_completion_tools, top_logprobs=top_logprobs, top_p=top_p, user=user,
                                  verbosity=verbosity, web_search_options=web_search_options, extra_headers=headers,
                                  extra_query=extra_query, extra_body=extra_body, timeout=timeout)
        return AsyncChatCompletionStreamManager(api_request, response_format=response_format, input_tools=chat_completion_tools)

    async def retrieve(self, completion_id, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not completion_id:
            raise ValueError("Expected a non-empty value for `completion_id` but received %r" % (completion_id,))
        return await self._client.get("/chat/completions/" + _brevi_sdk.quote_segment(completion_id),
                                      options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=ChatCompletion)


def _floats(s):
    # little-endian float32s (array("f", ...).tolist() in the SDK), each exactly as a Python float
    b = _b64.b64decode(s)
    out = []
    for i in range(0, len(b) - 3, 4):
        u = b[i] | (b[i + 1] << 8) | (b[i + 2] << 16) | (b[i + 3] << 24)
        e = (u >> 23) & 0xFF
        m = u & 0x7FFFFF
        if e == 0xFF:
            v = float("nan") if m else float("inf")
        elif e == 0:
            v = m * 2.0 ** -149
        else:
            v = (m + 8388608) * 2.0 ** (e - 150)
        out.append(-v if u >> 31 else v)
    return out


def _embedding_request(model, input, dimensions, encoding_format, user):
    body = {"model": model, "input": input, "user": user, "dimensions": dimensions, "encoding_format": encoding_format}
    if not is_given(encoding_format):
        body["encoding_format"] = "base64"

        def parse(obj):
            if not obj.data:
                raise ValueError("No embedding data received")
            for e in obj.data:
                if isinstance(e.embedding, str):
                    e.embedding = _floats(e.embedding)
            return obj

        return body, parse
    return body, None


class Embeddings:
    def __init__(self, client):
        self._client = client

    def create(self, *, input, model, dimensions=omit, encoding_format=omit, user=omit, extra_headers=None, extra_query=None, extra_body=None,
               timeout=not_given):
        body, parse = _embedding_request(model, input, dimensions, encoding_format, user)
        return self._client.post("/embeddings", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout, parse),
                                 cast_to=CreateEmbeddingResponse)


class AsyncEmbeddings:
    def __init__(self, client):
        self._client = client

    async def create(self, *, input, model, dimensions=omit, encoding_format=omit, user=omit, extra_headers=None, extra_query=None,
                     extra_body=None, timeout=not_given):
        body, parse = _embedding_request(model, input, dimensions, encoding_format, user)
        return await self._client.post("/embeddings", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout, parse),
                                       cast_to=CreateEmbeddingResponse)


def _model_path(model):
    if not model:
        raise ValueError("Expected a non-empty value for `model` but received %r" % (model,))
    return "/models/" + _brevi_sdk.quote_segment(model)


class Models:
    def __init__(self, client):
        self._client = client

    def retrieve(self, model, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        return self._client.get(_model_path(model), options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=Model)

    def list(self, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        return self._client.get_api_list("/models", page=_ModelsPage, model=Model, options=_opts(extra_headers, extra_query, extra_body, timeout))


class AsyncModels:
    def __init__(self, client):
        self._client = client

    async def retrieve(self, model, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        return await self._client.get(_model_path(model), options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=Model)

    def list(self, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        return self._client.get_api_list("/models", page=_AsyncModelsPage, model=Model,
                                         options=_opts(extra_headers, extra_query, extra_body, timeout))


def _responses_body(model, stream, service_tier, access_programs, background, context_management, conversation, image, include, input,
                    instructions, max_output_tokens, max_tool_calls, metadata, moderation, parallel_tool_calls, previous_response_id, prompt,
                    prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning, safety_identifier, store, stream_options,
                    temperature, text, tool_choice, tools, top_logprobs, top_p, truncation, user):
    return {
        "model": model, "stream": stream, "service_tier": service_tier, "access_programs": access_programs, "background": background,
        "context_management": context_management, "conversation": conversation, "image": image, "include": include, "input": input,
        "instructions": instructions, "max_output_tokens": max_output_tokens, "max_tool_calls": max_tool_calls, "metadata": metadata,
        "moderation": moderation, "parallel_tool_calls": parallel_tool_calls, "previous_response_id": previous_response_id,
        "prompt": prompt, "prompt_cache_key": prompt_cache_key, "prompt_cache_options": prompt_cache_options,
        "prompt_cache_retention": prompt_cache_retention, "reasoning": reasoning, "safety_identifier": safety_identifier, "store": store,
        "stream_options": stream_options, "temperature": temperature, "text": text, "tool_choice": tool_choice, "tools": tools,
        "top_logprobs": top_logprobs, "top_p": top_p, "truncation": truncation, "user": user,
    }


class Responses:
    def __init__(self, client):
        self._client = client

    def create(self, *, access_programs=omit, background=omit, context_management=omit, conversation=omit, image=omit, include=omit, input=omit,
               instructions=omit, max_output_tokens=omit, max_tool_calls=omit, metadata=omit, model=omit, moderation=omit,
               parallel_tool_calls=omit, previous_response_id=omit, prompt=omit, prompt_cache_key=omit, prompt_cache_options=omit,
               prompt_cache_retention=omit, reasoning=omit, safety_identifier=omit, service_tier=omit, store=omit, stream=omit,
               stream_options=omit, temperature=omit, text=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit, truncation=omit,
               user=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        body = _responses_body(model, stream, service_tier, access_programs, background, context_management, conversation, image, include,
                               input, instructions, max_output_tokens, max_tool_calls, metadata, moderation, parallel_tool_calls,
                               previous_response_id, prompt, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning,
                               safety_identifier, store, stream_options, temperature, text, tool_choice, tools, top_logprobs, top_p,
                               truncation, user)
        return self._client.post("/responses", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=Response,
                                 stream=bool(stream), stream_cls=(Stream, _EVENT))

    def parse(self, *, text_format=omit, parse_as=omit, access_programs=omit, background=omit, context_management=omit, conversation=omit,
              image=omit, include=omit, input=omit, instructions=omit, max_output_tokens=omit, max_tool_calls=omit, metadata=omit,
              model=omit, moderation=omit, parallel_tool_calls=omit, previous_response_id=omit, prompt=omit, prompt_cache_key=omit,
              prompt_cache_options=omit, prompt_cache_retention=omit, reasoning=omit, safety_identifier=omit, service_tier=omit, store=omit,
              stream=omit, stream_options=omit, temperature=omit, text=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit,
              truncation=omit, user=omit, verbosity=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        text = _with_text_format(text, text_format)
        output_format = _output_text_parser(extra_body.get("input", input) if isinstance(extra_body, dict) else input, text, text_format, parse_as)
        tools = _make_tools(tools)

        def parser(raw_response):
            return _parse_response(output_format, tools, raw_response)

        body = _responses_body(model, stream, service_tier, access_programs, background, context_management, conversation, image, include,
                               input, instructions, max_output_tokens, max_tool_calls, metadata, moderation, parallel_tool_calls,
                               previous_response_id, prompt, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning,
                               safety_identifier, store, stream_options, temperature, text, tool_choice, tools, top_logprobs, top_p,
                               truncation, user)
        body["verbosity"] = verbosity
        return self._client.post("/responses", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout, parser),
                                 cast_to=Response)

    def stream(self, *, response_id=omit, input=omit, model=omit, access_programs=omit, background=omit, context_management=omit,
               text_format=omit, tools=omit, conversation=omit, image=omit, include=omit, instructions=omit, max_output_tokens=omit,
               max_tool_calls=omit, metadata=omit, parallel_tool_calls=omit, moderation=omit, previous_response_id=omit, prompt=omit,
               prompt_cache_key=omit, prompt_cache_options=omit, prompt_cache_retention=omit, reasoning=omit, safety_identifier=omit,
               service_tier=omit, store=omit, stream_options=omit, temperature=omit, text=omit, tool_choice=omit, top_logprobs=omit,
               top_p=omit, truncation=omit, user=omit, starting_after=omit, extra_headers=None, extra_query=None, extra_body=None,
               timeout=not_given):
        new_response_args = {"access_programs": access_programs, "input": input, "model": model, "context_management": context_management,
                             "conversation": conversation, "image": image, "include": include, "instructions": instructions,
                             "max_output_tokens": max_output_tokens, "max_tool_calls": max_tool_calls, "metadata": metadata,
                             "moderation": moderation, "parallel_tool_calls": parallel_tool_calls,
                             "previous_response_id": previous_response_id, "prompt": prompt, "prompt_cache_key": prompt_cache_key,
                             "prompt_cache_options": prompt_cache_options, "prompt_cache_retention": prompt_cache_retention,
                             "reasoning": reasoning, "safety_identifier": safety_identifier, "service_tier": service_tier, "store": store,
                             "stream_options": stream_options, "temperature": temperature, "text": text, "tool_choice": tool_choice,
                             "top_logprobs": top_logprobs, "top_p": top_p, "truncation": truncation, "user": user, "background": background}
        new_names = [k for k, v in new_response_args.items() if is_given(v)]
        if (is_given(response_id) or is_given(starting_after)) and len(new_names) > 0:
            raise ValueError("Cannot provide both response_id/starting_after can't be provided together with " + ", ".join(new_names))
        tools = _make_tools(tools)
        if len(new_names) == 0:
            if not is_given(response_id):
                raise ValueError("id must be provided when streaming an existing response")
            raise NotImplementedError("Brevi's openai cannot stream an existing response yet")
        if not is_given(input):
            raise ValueError("input must be provided when creating a new response")
        if not is_given(model):
            raise ValueError("model must be provided when creating a new response")
        text = _with_text_format(text, text_format)

        def api_request():
            return self.create(access_programs=access_programs, input=input, model=model, context_management=context_management,
                               conversation=conversation, image=image, include=include, instructions=instructions,
                               max_output_tokens=max_output_tokens, max_tool_calls=max_tool_calls, metadata=metadata, moderation=moderation,
                               parallel_tool_calls=parallel_tool_calls, previous_response_id=previous_response_id, prompt=prompt,
                               prompt_cache_key=prompt_cache_key, prompt_cache_options=prompt_cache_options,
                               prompt_cache_retention=prompt_cache_retention, reasoning=reasoning, safety_identifier=safety_identifier,
                               service_tier=service_tier, store=store, stream_options=stream_options, temperature=temperature, text=text,
                               tool_choice=tool_choice, top_logprobs=top_logprobs, top_p=top_p, truncation=truncation, user=user,
                               background=background, tools=tools, stream=True, extra_headers=extra_headers, extra_query=extra_query,
                               extra_body=extra_body, timeout=timeout)

        return ResponseStreamManager(api_request, text_format=text_format, input_tools=tools, starting_after=None)

    def retrieve(self, response_id, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not response_id:
            raise ValueError("Expected a non-empty value for `response_id` but received %r" % (response_id,))
        return self._client.get("/responses/" + _brevi_sdk.quote_segment(response_id), options=_opts(extra_headers, extra_query, extra_body, timeout),
                                cast_to=Response)


class AsyncResponses:
    def __init__(self, client):
        self._client = client

    async def create(self, *, access_programs=omit, background=omit, context_management=omit, conversation=omit, image=omit, include=omit,
                     input=omit, instructions=omit, max_output_tokens=omit, max_tool_calls=omit, metadata=omit, model=omit, moderation=omit,
                     parallel_tool_calls=omit, previous_response_id=omit, prompt=omit, prompt_cache_key=omit, prompt_cache_options=omit,
                     prompt_cache_retention=omit, reasoning=omit, safety_identifier=omit, service_tier=omit, store=omit, stream=omit,
                     stream_options=omit, temperature=omit, text=omit, tool_choice=omit, tools=omit, top_logprobs=omit, top_p=omit,
                     truncation=omit, user=omit, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        body = _responses_body(model, stream, service_tier, access_programs, background, context_management, conversation, image, include,
                               input, instructions, max_output_tokens, max_tool_calls, metadata, moderation, parallel_tool_calls,
                               previous_response_id, prompt, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning,
                               safety_identifier, store, stream_options, temperature, text, tool_choice, tools, top_logprobs, top_p,
                               truncation, user)
        return await self._client.post("/responses", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout),
                                       cast_to=Response, stream=bool(stream), stream_cls=(AsyncStream, _EVENT))

    async def parse(self, *, text_format=omit, parse_as=omit, access_programs=omit, background=omit, context_management=omit,
                    conversation=omit, image=omit, include=omit, input=omit, instructions=omit, max_output_tokens=omit, max_tool_calls=omit,
                    metadata=omit, model=omit, moderation=omit, parallel_tool_calls=omit, previous_response_id=omit, prompt=omit,
                    prompt_cache_key=omit, prompt_cache_options=omit, prompt_cache_retention=omit, reasoning=omit, safety_identifier=omit,
                    service_tier=omit, store=omit, stream=omit, stream_options=omit, temperature=omit, text=omit, tool_choice=omit,
                    tools=omit, top_logprobs=omit, top_p=omit, truncation=omit, user=omit, verbosity=omit, extra_headers=None,
                    extra_query=None, extra_body=None, timeout=not_given):
        text = _with_text_format(text, text_format)
        output_format = _output_text_parser(extra_body.get("input", input) if isinstance(extra_body, dict) else input, text, text_format, parse_as)
        tools = _make_tools(tools)

        def parser(raw_response):
            return _parse_response(output_format, tools, raw_response)

        body = _responses_body(model, stream, service_tier, access_programs, background, context_management, conversation, image, include,
                               input, instructions, max_output_tokens, max_tool_calls, metadata, moderation, parallel_tool_calls,
                               previous_response_id, prompt, prompt_cache_key, prompt_cache_options, prompt_cache_retention, reasoning,
                               safety_identifier, store, stream_options, temperature, text, tool_choice, tools, top_logprobs, top_p,
                               truncation, user)
        body["verbosity"] = verbosity
        return await self._client.post("/responses", body=body, options=_opts(extra_headers, extra_query, extra_body, timeout, parser),
                                 cast_to=Response)

    def stream(self, *, response_id=omit, input=omit, model=omit, access_programs=omit, background=omit, context_management=omit,
               text_format=omit, tools=omit, conversation=omit, image=omit, include=omit, instructions=omit, max_output_tokens=omit,
               max_tool_calls=omit, metadata=omit, parallel_tool_calls=omit, moderation=omit, previous_response_id=omit, prompt=omit,
               prompt_cache_key=omit, prompt_cache_options=omit, prompt_cache_retention=omit, reasoning=omit, safety_identifier=omit,
               service_tier=omit, store=omit, stream_options=omit, temperature=omit, text=omit, tool_choice=omit, top_logprobs=omit,
               top_p=omit, truncation=omit, user=omit, starting_after=omit, extra_headers=None, extra_query=None, extra_body=None,
               timeout=not_given):
        new_response_args = {"access_programs": access_programs, "input": input, "model": model, "context_management": context_management,
                             "conversation": conversation, "image": image, "include": include, "instructions": instructions,
                             "max_output_tokens": max_output_tokens, "max_tool_calls": max_tool_calls, "metadata": metadata,
                             "moderation": moderation, "parallel_tool_calls": parallel_tool_calls,
                             "previous_response_id": previous_response_id, "prompt": prompt, "prompt_cache_key": prompt_cache_key,
                             "prompt_cache_options": prompt_cache_options, "prompt_cache_retention": prompt_cache_retention,
                             "reasoning": reasoning, "safety_identifier": safety_identifier, "service_tier": service_tier, "store": store,
                             "stream_options": stream_options, "temperature": temperature, "text": text, "tool_choice": tool_choice,
                             "top_logprobs": top_logprobs, "top_p": top_p, "truncation": truncation, "user": user, "background": background}
        new_names = [k for k, v in new_response_args.items() if is_given(v)]
        if (is_given(response_id) or is_given(starting_after)) and len(new_names) > 0:
            raise ValueError("Cannot provide both response_id/starting_after can't be provided together with " + ", ".join(new_names))
        tools = _make_tools(tools)
        if len(new_names) == 0:
            if not is_given(response_id):
                raise ValueError("id must be provided when streaming an existing response")
            raise NotImplementedError("Brevi's openai cannot stream an existing response yet")
        if not is_given(input):
            raise ValueError("input must be provided when creating a new response")
        if not is_given(model):
            raise ValueError("model must be provided when creating a new response")
        text = _with_text_format(text, text_format)
        api_request = self.create(access_programs=access_programs, input=input, model=model, context_management=context_management,
                                  conversation=conversation, image=image, include=include, instructions=instructions,
                                  max_output_tokens=max_output_tokens, max_tool_calls=max_tool_calls, metadata=metadata,
                                  moderation=moderation, parallel_tool_calls=parallel_tool_calls, previous_response_id=previous_response_id,
                                  prompt=prompt, prompt_cache_key=prompt_cache_key, prompt_cache_options=prompt_cache_options,
                                  prompt_cache_retention=prompt_cache_retention, reasoning=reasoning, safety_identifier=safety_identifier,
                                  service_tier=service_tier, store=store, stream_options=stream_options, temperature=temperature, text=text,
                                  tool_choice=tool_choice, top_logprobs=top_logprobs, top_p=top_p, truncation=truncation, user=user,
                                  background=background, tools=tools, stream=True, extra_headers=extra_headers, extra_query=extra_query,
                                  extra_body=extra_body, timeout=timeout)
        return AsyncResponseStreamManager(api_request, text_format=text_format, input_tools=tools, starting_after=None)

    async def retrieve(self, response_id, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not response_id:
            raise ValueError("Expected a non-empty value for `response_id` but received %r" % (response_id,))
        return await self._client.get("/responses/" + _brevi_sdk.quote_segment(response_id),
                                      options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=Response)


# ---------- structured outputs: parse() and the stream helpers (openai/lib) ----------

def _partial_json(s):
    # jiter's from_json(partial_mode=True): a JSON text that may stop anywhere; a value cut short is left out
    p = _PJ(s)
    p.ws()
    if p.i >= p.n:
        raise ValueError("EOF while parsing a value at line 1 column %d" % p.n)
    v = p.value()
    if v is _CUT:
        raise ValueError("EOF while parsing a value at line 1 column %d" % p.n)
    return v


class _Cut:
    pass


_CUT = _Cut()
_PJ_ESC = {'"': '"', "\\": "\\", "/": "/", "b": "\b", "f": "\f", "n": "\n", "r": "\r", "t": "\t"}


class _PJ:
    def __init__(self, s):
        self.s = s
        self.i = 0
        self.n = len(s)

    def ws(self):
        s = self.s
        while self.i < self.n and s[self.i] in " \t\r\n":
            self.i += 1

    def fail(self, what):
        raise ValueError("%s at line 1 column %d" % (what, self.i + 1))

    def value(self):
        s = self.s
        c = s[self.i]
        if c == "{":
            return self.obj()
        if c == "[":
            return self.arr()
        if c == '"':
            return self.string()
        if c == "-" or c.isdigit():
            return self.number()
        for word, v in (("true", True), ("false", False), ("null", None)):
            if s.startswith(word, self.i):
                self.i += len(word)
                return v
            if word.startswith(s[self.i:]):
                self.i = self.n
                return _CUT
        self.fail("expected value")

    def obj(self):
        out = {}
        self.i += 1
        self.ws()
        if self.i >= self.n:
            return out
        if self.s[self.i] == "}":
            self.i += 1
            return out
        while True:
            self.ws()
            if self.i >= self.n:
                return out
            if self.s[self.i] != '"':
                self.fail("key must be a string")
            k = self.string()
            if k is _CUT:
                return out
            self.ws()
            if self.i >= self.n:
                return out
            if self.s[self.i] != ":":
                self.fail("expected `:`")
            self.i += 1
            self.ws()
            if self.i >= self.n:
                return out
            v = self.value()
            if v is _CUT:
                return out
            out[k] = v
            self.ws()
            if self.i >= self.n:
                return out
            c = self.s[self.i]
            if c == ",":
                self.i += 1
                self.ws()
                if self.i < self.n and self.s[self.i] == "}":
                    self.fail("trailing comma")
                continue
            if c == "}":
                self.i += 1
                return out
            self.fail("expected `,` or `}`")

    def arr(self):
        out = []
        self.i += 1
        self.ws()
        if self.i >= self.n:
            return out
        if self.s[self.i] == "]":
            self.i += 1
            return out
        while True:
            self.ws()
            if self.i >= self.n:
                return out
            v = self.value()
            if v is _CUT:
                return out
            out.append(v)
            self.ws()
            if self.i >= self.n:
                return out
            c = self.s[self.i]
            if c == ",":
                self.i += 1
                self.ws()
                if self.i < self.n and self.s[self.i] == "]":
                    self.fail("trailing comma")
                continue
            if c == "]":
                self.i += 1
                return out
            self.i = self.n
            return out

    def string(self):
        s = self.s
        self.i += 1
        parts = []
        start = self.i
        while True:
            j = self.i
            if j >= self.n:
                self.i = self.n
                return _CUT
            c = s[j]
            if c == '"':
                parts.append(s[start:j])
                self.i = j + 1
                return "".join(parts)
            if c == "\\":
                parts.append(s[start:j])
                if j + 1 >= self.n:
                    self.i = self.n
                    return _CUT
                e = s[j + 1]
                if e == "u":
                    if j + 6 > self.n:
                        self.i = self.n
                        return _CUT
                    cp = int(s[j + 2:j + 6], 16)
                    k = j + 6
                    if 0xD800 <= cp < 0xDC00:
                        if k + 6 > self.n:
                            self.i = self.n
                            return _CUT
                        if s[k:k + 2] == "\\u":
                            lo = int(s[k + 2:k + 6], 16)
                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                            k += 6
                    parts.append(chr(cp))
                    self.i = k
                elif e in _PJ_ESC:
                    parts.append(_PJ_ESC[e])
                    self.i = j + 2
                else:
                    self.i = j + 1
                    self.fail("invalid escape")
                start = self.i
                continue
            self.i = j + 1

    def number(self):
        s = self.s
        j = self.i
        if s[j] == "-":
            j += 1
        k = j
        while j < self.n and s[j].isdigit():
            j += 1
        if j == k:
            if j >= self.n:
                self.i = self.n
                return _CUT
            self.i = j
            self.fail("invalid number")
        if s[k] == "0" and j - k > 1:
            self.i = j
            self.fail("invalid number")
        is_float = False
        if j < self.n and s[j] == ".":
            is_float = True
            j += 1
            k = j
            while j < self.n and s[j].isdigit():
                j += 1
            if j == k:
                self.i = self.n
                return _CUT
        if j < self.n and s[j] in "eE":
            is_float = True
            j += 1
            if j < self.n and s[j] in "+-":
                j += 1
            k = j
            while j < self.n and s[j].isdigit():
                j += 1
            if j == k:
                self.i = self.n
                return _CUT
        t = s[self.i:j]
        self.i = j
        return float(t) if is_float else int(t)


def _is_basemodel_type(t):
    return isinstance(t, type) and issubclass(t, _pyd.BaseModel)


def _ensure_strict(json_schema, path, root):
    # openai/lib/_pydantic.py: the strict JSON schema the API expects (every property required, no extras)
    if not isinstance(json_schema, dict):
        raise TypeError("Expected %s to be a dictionary; path=%s" % (json_schema, path))
    defs = json_schema.get("$defs")
    if isinstance(defs, dict):
        for def_name, def_schema in defs.items():
            _ensure_strict(def_schema, path + ("$defs", def_name), root)
    definitions = json_schema.get("definitions")
    if isinstance(definitions, dict):
        for definition_name, definition_schema in definitions.items():
            _ensure_strict(definition_schema, path + ("definitions", definition_name), root)
    typ = json_schema.get("type")
    if typ == "object" and "additionalProperties" not in json_schema:
        json_schema["additionalProperties"] = False
    properties = json_schema.get("properties")
    if isinstance(properties, dict):
        json_schema["required"] = [prop for prop in properties.keys()]
        json_schema["properties"] = {key: _ensure_strict(prop_schema, path + ("properties", key), root)
                                     for key, prop_schema in properties.items()}
    items = json_schema.get("items")
    if isinstance(items, dict):
        json_schema["items"] = _ensure_strict(items, path + ("items",), root)
    any_of = json_schema.get("anyOf")
    if isinstance(any_of, list):
        json_schema["anyOf"] = [_ensure_strict(variant, path + ("anyOf", str(i)), root) for i, variant in enumerate(any_of)]
    all_of = json_schema.get("allOf")
    if isinstance(all_of, list):
        if len(all_of) == 1:
            json_schema.update(_ensure_strict(all_of[0], path + ("allOf", "0"), root))
            json_schema.pop("allOf")
        else:
            json_schema["allOf"] = [_ensure_strict(entry, path + ("allOf", str(i)), root) for i, entry in enumerate(all_of)]
    if "default" in json_schema and json_schema["default"] is None:
        json_schema.pop("default")
    ref = json_schema.get("$ref")
    if ref and len(json_schema) > 1:
        resolved = _resolve_ref(root, ref)
        if not isinstance(resolved, dict):
            raise ValueError("Expected `$ref: %s` to resolved to a dictionary but got %s" % (ref, resolved))
        merged = dict(resolved)
        merged.update(json_schema)
        json_schema.update(merged)
        json_schema.pop("$ref")
        return _ensure_strict(json_schema, path, root)
    return json_schema


def _resolve_ref(root, ref):
    if not ref.startswith("#/"):
        raise ValueError("Unexpected $ref format %r; Does not start with #/" % (ref,))
    resolved = root
    for key in ref[2:].split("/"):
        resolved = resolved[key]
    return resolved


def to_strict_json_schema(model):
    if _is_basemodel_type(model):
        schema = model.model_json_schema()
    elif isinstance(model, _pyd.TypeAdapter):
        schema = model.json_schema()
    else:
        raise TypeError("Non BaseModel types are only supported with Pydantic v2 - %s" % (model,))
    return _ensure_strict(schema, (), schema)


class PydanticFunctionTool(dict):
    # a function definition that remembers its model, so its calls' arguments parse into it
    def __init__(self, defn, model):
        dict.__init__(self, defn)
        self.model = model

    def cast(self):
        return self


class ResponsesPydanticFunctionTool(dict):
    def __init__(self, tool, model):
        dict.__init__(self, tool)
        self.model = model

    def cast(self):
        return self


def pydantic_function_tool(model, *, name=None, description=None):
    if description is None:
        description = model.__doc__
    function = PydanticFunctionTool({"name": name or model.__name__, "strict": True, "parameters": to_strict_json_schema(model)}, model)
    if description is not None:
        function["description"] = description
    return {"type": "function", "function": function}


def pydantic_responses_function_tool(model, *, name=None, description=None, defer_loading=omit):
    function = pydantic_function_tool(model, name=name, description=description)["function"]
    d = {"type": "function"}
    d.update(function)
    tool = ResponsesPydanticFunctionTool(d, model)
    if is_given(defer_loading):
        tool["defer_loading"] = defer_loading
    return tool


def _type_to_response_format(response_format):
    if not is_given(response_format):
        return omit
    if isinstance(response_format, dict):
        return response_format
    if _is_basemodel_type(response_format):
        name = response_format.__name__
        json_schema_type = response_format
    else:
        raise TypeError("Unsupported response_format type - %s" % (response_format,))
    return {"type": "json_schema", "json_schema": {"schema": to_strict_json_schema(json_schema_type), "name": name, "strict": True}}


def _type_to_text_format(type_):
    d = _type_to_response_format(type_)
    return {"type": "json_schema", "strict": True, "name": d["json_schema"]["name"], "schema": d["json_schema"]["schema"]}


def _validate_input_tools(tools):
    if not is_given(tools):
        return omit
    input_tools = list(tools)
    for tool in input_tools:
        if tool["type"] != "function":
            raise ValueError("Currently only `function` tool types support auto-parsing; Received `%s`" % (tool["type"],))
        if tool["function"].get("strict") is not True:
            raise ValueError("`%s` is not strict. Only `strict` function tools can be auto-parsed" % (tool["function"]["name"],))
    return input_tools


def _materialize_input_tools(tools):
    if not is_given(tools):
        return omit
    return list(tools)


def _has_rich_response_format(response_format):
    return is_given(response_format) and not isinstance(response_format, dict)


def _parse_content(response_format, content):
    if _is_basemodel_type(response_format):
        return response_format.model_validate_json(content)
    raise TypeError("Unable to automatically parse response format type %s" % (response_format,))


def _maybe_parse_content(response_format, message):
    if _has_rich_response_format(response_format) and message.content and not message.refusal:
        return _parse_content(response_format, message.content)
    return None


def _is_parseable_tool(input_tool):
    if input_tool["type"] != "function":
        return False
    input_fn = input_tool.get("function")
    if isinstance(input_fn, PydanticFunctionTool):
        return True
    return input_fn.get("strict") or False


def _has_parseable_input(response_format, input_tools):
    if _has_rich_response_format(response_format):
        return True
    for input_tool in input_tools or []:
        if _is_parseable_tool(input_tool):
            return True
    return False


def _chat_tool_by_name(input_tools, name):
    for t in input_tools:
        if t["type"] == "function" and t.get("function", {}).get("name") == name:
            return t
    return None


def _parse_function_tool_arguments(input_tools, function):
    input_tool = _chat_tool_by_name(input_tools, function.name)
    if not input_tool:
        return None
    input_fn = input_tool.get("function")
    if isinstance(input_fn, PydanticFunctionTool):
        return input_fn.model.model_validate_json(function.arguments)
    if not input_fn.get("strict"):
        return None
    return _json.loads(function.arguments)


# the SDK's generic models, by the names their parametrized classes print (ParsedChatCompletion[TypeVar] from
# parse(), [object] for a stream's snapshot)
def _generic(cls, arg):
    return type(cls.__name__ + "[" + arg + "]", (cls,), {"__module__": cls.__module__})


_GCompletion = _generic(_ty.ParsedChatCompletion, "TypeVar")
_GChoice = _generic(_ty.ParsedChoice, "TypeVar")
_GMessage = _generic(_ty.ParsedChatCompletionMessage, "TypeVar")
_OCompletion = _generic(_ty.ParsedChatCompletion, "object")
_OChoice = _generic(_ty.ParsedChoice, "object")
_OMessage = _generic(_ty.ParsedChatCompletionMessage, "object")


def _parse_chat_completion(response_format, input_tools, chat_completion):
    input_tools = list(input_tools) if is_given(input_tools) else []
    choices = []
    for choice in chat_completion.choices:
        if choice.finish_reason == "length":
            raise LengthFinishReasonError(completion=chat_completion)
        if choice.finish_reason == "content_filter":
            raise ContentFilterFinishReasonError(completion=chat_completion)
        message = choice.message
        tool_calls = []
        if message.tool_calls:
            for tool_call in message.tool_calls:
                if tool_call.type == "function":
                    d = tool_call.to_dict()
                    fn = dict(d["function"])
                    fn["parsed_arguments"] = _parse_function_tool_arguments(input_tools, tool_call.function)
                    d["function"] = fn
                    tool_calls.append(_brevi_sdk.build(_ty.ParsedFunctionToolCall, **d))
                elif tool_call.type == "custom":
                    pass
                else:
                    tool_calls.append(tool_call)
        md = message.to_dict()
        md["parsed"] = _maybe_parse_content(response_format, message)
        md["tool_calls"] = tool_calls if tool_calls else None
        cd = choice.to_dict()
        cd.pop("message", None)
        ch = _brevi_sdk.build(_GChoice, **cd)
        ch.message = _brevi_sdk.build(_GMessage, **md)
        ch._bv_fs = _fields_with(ch, "message")
        choices.append(ch)
    d = chat_completion.to_dict()
    d["choices"] = choices
    return _brevi_sdk.build(_GCompletion, **d)


def _fields_with(m, name):
    # the fields set, in the class's order, with `name` among them
    fs = set(m._bv_fs)
    fs.add(name)
    return [f[0] for f in type(m)._F if f[0] in fs]


# ---------- chat.completions.stream ----------

def _has_indexed_entries(value):
    return isinstance(value, list) and any(isinstance(entry, dict) and "index" in entry for entry in value)


def _accumulate_delta(acc, delta):
    for key, delta_value in delta.items():
        if key not in acc or acc[key] is None:
            if _has_indexed_entries(delta_value):
                acc[key] = []
            else:
                acc[key] = delta_value
                continue
        acc_value = acc[key]
        if key == "index" or key == "type":
            acc[key] = delta_value
            continue
        if isinstance(acc_value, str) and isinstance(delta_value, str):
            acc_value += delta_value
        elif isinstance(acc_value, (int, float)) and isinstance(delta_value, (int, float)):
            acc_value += delta_value
        elif isinstance(acc_value, dict) and isinstance(delta_value, dict):
            acc_value = _accumulate_delta(acc_value, delta_value)
        elif isinstance(acc_value, list) and isinstance(delta_value, list):
            if all(isinstance(x, (str, int, float)) for x in acc_value) and (acc_value or not _has_indexed_entries(delta_value)):
                acc_value.extend(delta_value)
                continue
            for delta_entry in delta_value:
                if not isinstance(delta_entry, dict):
                    raise TypeError("Unexpected list delta entry is not a dictionary: %s" % (delta_entry,))
                if "index" not in delta_entry:
                    raise RuntimeError("Expected list delta entry to have an `index` key; %s" % (delta_entry,))
                index = delta_entry["index"]
                if not isinstance(index, int):
                    raise TypeError("Unexpected, list delta entry `index` value is not an integer; %s" % (index,))
                if index < len(acc_value):
                    acc_entry = acc_value[index]
                    if not isinstance(acc_entry, dict):
                        raise TypeError("not handled yet")
                    acc_value[index] = _accumulate_delta(acc_entry, delta_entry)
                else:
                    acc_value.insert(index, delta_entry)
        acc[key] = acc_value
    return acc


def _snapshot_choice(value):
    v = dict(value)
    msg = v.pop("message")
    ch = _brevi_sdk.build(_OChoice, **v)
    ch.message = _brevi_sdk.build(_OMessage, **msg)
    ch._bv_fs = _fields_with(ch, "message")
    return ch


def _initial_snapshot(chunk):
    data = chunk.to_dict()
    data.pop("obfuscation", None)
    choices = data["choices"]
    for choice in chunk.choices:
        cd = choice.model_dump(exclude_unset=True, exclude={"delta"})
        cd["message"] = _accumulate_delta({}, choice.delta.to_dict())
        choices[choice.index] = cd
    value = {"system_fingerprint": None}
    value.update(data)
    value["object"] = "chat.completion"
    built = [_snapshot_choice(c) for c in value.pop("choices")]
    snap = _brevi_sdk.build(_OCompletion, **value)
    snap.choices = built
    snap._bv_fs = _fields_with(snap, "choices")
    return snap


class ChatCompletionStreamState:
    def __init__(self, *, input_tools=omit, response_format=omit):
        self._snapshot = None
        self._choice_event_states = []
        self._input_tools = list(input_tools) if is_given(input_tools) else []
        self._response_format = response_format
        self._rich_response_format = response_format if isinstance(response_format, type) else omit

    def get_final_completion(self):
        return _parse_chat_completion(self._rich_response_format, self._input_tools, self.current_completion_snapshot)

    @property
    def current_completion_snapshot(self):
        assert self._snapshot is not None
        return self._snapshot

    def handle_chunk(self, chunk):
        self._snapshot = self._accumulate_chunk(chunk)
        return self._build_events(chunk, self._snapshot)

    def _get_choice_state(self, choice):
        if choice.index < len(self._choice_event_states):
            return self._choice_event_states[choice.index]
        st = _ChoiceEventState(self._input_tools)
        self._choice_event_states.append(st)
        return st

    def _accumulate_chunk(self, chunk):
        completion_snapshot = self._snapshot
        if completion_snapshot is None:
            return _initial_snapshot(chunk)
        for choice in chunk.choices:
            if choice.index < len(completion_snapshot.choices):
                choice_snapshot = completion_snapshot.choices[choice.index]
                previous_tool_calls = choice_snapshot.message.tool_calls or []
                md = choice_snapshot.message.model_dump(exclude={"parsed"})
                for tc in md.get("tool_calls") or []:
                    if isinstance(tc, dict) and isinstance(tc.get("function"), dict):
                        tc["function"].pop("parsed_arguments", None)
                choice_snapshot.message = _brevi_sdk.build(_OMessage, **_accumulate_delta(md, choice.delta.to_dict()))
                for tool_index, prev_tool in enumerate(previous_tool_calls):
                    new_tool = (choice_snapshot.message.tool_calls or [])[tool_index]
                    if prev_tool.type == "function":
                        new_tool.function.parsed_arguments = prev_tool.function.parsed_arguments
            else:
                cd = choice.model_dump(exclude_unset=True, exclude={"delta"})
                cd["message"] = _accumulate_delta({}, choice.delta.to_dict())
                choice_snapshot = _snapshot_choice(cd)
                completion_snapshot.choices.append(choice_snapshot)
            if choice.finish_reason:
                choice_snapshot.finish_reason = choice.finish_reason
                if _has_parseable_input(self._response_format, self._input_tools):
                    if choice.finish_reason == "length":
                        raise LengthFinishReasonError(completion=completion_snapshot)
                    if choice.finish_reason == "content_filter":
                        raise ContentFilterFinishReasonError(completion=completion_snapshot)
            m = choice_snapshot.message
            if m.content and not m.refusal and is_given(self._rich_response_format) and m.content.lstrip():
                m.parsed = _partial_json(m.content)
            for tool_call_chunk in choice.delta.tool_calls or []:
                tool_call_snapshot = (choice_snapshot.message.tool_calls or [])[tool_call_chunk.index]
                if tool_call_snapshot.type == "function":
                    input_tool = _chat_tool_by_name(self._input_tools, tool_call_snapshot.function.name)
                    if input_tool and input_tool.get("function", {}).get("strict") and tool_call_snapshot.function.arguments:
                        tool_call_snapshot.function.parsed_arguments = _partial_json(tool_call_snapshot.function.arguments)
            if choice.logprobs is not None:
                if choice_snapshot.logprobs is None:
                    choice_snapshot.logprobs = _brevi_sdk.build(_ty.ChoiceLogprobs, content=choice.logprobs.content,
                                                                refusal=choice.logprobs.refusal)
                else:
                    if choice.logprobs.content:
                        if choice_snapshot.logprobs.content is None:
                            choice_snapshot.logprobs.content = []
                        choice_snapshot.logprobs.content.extend(choice.logprobs.content)
                    if choice.logprobs.refusal:
                        if choice_snapshot.logprobs.refusal is None:
                            choice_snapshot.logprobs.refusal = []
                        choice_snapshot.logprobs.refusal.extend(choice.logprobs.refusal)
        completion_snapshot.usage = chunk.usage
        completion_snapshot.system_fingerprint = chunk.system_fingerprint
        if chunk.moderation is not None:
            completion_snapshot.moderation = _brevi_sdk.build(_ty.Moderation, **chunk.moderation.to_dict())
        return completion_snapshot

    def _build_events(self, chunk, completion_snapshot):
        events = [_brevi_sdk.build(_ty.ChunkEvent, type="chunk", chunk=chunk, snapshot=completion_snapshot)]
        for choice in chunk.choices:
            choice_state = self._get_choice_state(choice)
            choice_snapshot = completion_snapshot.choices[choice.index]
            if choice.delta.content is not None and choice_snapshot.message.content is not None:
                events.append(_brevi_sdk.build(_ty.ContentDeltaEvent, type="content.delta", delta=choice.delta.content,
                                               snapshot=choice_snapshot.message.content, parsed=choice_snapshot.message.parsed))
            if choice.delta.refusal is not None and choice_snapshot.message.refusal is not None:
                events.append(_brevi_sdk.build(_ty.RefusalDeltaEvent, type="refusal.delta", delta=choice.delta.refusal,
                                               snapshot=choice_snapshot.message.refusal))
            if choice.delta.tool_calls:
                tool_calls = choice_snapshot.message.tool_calls
                for tool_call_delta in choice.delta.tool_calls:
                    tool_call = tool_calls[tool_call_delta.index]
                    if tool_call.type == "function":
                        events.append(_brevi_sdk.build(
                            _ty.FunctionToolCallArgumentsDeltaEvent, type="tool_calls.function.arguments.delta", name=tool_call.function.name,
                            index=tool_call_delta.index, arguments=tool_call.function.arguments,
                            parsed_arguments=tool_call.function.parsed_arguments,
                            arguments_delta=tool_call_delta.function.arguments or ""))
            if choice.logprobs is not None and choice_snapshot.logprobs is not None:
                if choice.logprobs.content and choice_snapshot.logprobs.content:
                    events.append(_brevi_sdk.build(_ty.LogprobsContentDeltaEvent, type="logprobs.content.delta", content=choice.logprobs.content,
                                                   snapshot=choice_snapshot.logprobs.content))
                if choice.logprobs.refusal and choice_snapshot.logprobs.refusal:
                    events.append(_brevi_sdk.build(_ty.LogprobsRefusalDeltaEvent, type="logprobs.refusal.delta", refusal=choice.logprobs.refusal,
                                                   snapshot=choice_snapshot.logprobs.refusal))
            events.extend(choice_state.get_done_events(choice, choice_snapshot, self._response_format))
        return events


class _ChoiceEventState:
    def __init__(self, input_tools):
        self._input_tools = input_tools
        self._content_done = False
        self._refusal_done = False
        self._logprobs_content_done = False
        self._logprobs_refusal_done = False
        self._done_tool_calls = set()
        self._current_tool_call_index = None

    def get_done_events(self, choice_chunk, choice_snapshot, response_format):
        events = []
        if choice_snapshot.finish_reason:
            events.extend(self._content_done_events(choice_snapshot, response_format))
            if self._current_tool_call_index is not None and self._current_tool_call_index not in self._done_tool_calls:
                self._add_tool_done_event(events, choice_snapshot, self._current_tool_call_index)
        for tool_call in choice_chunk.delta.tool_calls or []:
            if self._current_tool_call_index != tool_call.index:
                events.extend(self._content_done_events(choice_snapshot, response_format))
                if self._current_tool_call_index is not None:
                    self._add_tool_done_event(events, choice_snapshot, self._current_tool_call_index)
            self._current_tool_call_index = tool_call.index
        return events

    def _content_done_events(self, choice_snapshot, response_format):
        events = []
        if choice_snapshot.message.content and not self._content_done:
            self._content_done = True
            parsed = _maybe_parse_content(response_format, choice_snapshot.message)
            choice_snapshot.message.parsed = parsed
            events.append(_brevi_sdk.build(_ty.ContentDoneEvent, type="content.done", content=choice_snapshot.message.content, parsed=parsed))
        if choice_snapshot.message.refusal is not None and not self._refusal_done:
            self._refusal_done = True
            events.append(_brevi_sdk.build(_ty.RefusalDoneEvent, type="refusal.done", refusal=choice_snapshot.message.refusal))
        lp = choice_snapshot.logprobs
        if lp is not None and lp.content is not None and not self._logprobs_content_done:
            self._logprobs_content_done = True
            events.append(_brevi_sdk.build(_ty.LogprobsContentDoneEvent, type="logprobs.content.done", content=lp.content))
        if lp is not None and lp.refusal is not None and not self._logprobs_refusal_done:
            self._logprobs_refusal_done = True
            events.append(_brevi_sdk.build(_ty.LogprobsRefusalDoneEvent, type="logprobs.refusal.done", refusal=lp.refusal))
        return events

    def _add_tool_done_event(self, events, choice_snapshot, tool_index):
        if tool_index in self._done_tool_calls:
            return
        self._done_tool_calls.add(tool_index)
        tool_call_snapshot = choice_snapshot.message.tool_calls[tool_index]
        if tool_call_snapshot.type == "function":
            parsed_arguments = _parse_function_tool_arguments(self._input_tools, tool_call_snapshot.function)
            tool_call_snapshot.function.parsed_arguments = parsed_arguments
            events.append(_brevi_sdk.build(_ty.FunctionToolCallArgumentsDoneEvent, type="tool_calls.function.arguments.done", index=tool_index,
                                           name=tool_call_snapshot.function.name, arguments=tool_call_snapshot.function.arguments,
                                           parsed_arguments=parsed_arguments))


class ChatCompletionStream:
    def __init__(self, *, raw_stream, response_format, input_tools):
        self._raw_stream = raw_stream
        self._response = raw_stream.response
        self._iterator = self._stream()
        self._state = ChatCompletionStreamState(response_format=response_format, input_tools=input_tools)

    def __next__(self):
        return self._iterator.__next__()

    def __iter__(self):
        for item in self._iterator:
            yield item

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def close(self):
        self._response.close()

    def get_final_completion(self):
        self.until_done()
        return self._state.get_final_completion()

    def until_done(self):
        for _ in self:
            pass
        return self

    @property
    def current_completion_snapshot(self):
        return self._state.current_completion_snapshot

    def _stream(self):
        for sse_event in self._raw_stream:
            if sse_event.object != "chat.completion.chunk":
                continue
            for event in self._state.handle_chunk(sse_event):
                yield event


class ChatCompletionStreamManager:
    def __init__(self, api_request, *, response_format, input_tools):
        self._stream = None
        self._api_request = api_request
        self._response_format = response_format
        self._input_tools = input_tools

    def __enter__(self):
        self._stream = ChatCompletionStream(raw_stream=self._api_request(), response_format=self._response_format, input_tools=self._input_tools)
        return self._stream

    def __exit__(self, exc_type, exc, tb):
        if self._stream is not None:
            self._stream.close()


class AsyncChatCompletionStream:
    def __init__(self, *, raw_stream, response_format, input_tools):
        self._raw_stream = raw_stream
        self._response = raw_stream.response
        self._iterator = self._stream()
        self._state = ChatCompletionStreamState(response_format=response_format, input_tools=input_tools)

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
        await self._response.aclose()

    async def get_final_completion(self):
        await self.until_done()
        return self._state.get_final_completion()

    async def until_done(self):
        async for _ in self:
            pass
        return self

    @property
    def current_completion_snapshot(self):
        return self._state.current_completion_snapshot

    async def _stream(self):
        async for sse_event in self._raw_stream:
            if sse_event.object != "chat.completion.chunk":
                continue
            for event in self._state.handle_chunk(sse_event):
                yield event


class AsyncChatCompletionStreamManager:
    def __init__(self, api_request, *, response_format, input_tools):
        self._stream = None
        self._api_request = api_request
        self._response_format = response_format
        self._input_tools = input_tools

    async def __aenter__(self):
        raw_stream = await self._api_request
        self._stream = AsyncChatCompletionStream(raw_stream=raw_stream, response_format=self._response_format, input_tools=self._input_tools)
        return self._stream

    async def __aexit__(self, exc_type, exc, tb):
        if self._stream is not None:
            await self._stream.close()


# ---------- responses.parse / responses.stream ----------

def _make_tools(tools):
    if not is_given(tools):
        return omit
    converted = []
    for tool in tools:
        if tool["type"] == "namespace":
            t = dict(tool)
            t["tools"] = list(tool["tools"])
            converted.append(t)
            continue
        if tool["type"] != "function" or "function" not in tool:
            converted.append(tool)
            continue
        function = tool["function"]
        if not isinstance(function, PydanticFunctionTool):
            raise Exception("Expected Chat Completions function tool shape to be created using `openai.pydantic_function_tool()`")
        new_tool = ResponsesPydanticFunctionTool({"type": "function", "name": function["name"], "description": function.get("description"),
                                                  "parameters": function["parameters"], "strict": function.get("strict") or False},
                                                 function.model)
        for key, value in function.items():
            if key not in new_tool:
                new_tool[key] = value
        converted.append(new_tool)
    return converted


def _parse_text(text, text_format, phase):
    if phase is not None and phase != "final_answer":
        return None
    if not is_given(text_format):
        return None
    if _is_basemodel_type(text_format):
        return text_format.model_validate_json(text)
    raise TypeError("Unable to automatically parse response format type %s" % (text_format,))


def _responses_tool_by_name(input_tools, name, namespace=None):
    if namespace is not None:
        for tool in input_tools:
            if tool["type"] == "namespace" and tool["name"] == namespace:
                for nested in tool["tools"]:
                    if nested["type"] == "function" and nested["name"] == name:
                        return nested
                return None
    if namespace is not None and namespace != name:
        return None
    for tool in input_tools:
        if tool["type"] == "function" and tool.get("name") == name:
            return tool
    return None


def _responses_tool_arguments(input_tools, function_call):
    if input_tools is None or not is_given(input_tools):
        return None
    input_tool = _responses_tool_by_name(input_tools, function_call.name, getattr(function_call, "namespace", None))
    if not input_tool:
        return None
    if isinstance(input_tool, ResponsesPydanticFunctionTool):
        return input_tool.model.model_validate_json(function_call.arguments)
    if not input_tool.get("strict"):
        return None
    return _json.loads(function_call.arguments)


def _parse_response(text_format, input_tools, response):
    output_list = []
    for output in response.output or []:
        if output.type == "message":
            content_list = []
            for item in output.content or []:
                if item.type != "output_text":
                    content_list.append(item)
                    continue
                d = item.to_dict()
                d["parsed"] = _parse_text(item.text, text_format, getattr(output, "phase", None))
                content_list.append(_brevi_sdk.build(_ty.ParsedResponseOutputText, **d))
            d = output.to_dict()
            d["content"] = content_list
            output_list.append(_brevi_sdk.build(_ty.ParsedResponseOutputMessage, **d))
        elif output.type == "function_call":
            d = output.to_dict()
            d["parsed_arguments"] = _responses_tool_arguments(input_tools, output)
            output_list.append(_brevi_sdk.build(_ty.ParsedResponseFunctionToolCall, **d))
        else:
            output_list.append(output)
    d = response.to_dict()
    d["output"] = output_list
    m = _brevi_sdk.build(_ty.ParsedResponse, **d)
    rid = getattr(response, "_request_id", None)
    if rid is not None:
        m._request_id = rid
    return m


def _output_text_parser(input, text, text_format, parse_as):
    if parse_as is None:
        return omit
    if isinstance(input, list):
        for item in reversed(input):
            if not isinstance(item, dict) or item.get("type") != "configuration_update":
                continue
            update = item.get("text")
            if not isinstance(update, dict):
                continue
            fmt = update.get("format")
            if not isinstance(fmt, dict):
                continue
            if fmt.get("type") == "text":
                if is_given(parse_as):
                    raise TypeError("Cannot use parse_as with a plain-text format update")
                return omit
            if is_given(text_format) and not is_given(parse_as) and (not is_given(text) or fmt != text.get("format")):
                raise TypeError("Pass parse_as for the updated output schema, or parse_as=None to read plain output_text")
            break
    return parse_as if is_given(parse_as) else text_format


def _with_text_format(text, text_format):
    if is_given(text_format):
        if not text:
            text = {}
        if "format" in text:
            raise TypeError("Cannot mix and match text.format with text_format")
        text = dict(text)
        text["format"] = _type_to_text_format(text_format)
    return text


def _output_identity(item):
    call_id = getattr(item, "call_id", None)
    if item.type in ("function_call", "custom_tool_call", "shell_call", "shell_call_output"):
        return (item.type, None, call_id)
    return (item.type, getattr(item, "id", None), call_id)


def _compatible_output_id(previous, replacement):
    previous_id = getattr(previous, "id", None)
    return previous_id == getattr(replacement, "id", None) or (previous_id is None and previous.type in ("function_call", "custom_tool_call"))


_LIFECYCLE = ("response.created", "response.queued", "response.in_progress", "response.failed", "response.incomplete")


class ResponseStreamState:
    def __init__(self, *, input_tools, text_format):
        self._snapshot = None
        self._completed_response = None
        self._completed_output = {}
        self._completed_positions = {}
        self._output_items = {}
        self._input_tools = list(input_tools) if is_given(input_tools) else []
        self._text_format = text_format
        self._rich_text_format = text_format if isinstance(text_format, type) else omit

    def handle_event(self, event):
        self._snapshot = self.accumulate_event(event)
        events = []
        t = event.type
        if t == "response.output_item.added" and getattr(event, "item", None) is None:
            return events
        if t == "response.output_text.delta":
            output = self._get_output_item(event.output_index)
            content = output.content[event.content_index]
            events.append(_brevi_sdk.build(_ty.ResponseTextDeltaEvent_2, content_index=event.content_index, delta=event.delta,
                                           item_id=event.item_id, output_index=event.output_index, sequence_number=event.sequence_number,
                                           logprobs=event.logprobs, type="response.output_text.delta", snapshot=content.text))
        elif t == "response.output_text.done":
            output = self._get_output_item(event.output_index)
            events.append(_brevi_sdk.build(_ty.ResponseTextDoneEvent_2, content_index=event.content_index, item_id=event.item_id,
                                           output_index=event.output_index, sequence_number=event.sequence_number, logprobs=event.logprobs,
                                           type="response.output_text.done", text=event.text,
                                           parsed=_parse_text(event.text, self._text_format, getattr(output, "phase", None))))
        elif t == "response.function_call_arguments.delta":
            output = self._get_output_item(event.output_index)
            events.append(_brevi_sdk.build(_ty.ResponseFunctionCallArgumentsDeltaEvent_2, delta=event.delta, item_id=event.item_id,
                                           output_index=event.output_index, sequence_number=event.sequence_number,
                                           type="response.function_call_arguments.delta", snapshot=output.arguments))
        elif t == "response.completed":
            events.append(_brevi_sdk.build(_ty.ResponseCompletedEvent_2, sequence_number=event.sequence_number, type="response.completed",
                                           response=self._completed_response))
        elif t in _LIFECYCLE:
            if getattr(event.response, "output", None) is None:
                event = event.model_copy(deep=True)
                if t == "response.failed" or t == "response.incomplete":
                    keys = sorted(set(self._output_items.keys()) | set(self._completed_output.keys()))
                    event.response.output = [(self._completed_output[k] if k in self._completed_output else self._output_items[k]).model_copy(deep=True)
                                             for k in keys]
                else:
                    event.response.output = []
            events.append(event)
        else:
            events.append(event)
        return events

    def accumulate_event(self, event):
        snapshot = self._snapshot
        if snapshot is None:
            return self._create_initial_response(event)
        t = event.type
        if (t in _LIFECYCLE or t == "response.completed") and getattr(event.response, "id", None) != getattr(snapshot, "id", None):
            raise RuntimeError("Lifecycle response identity changed.")
        if t == "response.created" or t == "response.queued" or t == "response.in_progress":
            replacements = {}
            for replacement_item in getattr(event.response, "output", None) or []:
                identity = _output_identity(replacement_item)
                if identity in replacements:
                    raise RuntimeError("Response snapshot has conflicting output identities.")
                replacements[identity] = replacement_item
            for updated_identity, replacement in replacements.items():
                index = self._completed_positions.get(updated_identity)
                if index is not None:
                    if not _compatible_output_id(self._completed_output[index], replacement):
                        raise RuntimeError("Response snapshot has conflicting output identities.")
                    self._completed_output[index] = replacement.model_copy(deep=True)
        elif t == "response.output_item.added":
            if getattr(event, "item", None) is None:
                return snapshot
            if event.output_index in self._output_items:
                raise RuntimeError("Duplicate response output index %d." % event.output_index)
            if event.item.type == "function_call":
                snapshot.output.append(_brevi_sdk.build(_ty.ParsedResponseFunctionToolCall, **event.item.to_dict()))
            elif event.item.type == "message":
                snapshot.output.append(_brevi_sdk.build(_ty.ParsedResponseOutputMessage, **event.item.to_dict()))
            else:
                snapshot.output.append(event.item)
            self._output_items[event.output_index] = snapshot.output[-1]
        elif t == "response.content_part.added":
            output = self._get_output_item(event.output_index)
            if output.type == "message":
                if event.part.type == "output_text":
                    output.content.append(_brevi_sdk.build(_ty.ParsedResponseOutputText, **event.part.to_dict()))
                else:
                    output.content.append(event.part)
        elif t == "response.output_text.delta":
            output = self._get_output_item(event.output_index)
            if output.type == "message":
                content = output.content[event.content_index]
                content.text += event.delta
        elif t == "response.function_call_arguments.delta":
            output = self._get_output_item(event.output_index)
            if output.type == "function_call":
                output.arguments += event.delta
        elif t == "response.output_item.done":
            added = self._output_items.get(event.output_index)
            if added is not None and (_output_identity(added) != _output_identity(event.item) or not _compatible_output_id(added, event.item)):
                raise RuntimeError("Completed output item identity changed.")
            identity = _output_identity(event.item)
            position = self._completed_positions.get(identity)
            if position is not None and position != event.output_index:
                raise RuntimeError("Response snapshot has conflicting output identities.")
            previous = self._completed_output.get(event.output_index)
            if previous is not None:
                del self._completed_positions[_output_identity(previous)]
            self._completed_output[event.output_index] = event.item
            self._completed_positions[identity] = event.output_index
        elif t == "response.completed":
            response = event.response
            final_items = {}
            for item in getattr(response, "output", None) or []:
                final_items[_output_identity(item)] = item
            for completed_item in self._completed_output.values():
                f = final_items.get(_output_identity(completed_item))
                if f is not None and not _compatible_output_id(completed_item, f):
                    raise RuntimeError("Completed response output identity changed.")
            for index, item in self._output_items.items():
                f = final_items.get(_output_identity(item))
                if index not in self._completed_output and (f is None or not _compatible_output_id(item, f)):
                    raise RuntimeError("Cannot complete response with unfinished output items.")
            if getattr(response, "output", None) is None:
                response = response.model_copy()
                response.output = [self._completed_output[i] for i in sorted(self._completed_output)]
            self._completed_response = _parse_response(self._text_format, self._input_tools, response)
        return snapshot

    def _get_output_item(self, output_index):
        if output_index not in self._output_items:
            raise RuntimeError("Received a content event for output index %d before receiving its output item" % output_index)
        return self._output_items[output_index]

    def _create_initial_response(self, event):
        if event.type != "response.created":
            raise RuntimeError("Expected to have received `response.created` before `%s`" % event.type)
        snapshot = _brevi_sdk.build(_ty.ParsedResponse, **event.response.to_dict())
        if getattr(snapshot, "output", None) is None:
            snapshot.output = []
        self._output_items = dict(enumerate(snapshot.output))
        return snapshot


class ResponseStream:
    def __init__(self, *, raw_stream, text_format, input_tools, starting_after):
        self._raw_stream = raw_stream
        self._response = raw_stream.response
        self._iterator = self._stream()
        self._state = ResponseStreamState(text_format=text_format, input_tools=input_tools)
        self._starting_after = starting_after

    def __next__(self):
        return self._iterator.__next__()

    def __iter__(self):
        for item in self._iterator:
            yield item

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def close(self):
        self._response.close()

    def get_final_response(self):
        self.until_done()
        response = self._state._completed_response
        if not response:
            raise RuntimeError("Didn't receive a `response.completed` event.")
        return response

    def until_done(self):
        for _ in self:
            pass
        return self

    def _stream(self):
        for sse_event in self._raw_stream:
            for event in self._state.handle_event(sse_event):
                if self._starting_after is None or event.sequence_number > self._starting_after:
                    yield event


class ResponseStreamManager:
    def __init__(self, api_request, *, text_format, input_tools, starting_after):
        self._stream = None
        self._api_request = api_request
        self._text_format = text_format
        self._input_tools = input_tools
        self._starting_after = starting_after

    def __enter__(self):
        self._stream = ResponseStream(raw_stream=self._api_request(), text_format=self._text_format, input_tools=self._input_tools,
                                      starting_after=self._starting_after)
        return self._stream

    def __exit__(self, exc_type, exc, tb):
        if self._stream is not None:
            self._stream.close()


class AsyncResponseStream:
    def __init__(self, *, raw_stream, text_format, input_tools, starting_after):
        self._raw_stream = raw_stream
        self._response = raw_stream.response
        self._iterator = self._stream()
        self._state = ResponseStreamState(text_format=text_format, input_tools=input_tools)
        self._starting_after = starting_after

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
        await self._response.aclose()

    async def get_final_response(self):
        await self.until_done()
        response = self._state._completed_response
        if not response:
            raise RuntimeError("Didn't receive a `response.completed` event.")
        return response

    async def until_done(self):
        async for _ in self:
            pass
        return self

    async def _stream(self):
        async for sse_event in self._raw_stream:
            for event in self._state.handle_event(sse_event):
                if self._starting_after is None or event.sequence_number > self._starting_after:
                    yield event


class AsyncResponseStreamManager:
    def __init__(self, api_request, *, text_format, input_tools, starting_after):
        self._stream = None
        self._api_request = api_request
        self._text_format = text_format
        self._input_tools = input_tools
        self._starting_after = starting_after

    async def __aenter__(self):
        raw_stream = await self._api_request
        self._stream = AsyncResponseStream(raw_stream=raw_stream, text_format=self._text_format, input_tools=self._input_tools,
                                           starting_after=self._starting_after)
        return self._stream

    async def __aexit__(self, exc_type, exc, tb):
        if self._stream is not None:
            await self._stream.close()
