# Brevi's openai: the Python SDK's API (openai 3.46) for what agent programs use: OpenAI and AsyncOpenAI,
# chat.completions.create (also stream=True, tools), embeddings.create (the SDK's base64 default decoded as float32),
# models.list / retrieve, responses.create (also stream=True) with Response.output_text, with_options / copy, the
# SDK's errors, retries and request headers. Response objects are the SDK's own models
# (src/dynpy/py/_brevi_openai_types.py, written from the SDK by tools/gen_llm_types.py). It runs over Brevi's httpx.
# caps.rs lists what it has (OWN_OPENAI) and what sends a program to CPython's openai instead (OPENAI_LACKS: beta,
# audio, images, files, parse, realtime, providers, raw responses, ...).
# Its behaviour follows openai-python (Copyright OpenAI; Apache-2.0 licensed, as Brevi is), whose error messages it repeats.
import base64 as _b64
import json as _json

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

    async def retrieve(self, response_id, *, extra_headers=None, extra_query=None, extra_body=None, timeout=not_given):
        if not response_id:
            raise ValueError("Expected a non-empty value for `response_id` but received %r" % (response_id,))
        return await self._client.get("/responses/" + _brevi_sdk.quote_segment(response_id),
                                      options=_opts(extra_headers, extra_query, extra_body, timeout), cast_to=Response)
