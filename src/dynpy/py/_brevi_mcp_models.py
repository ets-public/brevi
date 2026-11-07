# The protocol types of mcp 2.3.0 (mcp-types 2.3.0) as models of Brevi's pydantic, written by tools/gen_mcp_types.py
# from the SDK's own models: do not edit.
from pydantic import BaseModel as _BaseModel, Field as _Field


class _MCPModel(_BaseModel):
    model_config = {'populate_by_name': True}


class MCPModel(_MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
MCPModel.__module__ = 'mcp_types'

class RequestParams(MCPModel):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
RequestParams.__module__ = 'mcp_types'

class PaginatedRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    cursor = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('cursor', ('union', (('str',), ('none',), ))), )
PaginatedRequestParams.__module__ = 'mcp_types'

class NotificationParams(MCPModel):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
NotificationParams.__module__ = 'mcp_types'

class Request(MCPModel):
    model_config = {'populate_by_name': True}
    method = _Field()
    params = _Field()
    __annotations__ = (('method', ('any', 'Any')), ('params', ('any', 'Any')), )
Request.__module__ = 'mcp_types'

class PaginatedRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field()
    params = _Field(None)
    __annotations__ = (('method', ('any', 'Any')), ('params', ('union', (('lazy', lambda: PaginatedRequestParams, 'PaginatedRequestParams'), ('none',), ))), )
PaginatedRequest.__module__ = 'mcp_types'

class Notification(MCPModel):
    model_config = {'populate_by_name': True}
    method = _Field()
    params = _Field()
    __annotations__ = (('method', ('any', 'Any')), ('params', ('any', 'Any')), )
Notification.__module__ = 'mcp_types'

class Result(MCPModel):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
Result.__module__ = 'mcp_types'

class PaginatedResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    next_cursor = _Field(None, alias='nextCursor')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('next_cursor', ('union', (('str',), ('none',), ))), )
PaginatedResult.__module__ = 'mcp_types'

class CacheableResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), )
CacheableResult.__module__ = 'mcp_types'

class EmptyResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    result_type = _Field(None, alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ('none',), ))), )
EmptyResult.__module__ = 'mcp_types'

class BaseMetadata(MCPModel):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), )
BaseMetadata.__module__ = 'mcp_types'

class Icon(MCPModel):
    model_config = {'populate_by_name': True}
    src = _Field()
    mime_type = _Field(None, alias='mimeType')
    sizes = _Field(None)
    theme = _Field(None)
    __annotations__ = (('src', ('str',)), ('mime_type', ('union', (('str',), ('none',), ))), ('sizes', ('union', (('list', ('str',), 'list', False), ('none',), ))), ('theme', ('union', (('literal', ('light', 'dark', )), ('none',), ))), )
Icon.__module__ = 'mcp_types'

class Implementation(BaseMetadata):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    version = _Field()
    description = _Field(None)
    website_url = _Field(None, alias='websiteUrl')
    icons = _Field(None)
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('version', ('str',)), ('description', ('union', (('str',), ('none',), ))), ('website_url', ('union', (('str',), ('none',), ))), ('icons', ('union', (('list', ('lazy', lambda: Icon, 'Icon'), 'list', False), ('none',), ))), )
Implementation.__module__ = 'mcp_types'

class RootsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    list_changed = _Field(None, alias='listChanged')
    __annotations__ = (('list_changed', ('union', (('bool',), ('none',), ))), )
RootsCapability.__module__ = 'mcp_types'

class SamplingContextCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
SamplingContextCapability.__module__ = 'mcp_types'

class SamplingToolsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
SamplingToolsCapability.__module__ = 'mcp_types'

class FormElicitationCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
FormElicitationCapability.__module__ = 'mcp_types'

class UrlElicitationCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
UrlElicitationCapability.__module__ = 'mcp_types'

class ElicitationCapability(MCPModel):
    model_config = {'populate_by_name': True}
    form = _Field(None)
    url = _Field(None)
    __annotations__ = (('form', ('union', (('lazy', lambda: FormElicitationCapability, 'FormElicitationCapability'), ('none',), ))), ('url', ('union', (('lazy', lambda: UrlElicitationCapability, 'UrlElicitationCapability'), ('none',), ))), )
ElicitationCapability.__module__ = 'mcp_types'

class SamplingCapability(MCPModel):
    model_config = {'populate_by_name': True}
    context = _Field(None)
    tools = _Field(None)
    __annotations__ = (('context', ('union', (('lazy', lambda: SamplingContextCapability, 'SamplingContextCapability'), ('none',), ))), ('tools', ('union', (('lazy', lambda: SamplingToolsCapability, 'SamplingToolsCapability'), ('none',), ))), )
SamplingCapability.__module__ = 'mcp_types'

class TasksListCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
TasksListCapability.__module__ = 'mcp_types'

class TasksCancelCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
TasksCancelCapability.__module__ = 'mcp_types'

class TasksCreateMessageCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
TasksCreateMessageCapability.__module__ = 'mcp_types'

class TasksSamplingCapability(MCPModel):
    model_config = {'populate_by_name': True}
    create_message = _Field(None, alias='createMessage')
    __annotations__ = (('create_message', ('union', (('lazy', lambda: TasksCreateMessageCapability, 'TasksCreateMessageCapability'), ('none',), ))), )
TasksSamplingCapability.__module__ = 'mcp_types'

class TasksCreateElicitationCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
TasksCreateElicitationCapability.__module__ = 'mcp_types'

class TasksElicitationCapability(MCPModel):
    model_config = {'populate_by_name': True}
    create = _Field(None)
    __annotations__ = (('create', ('union', (('lazy', lambda: TasksCreateElicitationCapability, 'TasksCreateElicitationCapability'), ('none',), ))), )
TasksElicitationCapability.__module__ = 'mcp_types'

class ClientTasksRequestsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    sampling = _Field(None)
    elicitation = _Field(None)
    __annotations__ = (('sampling', ('union', (('lazy', lambda: TasksSamplingCapability, 'TasksSamplingCapability'), ('none',), ))), ('elicitation', ('union', (('lazy', lambda: TasksElicitationCapability, 'TasksElicitationCapability'), ('none',), ))), )
ClientTasksRequestsCapability.__module__ = 'mcp_types'

class ClientTasksCapability(MCPModel):
    model_config = {'populate_by_name': True}
    list = _Field(None)
    cancel = _Field(None)
    requests = _Field(None)
    __annotations__ = (('list', ('union', (('lazy', lambda: TasksListCapability, 'TasksListCapability'), ('none',), ))), ('cancel', ('union', (('lazy', lambda: TasksCancelCapability, 'TasksCancelCapability'), ('none',), ))), ('requests', ('union', (('lazy', lambda: ClientTasksRequestsCapability, 'ClientTasksRequestsCapability'), ('none',), ))), )
ClientTasksCapability.__module__ = 'mcp_types'

class ClientCapabilities(MCPModel):
    model_config = {'populate_by_name': True}
    experimental = _Field(None)
    sampling = _Field(None)
    elicitation = _Field(None)
    roots = _Field(None)
    extensions = _Field(None)
    tasks = _Field(None)
    __annotations__ = (('experimental', ('union', (('dict', ('str',), ('dict', ('str',), ('any', 'Any'), 'dict', False), 'dict', False), ('none',), ))), ('sampling', ('union', (('lazy', lambda: SamplingCapability, 'SamplingCapability'), ('none',), ))), ('elicitation', ('union', (('lazy', lambda: ElicitationCapability, 'ElicitationCapability'), ('none',), ))), ('roots', ('union', (('lazy', lambda: RootsCapability, 'RootsCapability'), ('none',), ))), ('extensions', ('union', (('dict', ('str',), ('dict', ('str',), ('any', 'Any'), 'dict', False), 'dict', False), ('none',), ))), ('tasks', ('union', (('lazy', lambda: ClientTasksCapability, 'ClientTasksCapability'), ('none',), ))), )
ClientCapabilities.__module__ = 'mcp_types'

class UnsupportedProtocolVersionErrorData(MCPModel):
    model_config = {'populate_by_name': True}
    supported = _Field()
    requested = _Field()
    __annotations__ = (('supported', ('list', ('str',), 'list', False)), ('requested', ('str',)), )
UnsupportedProtocolVersionErrorData.__module__ = 'mcp_types'

class MissingRequiredClientCapabilityErrorData(MCPModel):
    model_config = {'populate_by_name': True}
    required_capabilities = _Field(alias='requiredCapabilities')
    __annotations__ = (('required_capabilities', ('lazy', lambda: ClientCapabilities, 'ClientCapabilities')), )
MissingRequiredClientCapabilityErrorData.__module__ = 'mcp_types'

class PromptsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    list_changed = _Field(None, alias='listChanged')
    __annotations__ = (('list_changed', ('union', (('bool',), ('none',), ))), )
PromptsCapability.__module__ = 'mcp_types'

class ResourcesCapability(MCPModel):
    model_config = {'populate_by_name': True}
    subscribe = _Field(None)
    list_changed = _Field(None, alias='listChanged')
    __annotations__ = (('subscribe', ('union', (('bool',), ('none',), ))), ('list_changed', ('union', (('bool',), ('none',), ))), )
ResourcesCapability.__module__ = 'mcp_types'

class ToolsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    list_changed = _Field(None, alias='listChanged')
    __annotations__ = (('list_changed', ('union', (('bool',), ('none',), ))), )
ToolsCapability.__module__ = 'mcp_types'

class LoggingCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
LoggingCapability.__module__ = 'mcp_types'

class CompletionsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
CompletionsCapability.__module__ = 'mcp_types'

class TasksCallCapability(MCPModel):
    model_config = {'populate_by_name': True}
    __annotations__ = ()
TasksCallCapability.__module__ = 'mcp_types'

class TasksToolsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    call = _Field(None)
    __annotations__ = (('call', ('union', (('lazy', lambda: TasksCallCapability, 'TasksCallCapability'), ('none',), ))), )
TasksToolsCapability.__module__ = 'mcp_types'

class ServerTasksRequestsCapability(MCPModel):
    model_config = {'populate_by_name': True}
    tools = _Field(None)
    __annotations__ = (('tools', ('union', (('lazy', lambda: TasksToolsCapability, 'TasksToolsCapability'), ('none',), ))), )
ServerTasksRequestsCapability.__module__ = 'mcp_types'

class ServerTasksCapability(MCPModel):
    model_config = {'populate_by_name': True}
    list = _Field(None)
    cancel = _Field(None)
    requests = _Field(None)
    __annotations__ = (('list', ('union', (('lazy', lambda: TasksListCapability, 'TasksListCapability'), ('none',), ))), ('cancel', ('union', (('lazy', lambda: TasksCancelCapability, 'TasksCancelCapability'), ('none',), ))), ('requests', ('union', (('lazy', lambda: ServerTasksRequestsCapability, 'ServerTasksRequestsCapability'), ('none',), ))), )
ServerTasksCapability.__module__ = 'mcp_types'

class ServerCapabilities(MCPModel):
    model_config = {'populate_by_name': True}
    experimental = _Field(None)
    logging = _Field(None)
    prompts = _Field(None)
    resources = _Field(None)
    tools = _Field(None)
    completions = _Field(None)
    extensions = _Field(None)
    tasks = _Field(None)
    __annotations__ = (('experimental', ('union', (('dict', ('str',), ('dict', ('str',), ('any', 'Any'), 'dict', False), 'dict', False), ('none',), ))), ('logging', ('union', (('lazy', lambda: LoggingCapability, 'LoggingCapability'), ('none',), ))), ('prompts', ('union', (('lazy', lambda: PromptsCapability, 'PromptsCapability'), ('none',), ))), ('resources', ('union', (('lazy', lambda: ResourcesCapability, 'ResourcesCapability'), ('none',), ))), ('tools', ('union', (('lazy', lambda: ToolsCapability, 'ToolsCapability'), ('none',), ))), ('completions', ('union', (('lazy', lambda: CompletionsCapability, 'CompletionsCapability'), ('none',), ))), ('extensions', ('union', (('dict', ('str',), ('dict', ('str',), ('any', 'Any'), 'dict', False), 'dict', False), ('none',), ))), ('tasks', ('union', (('lazy', lambda: ServerTasksCapability, 'ServerTasksCapability'), ('none',), ))), )
ServerCapabilities.__module__ = 'mcp_types'

class InitializeRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    protocol_version = _Field(alias='protocolVersion')
    capabilities = _Field()
    client_info = _Field(alias='clientInfo')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('protocol_version', ('str',)), ('capabilities', ('lazy', lambda: ClientCapabilities, 'ClientCapabilities')), ('client_info', ('lazy', lambda: Implementation, 'Implementation')), )
InitializeRequestParams.__module__ = 'mcp_types'

class InitializeRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('initialize')
    params = _Field()
    __annotations__ = (('method', ('literal', ('initialize', ))), ('params', ('lazy', lambda: InitializeRequestParams, 'InitializeRequestParams')), )
InitializeRequest.__module__ = 'mcp_types'

class InitializeResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    protocol_version = _Field(alias='protocolVersion')
    capabilities = _Field()
    server_info = _Field(alias='serverInfo')
    instructions = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('protocol_version', ('str',)), ('capabilities', ('lazy', lambda: ServerCapabilities, 'ServerCapabilities')), ('server_info', ('lazy', lambda: Implementation, 'Implementation')), ('instructions', ('union', (('str',), ('none',), ))), )
InitializeResult.__module__ = 'mcp_types'

class InitializedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/initialized')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('notifications/initialized', ))), ('params', ('union', (('lazy', lambda: NotificationParams, 'NotificationParams'), ('none',), ))), )
InitializedNotification.__module__ = 'mcp_types'

class PingRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('ping')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('ping', ))), ('params', ('union', (('lazy', lambda: RequestParams, 'RequestParams'), ('none',), ))), )
PingRequest.__module__ = 'mcp_types'

class DiscoverRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('server/discover')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('server/discover', ))), ('params', ('union', (('lazy', lambda: RequestParams, 'RequestParams'), ('none',), ))), )
DiscoverRequest.__module__ = 'mcp_types'

class DiscoverResult(CacheableResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    supported_versions = _Field(alias='supportedVersions')
    capabilities = _Field()
    instructions = _Field(None)
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), ('supported_versions', ('list', ('str',), 'list', False)), ('capabilities', ('lazy', lambda: ServerCapabilities, 'ServerCapabilities')), ('instructions', ('union', (('str',), ('none',), ))), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
DiscoverResult.__module__ = 'mcp_types'

class ToolExecution(MCPModel):
    model_config = {'populate_by_name': True}
    task_support = _Field(None, alias='taskSupport')
    __annotations__ = (('task_support', ('union', (('literal', ('forbidden', 'optional', 'required', )), ('none',), ))), )
ToolExecution.__module__ = 'mcp_types'

class TaskMetadata(MCPModel):
    model_config = {'populate_by_name': True}
    ttl = _Field(None)
    __annotations__ = (('ttl', ('union', (('int',), ('none',), ))), )
TaskMetadata.__module__ = 'mcp_types'

class RelatedTaskMetadata(MCPModel):
    model_config = {'populate_by_name': True}
    task_id = _Field(alias='taskId')
    __annotations__ = (('task_id', ('str',)), )
RelatedTaskMetadata.__module__ = 'mcp_types'

class Task(MCPModel):
    model_config = {'populate_by_name': True}
    task_id = _Field(alias='taskId')
    status = _Field()
    status_message = _Field(None, alias='statusMessage')
    created_at = _Field(alias='createdAt')
    last_updated_at = _Field(alias='lastUpdatedAt')
    ttl = _Field()
    poll_interval = _Field(None, alias='pollInterval')
    __annotations__ = (('task_id', ('str',)), ('status', ('literal', ('working', 'input_required', 'completed', 'failed', 'cancelled', ))), ('status_message', ('union', (('str',), ('none',), ))), ('created_at', ('str',)), ('last_updated_at', ('str',)), ('ttl', ('union', (('int',), ('none',), ))), ('poll_interval', ('union', (('int',), ('none',), ))), )
Task.__module__ = 'mcp_types'

class CreateTaskResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    task = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('task', ('lazy', lambda: Task, 'Task')), )
CreateTaskResult.__module__ = 'mcp_types'

class GetTaskRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    task_id = _Field(alias='taskId')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('task_id', ('str',)), )
GetTaskRequestParams.__module__ = 'mcp_types'

class GetTaskRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('tasks/get')
    params = _Field()
    __annotations__ = (('method', ('literal', ('tasks/get', ))), ('params', ('lazy', lambda: GetTaskRequestParams, 'GetTaskRequestParams')), )
GetTaskRequest.__module__ = 'mcp_types'

class GetTaskResult(Result):
    model_config = {'populate_by_name': True}
    task_id = _Field(alias='taskId')
    status = _Field()
    status_message = _Field(None, alias='statusMessage')
    created_at = _Field(alias='createdAt')
    last_updated_at = _Field(alias='lastUpdatedAt')
    ttl = _Field()
    poll_interval = _Field(None, alias='pollInterval')
    meta = _Field(None, alias='_meta')
    __annotations__ = (('task_id', ('str',)), ('status', ('literal', ('working', 'input_required', 'completed', 'failed', 'cancelled', ))), ('status_message', ('union', (('str',), ('none',), ))), ('created_at', ('str',)), ('last_updated_at', ('str',)), ('ttl', ('union', (('int',), ('none',), ))), ('poll_interval', ('union', (('int',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
GetTaskResult.__module__ = 'mcp_types'

class CancelTaskRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    task_id = _Field(alias='taskId')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('task_id', ('str',)), )
CancelTaskRequestParams.__module__ = 'mcp_types'

class CancelTaskRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('tasks/cancel')
    params = _Field()
    __annotations__ = (('method', ('literal', ('tasks/cancel', ))), ('params', ('lazy', lambda: CancelTaskRequestParams, 'CancelTaskRequestParams')), )
CancelTaskRequest.__module__ = 'mcp_types'

class CancelTaskResult(Result):
    model_config = {'populate_by_name': True}
    task_id = _Field(alias='taskId')
    status = _Field()
    status_message = _Field(None, alias='statusMessage')
    created_at = _Field(alias='createdAt')
    last_updated_at = _Field(alias='lastUpdatedAt')
    ttl = _Field()
    poll_interval = _Field(None, alias='pollInterval')
    meta = _Field(None, alias='_meta')
    __annotations__ = (('task_id', ('str',)), ('status', ('literal', ('working', 'input_required', 'completed', 'failed', 'cancelled', ))), ('status_message', ('union', (('str',), ('none',), ))), ('created_at', ('str',)), ('last_updated_at', ('str',)), ('ttl', ('union', (('int',), ('none',), ))), ('poll_interval', ('union', (('int',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
CancelTaskResult.__module__ = 'mcp_types'

class TaskStatusNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    task_id = _Field(alias='taskId')
    status = _Field()
    status_message = _Field(None, alias='statusMessage')
    created_at = _Field(alias='createdAt')
    last_updated_at = _Field(alias='lastUpdatedAt')
    ttl = _Field()
    poll_interval = _Field(None, alias='pollInterval')
    meta = _Field(None, alias='_meta')
    __annotations__ = (('task_id', ('str',)), ('status', ('literal', ('working', 'input_required', 'completed', 'failed', 'cancelled', ))), ('status_message', ('union', (('str',), ('none',), ))), ('created_at', ('str',)), ('last_updated_at', ('str',)), ('ttl', ('union', (('int',), ('none',), ))), ('poll_interval', ('union', (('int',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
TaskStatusNotificationParams.__module__ = 'mcp_types'

class TaskStatusNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/tasks/status')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/tasks/status', ))), ('params', ('lazy', lambda: TaskStatusNotificationParams, 'TaskStatusNotificationParams')), )
TaskStatusNotification.__module__ = 'mcp_types'

class GetTaskPayloadRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    task_id = _Field(alias='taskId')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('task_id', ('str',)), )
GetTaskPayloadRequestParams.__module__ = 'mcp_types'

class GetTaskPayloadRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('tasks/result')
    params = _Field()
    __annotations__ = (('method', ('literal', ('tasks/result', ))), ('params', ('lazy', lambda: GetTaskPayloadRequestParams, 'GetTaskPayloadRequestParams')), )
GetTaskPayloadRequest.__module__ = 'mcp_types'

class GetTaskPayloadResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
GetTaskPayloadResult.__module__ = 'mcp_types'

class ListTasksRequest(PaginatedRequest):
    model_config = {'populate_by_name': True}
    method = _Field('tasks/list')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('tasks/list', ))), ('params', ('union', (('lazy', lambda: PaginatedRequestParams, 'PaginatedRequestParams'), ('none',), ))), )
ListTasksRequest.__module__ = 'mcp_types'

class ListTasksResult(PaginatedResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    next_cursor = _Field(None, alias='nextCursor')
    tasks = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('next_cursor', ('union', (('str',), ('none',), ))), ('tasks', ('list', ('lazy', lambda: Task, 'Task'), 'list', False)), )
ListTasksResult.__module__ = 'mcp_types'

class ProgressNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    progress_token = _Field(alias='progressToken')
    progress = _Field()
    total = _Field(None)
    message = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('progress_token', ('union', (('str',), ('int',), ))), ('progress', ('float',)), ('total', ('union', (('float',), ('none',), ))), ('message', ('union', (('str',), ('none',), ))), )
ProgressNotificationParams.__module__ = 'mcp_types'

class ProgressNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/progress')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/progress', ))), ('params', ('lazy', lambda: ProgressNotificationParams, 'ProgressNotificationParams')), )
ProgressNotification.__module__ = 'mcp_types'

class ListResourcesRequest(PaginatedRequest):
    model_config = {'populate_by_name': True}
    method = _Field('resources/list')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('resources/list', ))), ('params', ('union', (('lazy', lambda: PaginatedRequestParams, 'PaginatedRequestParams'), ('none',), ))), )
ListResourcesRequest.__module__ = 'mcp_types'

class Annotations(MCPModel):
    model_config = {'populate_by_name': True}
    audience = _Field(None)
    priority = _Field(None)
    last_modified = _Field(None, alias='lastModified')
    __annotations__ = (('audience', ('union', (('list', ('literal', ('user', 'assistant', )), 'list', False), ('none',), ))), ('priority', ('union', (('float',), ('none',), ))), ('last_modified', ('union', (('str',), ('none',), ))), )
Annotations.__module__ = 'mcp_types'

class Resource(BaseMetadata):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    uri = _Field()
    description = _Field(None)
    mime_type = _Field(None, alias='mimeType')
    size = _Field(None)
    icons = _Field(None)
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('uri', ('str',)), ('description', ('union', (('str',), ('none',), ))), ('mime_type', ('union', (('str',), ('none',), ))), ('size', ('union', (('int',), ('none',), ))), ('icons', ('union', (('list', ('lazy', lambda: Icon, 'Icon'), 'list', False), ('none',), ))), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
Resource.__module__ = 'mcp_types'

class ResourceTemplate(BaseMetadata):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    uri_template = _Field(alias='uriTemplate')
    description = _Field(None)
    mime_type = _Field(None, alias='mimeType')
    icons = _Field(None)
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('uri_template', ('str',)), ('description', ('union', (('str',), ('none',), ))), ('mime_type', ('union', (('str',), ('none',), ))), ('icons', ('union', (('list', ('lazy', lambda: Icon, 'Icon'), 'list', False), ('none',), ))), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
ResourceTemplate.__module__ = 'mcp_types'

class ListResourcesResult(PaginatedResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    next_cursor = _Field(None, alias='nextCursor')
    resources = _Field()
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), ('next_cursor', ('union', (('str',), ('none',), ))), ('resources', ('list', ('lazy', lambda: Resource, 'Resource'), 'list', False)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
ListResourcesResult.__module__ = 'mcp_types'

class ListResourceTemplatesRequest(PaginatedRequest):
    model_config = {'populate_by_name': True}
    method = _Field('resources/templates/list')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('resources/templates/list', ))), ('params', ('union', (('lazy', lambda: PaginatedRequestParams, 'PaginatedRequestParams'), ('none',), ))), )
ListResourceTemplatesRequest.__module__ = 'mcp_types'

class ListResourceTemplatesResult(PaginatedResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    next_cursor = _Field(None, alias='nextCursor')
    resource_templates = _Field(alias='resourceTemplates')
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), ('next_cursor', ('union', (('str',), ('none',), ))), ('resource_templates', ('list', ('lazy', lambda: ResourceTemplate, 'ResourceTemplate'), 'list', False)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
ListResourceTemplatesResult.__module__ = 'mcp_types'

class TextContent(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('text')
    text = _Field()
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('type', ('literal', ('text', ))), ('text', ('str',)), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
TextContent.__module__ = 'mcp_types'

class ImageContent(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('image')
    data = _Field()
    mime_type = _Field(alias='mimeType')
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('type', ('literal', ('image', ))), ('data', ('str',)), ('mime_type', ('str',)), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
ImageContent.__module__ = 'mcp_types'

class AudioContent(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('audio')
    data = _Field()
    mime_type = _Field(alias='mimeType')
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('type', ('literal', ('audio', ))), ('data', ('str',)), ('mime_type', ('str',)), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
AudioContent.__module__ = 'mcp_types'

class CreateMessageResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    role = _Field()
    content = _Field()
    model = _Field()
    stop_reason = _Field(None, alias='stopReason')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('role', ('literal', ('user', 'assistant', ))), ('content', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ))), ('model', ('str',)), ('stop_reason', ('union', (('literal', ('endTurn', 'stopSequence', 'maxTokens', 'toolUse', )), ('str',), ('none',), ))), )
CreateMessageResult.__module__ = 'mcp_types'

class ToolUseContent(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('tool_use')
    name = _Field()
    id = _Field()
    input = _Field()
    meta = _Field(None, alias='_meta')
    __annotations__ = (('type', ('literal', ('tool_use', ))), ('name', ('str',)), ('id', ('str',)), ('input', ('dict', ('str',), ('any', 'Any'), 'dict', False)), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
ToolUseContent.__module__ = 'mcp_types'

class ResourceLink(Resource):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    uri = _Field()
    description = _Field(None)
    mime_type = _Field(None, alias='mimeType')
    size = _Field(None)
    icons = _Field(None)
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    type = _Field('resource_link')
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('uri', ('str',)), ('description', ('union', (('str',), ('none',), ))), ('mime_type', ('union', (('str',), ('none',), ))), ('size', ('union', (('int',), ('none',), ))), ('icons', ('union', (('list', ('lazy', lambda: Icon, 'Icon'), 'list', False), ('none',), ))), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('type', ('literal', ('resource_link', ))), )
ResourceLink.__module__ = 'mcp_types'

class ResourceContents(MCPModel):
    model_config = {'populate_by_name': True}
    uri = _Field()
    mime_type = _Field(None, alias='mimeType')
    meta = _Field(None, alias='_meta')
    __annotations__ = (('uri', ('str',)), ('mime_type', ('union', (('str',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
ResourceContents.__module__ = 'mcp_types'

class TextResourceContents(ResourceContents):
    model_config = {'populate_by_name': True}
    uri = _Field()
    mime_type = _Field(None, alias='mimeType')
    meta = _Field(None, alias='_meta')
    text = _Field()
    __annotations__ = (('uri', ('str',)), ('mime_type', ('union', (('str',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('text', ('str',)), )
TextResourceContents.__module__ = 'mcp_types'

class BlobResourceContents(ResourceContents):
    model_config = {'populate_by_name': True}
    uri = _Field()
    mime_type = _Field(None, alias='mimeType')
    meta = _Field(None, alias='_meta')
    blob = _Field()
    __annotations__ = (('uri', ('str',)), ('mime_type', ('union', (('str',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('blob', ('str',)), )
BlobResourceContents.__module__ = 'mcp_types'

class EmbeddedResource(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('resource')
    resource = _Field()
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('type', ('literal', ('resource', ))), ('resource', ('union', (('lazy', lambda: TextResourceContents, 'TextResourceContents'), ('lazy', lambda: BlobResourceContents, 'BlobResourceContents'), ))), ('annotations', ('union', (('lazy', lambda: Annotations, 'Annotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
EmbeddedResource.__module__ = 'mcp_types'

class ToolResultContent(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('tool_result')
    tool_use_id = _Field(alias='toolUseId')
    content = _Field([])
    structured_content = _Field(None, alias='structuredContent')
    is_error = _Field(None, alias='isError')
    meta = _Field(None, alias='_meta')
    __annotations__ = (('type', ('literal', ('tool_result', ))), ('tool_use_id', ('str',)), ('content', ('list', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ResourceLink, 'ResourceLink'), ('lazy', lambda: EmbeddedResource, 'EmbeddedResource'), )), 'list', False)), ('structured_content', ('any', 'Any')), ('is_error', ('union', (('bool',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
ToolResultContent.__module__ = 'mcp_types'

class CreateMessageResultWithTools(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    role = _Field()
    content = _Field()
    model = _Field()
    stop_reason = _Field(None, alias='stopReason')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('role', ('literal', ('user', 'assistant', ))), ('content', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ToolUseContent, 'ToolUseContent'), ('lazy', lambda: ToolResultContent, 'ToolResultContent'), ('list', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ToolUseContent, 'ToolUseContent'), ('lazy', lambda: ToolResultContent, 'ToolResultContent'), )), 'list', False), ))), ('model', ('str',)), ('stop_reason', ('union', (('literal', ('endTurn', 'stopSequence', 'maxTokens', 'toolUse', )), ('str',), ('none',), ))), )
CreateMessageResultWithTools.__module__ = 'mcp_types'

class Root(MCPModel):
    model_config = {'populate_by_name': True}
    uri = _Field()
    name = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('uri', ('str',)), ('name', ('union', (('str',), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
Root.__module__ = 'mcp_types'

class ListRootsResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    roots = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('roots', ('list', ('lazy', lambda: Root, 'Root'), 'list', False)), )
ListRootsResult.__module__ = 'mcp_types'

class ElicitResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    action = _Field()
    content = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('action', ('literal', ('accept', 'decline', 'cancel', ))), ('content', ('union', (('dict', ('str',), ('union', (('str',), ('int',), ('float',), ('bool',), ('list', ('str',), 'list', False), ('none',), )), 'dict', False), ('none',), ))), )
ElicitResult.__module__ = 'mcp_types'

class InputResponseRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    input_responses = _Field(None, alias='inputResponses')
    request_state = _Field(None, alias='requestState')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('input_responses', ('union', (('dict', ('str',), ('union', (('lazy', lambda: CreateMessageResult, 'CreateMessageResult'), ('lazy', lambda: CreateMessageResultWithTools, 'CreateMessageResultWithTools'), ('lazy', lambda: ListRootsResult, 'ListRootsResult'), ('lazy', lambda: ElicitResult, 'ElicitResult'), )), 'dict', False), ('none',), ))), ('request_state', ('union', (('str',), ('none',), ))), )
InputResponseRequestParams.__module__ = 'mcp_types'

class ReadResourceRequestParams(InputResponseRequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    input_responses = _Field(None, alias='inputResponses')
    request_state = _Field(None, alias='requestState')
    uri = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('input_responses', ('union', (('dict', ('str',), ('union', (('lazy', lambda: CreateMessageResult, 'CreateMessageResult'), ('lazy', lambda: CreateMessageResultWithTools, 'CreateMessageResultWithTools'), ('lazy', lambda: ListRootsResult, 'ListRootsResult'), ('lazy', lambda: ElicitResult, 'ElicitResult'), )), 'dict', False), ('none',), ))), ('request_state', ('union', (('str',), ('none',), ))), ('uri', ('str',)), )
ReadResourceRequestParams.__module__ = 'mcp_types'

class ReadResourceRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('resources/read')
    params = _Field()
    __annotations__ = (('method', ('literal', ('resources/read', ))), ('params', ('lazy', lambda: ReadResourceRequestParams, 'ReadResourceRequestParams')), )
ReadResourceRequest.__module__ = 'mcp_types'

class ReadResourceResult(CacheableResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    contents = _Field()
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), ('contents', ('list', ('union', (('lazy', lambda: TextResourceContents, 'TextResourceContents'), ('lazy', lambda: BlobResourceContents, 'BlobResourceContents'), )), 'list', False)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
ReadResourceResult.__module__ = 'mcp_types'

class ResourceListChangedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/resources/list_changed')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('notifications/resources/list_changed', ))), ('params', ('union', (('lazy', lambda: NotificationParams, 'NotificationParams'), ('none',), ))), )
ResourceListChangedNotification.__module__ = 'mcp_types'

class SubscribeRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    uri = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('uri', ('str',)), )
SubscribeRequestParams.__module__ = 'mcp_types'

class SubscribeRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('resources/subscribe')
    params = _Field()
    __annotations__ = (('method', ('literal', ('resources/subscribe', ))), ('params', ('lazy', lambda: SubscribeRequestParams, 'SubscribeRequestParams')), )
SubscribeRequest.__module__ = 'mcp_types'

class UnsubscribeRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    uri = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('uri', ('str',)), )
UnsubscribeRequestParams.__module__ = 'mcp_types'

class UnsubscribeRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('resources/unsubscribe')
    params = _Field()
    __annotations__ = (('method', ('literal', ('resources/unsubscribe', ))), ('params', ('lazy', lambda: UnsubscribeRequestParams, 'UnsubscribeRequestParams')), )
UnsubscribeRequest.__module__ = 'mcp_types'

class ResourceUpdatedNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    uri = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('uri', ('str',)), )
ResourceUpdatedNotificationParams.__module__ = 'mcp_types'

class ResourceUpdatedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/resources/updated')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/resources/updated', ))), ('params', ('lazy', lambda: ResourceUpdatedNotificationParams, 'ResourceUpdatedNotificationParams')), )
ResourceUpdatedNotification.__module__ = 'mcp_types'

class SubscriptionFilter(MCPModel):
    model_config = {'populate_by_name': True, 'extra': 'allow'}
    tools_list_changed = _Field(None, alias='toolsListChanged')
    prompts_list_changed = _Field(None, alias='promptsListChanged')
    resources_list_changed = _Field(None, alias='resourcesListChanged')
    resource_subscriptions = _Field(None, alias='resourceSubscriptions')
    __annotations__ = (('tools_list_changed', ('union', (('bool',), ('none',), ))), ('prompts_list_changed', ('union', (('bool',), ('none',), ))), ('resources_list_changed', ('union', (('bool',), ('none',), ))), ('resource_subscriptions', ('union', (('list', ('str',), 'list', False), ('none',), ))), )
SubscriptionFilter.__module__ = 'mcp_types'

class SubscriptionsListenRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    notifications = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('notifications', ('lazy', lambda: SubscriptionFilter, 'SubscriptionFilter')), )
SubscriptionsListenRequestParams.__module__ = 'mcp_types'

class SubscriptionsListenRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('subscriptions/listen')
    params = _Field()
    __annotations__ = (('method', ('literal', ('subscriptions/listen', ))), ('params', ('lazy', lambda: SubscriptionsListenRequestParams, 'SubscriptionsListenRequestParams')), )
SubscriptionsListenRequest.__module__ = 'mcp_types'

class SubscriptionsAcknowledgedNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    notifications = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('notifications', ('lazy', lambda: SubscriptionFilter, 'SubscriptionFilter')), )
SubscriptionsAcknowledgedNotificationParams.__module__ = 'mcp_types'

class SubscriptionsAcknowledgedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/subscriptions/acknowledged')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/subscriptions/acknowledged', ))), ('params', ('lazy', lambda: SubscriptionsAcknowledgedNotificationParams, 'SubscriptionsAcknowledgedNotificationParams')), )
SubscriptionsAcknowledgedNotification.__module__ = 'mcp_types'

class SubscriptionsListenResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
SubscriptionsListenResult.__module__ = 'mcp_types'

class ListPromptsRequest(PaginatedRequest):
    model_config = {'populate_by_name': True}
    method = _Field('prompts/list')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('prompts/list', ))), ('params', ('union', (('lazy', lambda: PaginatedRequestParams, 'PaginatedRequestParams'), ('none',), ))), )
ListPromptsRequest.__module__ = 'mcp_types'

class PromptArgument(BaseMetadata):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    description = _Field(None)
    required = _Field(None)
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('description', ('union', (('str',), ('none',), ))), ('required', ('union', (('bool',), ('none',), ))), )
PromptArgument.__module__ = 'mcp_types'

class Prompt(BaseMetadata):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    description = _Field(None)
    arguments = _Field(None)
    icons = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('description', ('union', (('str',), ('none',), ))), ('arguments', ('union', (('list', ('lazy', lambda: PromptArgument, 'PromptArgument'), 'list', False), ('none',), ))), ('icons', ('union', (('list', ('lazy', lambda: Icon, 'Icon'), 'list', False), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
Prompt.__module__ = 'mcp_types'

class ListPromptsResult(PaginatedResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    next_cursor = _Field(None, alias='nextCursor')
    prompts = _Field()
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), ('next_cursor', ('union', (('str',), ('none',), ))), ('prompts', ('list', ('lazy', lambda: Prompt, 'Prompt'), 'list', False)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
ListPromptsResult.__module__ = 'mcp_types'

class GetPromptRequestParams(InputResponseRequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    input_responses = _Field(None, alias='inputResponses')
    request_state = _Field(None, alias='requestState')
    name = _Field()
    arguments = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('input_responses', ('union', (('dict', ('str',), ('union', (('lazy', lambda: CreateMessageResult, 'CreateMessageResult'), ('lazy', lambda: CreateMessageResultWithTools, 'CreateMessageResultWithTools'), ('lazy', lambda: ListRootsResult, 'ListRootsResult'), ('lazy', lambda: ElicitResult, 'ElicitResult'), )), 'dict', False), ('none',), ))), ('request_state', ('union', (('str',), ('none',), ))), ('name', ('str',)), ('arguments', ('union', (('dict', ('str',), ('str',), 'dict', False), ('none',), ))), )
GetPromptRequestParams.__module__ = 'mcp_types'

class GetPromptRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('prompts/get')
    params = _Field()
    __annotations__ = (('method', ('literal', ('prompts/get', ))), ('params', ('lazy', lambda: GetPromptRequestParams, 'GetPromptRequestParams')), )
GetPromptRequest.__module__ = 'mcp_types'

class SamplingMessage(MCPModel):
    model_config = {'populate_by_name': True}
    role = _Field()
    content = _Field()
    meta = _Field(None, alias='_meta')
    __annotations__ = (('role', ('literal', ('user', 'assistant', ))), ('content', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ToolUseContent, 'ToolUseContent'), ('lazy', lambda: ToolResultContent, 'ToolResultContent'), ('list', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ToolUseContent, 'ToolUseContent'), ('lazy', lambda: ToolResultContent, 'ToolResultContent'), )), 'list', False), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
SamplingMessage.__module__ = 'mcp_types'

class PromptMessage(MCPModel):
    model_config = {'populate_by_name': True}
    role = _Field()
    content = _Field()
    __annotations__ = (('role', ('literal', ('user', 'assistant', ))), ('content', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ResourceLink, 'ResourceLink'), ('lazy', lambda: EmbeddedResource, 'EmbeddedResource'), ))), )
PromptMessage.__module__ = 'mcp_types'

class GetPromptResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    description = _Field(None)
    messages = _Field()
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('description', ('union', (('str',), ('none',), ))), ('messages', ('list', ('lazy', lambda: PromptMessage, 'PromptMessage'), 'list', False)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
GetPromptResult.__module__ = 'mcp_types'

class PromptListChangedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/prompts/list_changed')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('notifications/prompts/list_changed', ))), ('params', ('union', (('lazy', lambda: NotificationParams, 'NotificationParams'), ('none',), ))), )
PromptListChangedNotification.__module__ = 'mcp_types'

class ListToolsRequest(PaginatedRequest):
    model_config = {'populate_by_name': True}
    method = _Field('tools/list')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('tools/list', ))), ('params', ('union', (('lazy', lambda: PaginatedRequestParams, 'PaginatedRequestParams'), ('none',), ))), )
ListToolsRequest.__module__ = 'mcp_types'

class ToolAnnotations(MCPModel):
    model_config = {'populate_by_name': True}
    title = _Field(None)
    read_only_hint = _Field(None, alias='readOnlyHint')
    destructive_hint = _Field(None, alias='destructiveHint')
    idempotent_hint = _Field(None, alias='idempotentHint')
    open_world_hint = _Field(None, alias='openWorldHint')
    __annotations__ = (('title', ('union', (('str',), ('none',), ))), ('read_only_hint', ('union', (('bool',), ('none',), ))), ('destructive_hint', ('union', (('bool',), ('none',), ))), ('idempotent_hint', ('union', (('bool',), ('none',), ))), ('open_world_hint', ('union', (('bool',), ('none',), ))), )
ToolAnnotations.__module__ = 'mcp_types'

class Tool(BaseMetadata):
    model_config = {'populate_by_name': True}
    name = _Field()
    title = _Field(None)
    description = _Field(None)
    input_schema = _Field(alias='inputSchema')
    execution = _Field(None)
    output_schema = _Field(None, alias='outputSchema')
    icons = _Field(None)
    annotations = _Field(None)
    meta = _Field(None, alias='_meta')
    __annotations__ = (('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), ('description', ('union', (('str',), ('none',), ))), ('input_schema', ('dict', ('str',), ('any', 'Any'), 'dict', False)), ('execution', ('union', (('lazy', lambda: ToolExecution, 'ToolExecution'), ('none',), ))), ('output_schema', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('icons', ('union', (('list', ('lazy', lambda: Icon, 'Icon'), 'list', False), ('none',), ))), ('annotations', ('union', (('lazy', lambda: ToolAnnotations, 'ToolAnnotations'), ('none',), ))), ('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
Tool.__module__ = 'mcp_types'

class ListToolsResult(PaginatedResult):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ttl_ms = _Field(0, alias='ttlMs')
    cache_scope = _Field('private', alias='cacheScope')
    next_cursor = _Field(None, alias='nextCursor')
    tools = _Field()
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ttl_ms', ('int',)), ('cache_scope', ('literal', ('public', 'private', ))), ('next_cursor', ('union', (('str',), ('none',), ))), ('tools', ('list', ('lazy', lambda: Tool, 'Tool'), 'list', False)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
ListToolsResult.__module__ = 'mcp_types'

class CallToolRequestParams(InputResponseRequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    input_responses = _Field(None, alias='inputResponses')
    request_state = _Field(None, alias='requestState')
    name = _Field()
    arguments = _Field(None)
    task = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('input_responses', ('union', (('dict', ('str',), ('union', (('lazy', lambda: CreateMessageResult, 'CreateMessageResult'), ('lazy', lambda: CreateMessageResultWithTools, 'CreateMessageResultWithTools'), ('lazy', lambda: ListRootsResult, 'ListRootsResult'), ('lazy', lambda: ElicitResult, 'ElicitResult'), )), 'dict', False), ('none',), ))), ('request_state', ('union', (('str',), ('none',), ))), ('name', ('str',)), ('arguments', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('task', ('union', (('lazy', lambda: TaskMetadata, 'TaskMetadata'), ('none',), ))), )
CallToolRequestParams.__module__ = 'mcp_types'

class CallToolRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('tools/call')
    params = _Field()
    __annotations__ = (('method', ('literal', ('tools/call', ))), ('params', ('lazy', lambda: CallToolRequestParams, 'CallToolRequestParams')), )
CallToolRequest.__module__ = 'mcp_types'

class CallToolResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    content = _Field()
    structured_content = _Field(None, alias='structuredContent')
    is_error = _Field(False, alias='isError')
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('content', ('list', ('union', (('lazy', lambda: TextContent, 'TextContent'), ('lazy', lambda: ImageContent, 'ImageContent'), ('lazy', lambda: AudioContent, 'AudioContent'), ('lazy', lambda: ResourceLink, 'ResourceLink'), ('lazy', lambda: EmbeddedResource, 'EmbeddedResource'), )), 'list', False)), ('structured_content', ('any', 'Any')), ('is_error', ('bool',)), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
CallToolResult.__module__ = 'mcp_types'

class ToolListChangedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/tools/list_changed')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('notifications/tools/list_changed', ))), ('params', ('union', (('lazy', lambda: NotificationParams, 'NotificationParams'), ('none',), ))), )
ToolListChangedNotification.__module__ = 'mcp_types'

class SetLevelRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    level = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('level', ('literal', ('debug', 'info', 'notice', 'warning', 'error', 'critical', 'alert', 'emergency', ))), )
SetLevelRequestParams.__module__ = 'mcp_types'

class SetLevelRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('logging/setLevel')
    params = _Field()
    __annotations__ = (('method', ('literal', ('logging/setLevel', ))), ('params', ('lazy', lambda: SetLevelRequestParams, 'SetLevelRequestParams')), )
SetLevelRequest.__module__ = 'mcp_types'

class LoggingMessageNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    level = _Field()
    logger = _Field(None)
    data = _Field()
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('level', ('literal', ('debug', 'info', 'notice', 'warning', 'error', 'critical', 'alert', 'emergency', ))), ('logger', ('union', (('str',), ('none',), ))), ('data', ('any', 'Any')), )
LoggingMessageNotificationParams.__module__ = 'mcp_types'

class LoggingMessageNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/message')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/message', ))), ('params', ('lazy', lambda: LoggingMessageNotificationParams, 'LoggingMessageNotificationParams')), )
LoggingMessageNotification.__module__ = 'mcp_types'

class ModelHint(MCPModel):
    model_config = {'populate_by_name': True}
    name = _Field(None)
    __annotations__ = (('name', ('union', (('str',), ('none',), ))), )
ModelHint.__module__ = 'mcp_types'

class ModelPreferences(MCPModel):
    model_config = {'populate_by_name': True}
    hints = _Field(None)
    cost_priority = _Field(None, alias='costPriority')
    speed_priority = _Field(None, alias='speedPriority')
    intelligence_priority = _Field(None, alias='intelligencePriority')
    __annotations__ = (('hints', ('union', (('list', ('lazy', lambda: ModelHint, 'ModelHint'), 'list', False), ('none',), ))), ('cost_priority', ('union', (('float',), ('none',), ))), ('speed_priority', ('union', (('float',), ('none',), ))), ('intelligence_priority', ('union', (('float',), ('none',), ))), )
ModelPreferences.__module__ = 'mcp_types'

class ToolChoice(MCPModel):
    model_config = {'populate_by_name': True}
    mode = _Field(None)
    __annotations__ = (('mode', ('union', (('literal', ('auto', 'required', 'none', )), ('none',), ))), )
ToolChoice.__module__ = 'mcp_types'

class CreateMessageRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    messages = _Field()
    model_preferences = _Field(None, alias='modelPreferences')
    system_prompt = _Field(None, alias='systemPrompt')
    include_context = _Field(None, alias='includeContext')
    temperature = _Field(None)
    max_tokens = _Field(alias='maxTokens')
    stop_sequences = _Field(None, alias='stopSequences')
    metadata = _Field(None)
    tools = _Field(None)
    tool_choice = _Field(None, alias='toolChoice')
    task = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('messages', ('list', ('lazy', lambda: SamplingMessage, 'SamplingMessage'), 'list', False)), ('model_preferences', ('union', (('lazy', lambda: ModelPreferences, 'ModelPreferences'), ('none',), ))), ('system_prompt', ('union', (('str',), ('none',), ))), ('include_context', ('union', (('literal', ('none', 'thisServer', 'allServers', )), ('none',), ))), ('temperature', ('union', (('float',), ('none',), ))), ('max_tokens', ('int',)), ('stop_sequences', ('union', (('list', ('str',), 'list', False), ('none',), ))), ('metadata', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('tools', ('union', (('list', ('lazy', lambda: Tool, 'Tool'), 'list', False), ('none',), ))), ('tool_choice', ('union', (('lazy', lambda: ToolChoice, 'ToolChoice'), ('none',), ))), ('task', ('union', (('lazy', lambda: TaskMetadata, 'TaskMetadata'), ('none',), ))), )
CreateMessageRequestParams.__module__ = 'mcp_types'

class CreateMessageRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('sampling/createMessage')
    params = _Field()
    __annotations__ = (('method', ('literal', ('sampling/createMessage', ))), ('params', ('lazy', lambda: CreateMessageRequestParams, 'CreateMessageRequestParams')), )
CreateMessageRequest.__module__ = 'mcp_types'

class ResourceTemplateReference(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('ref/resource')
    uri = _Field()
    __annotations__ = (('type', ('literal', ('ref/resource', ))), ('uri', ('str',)), )
ResourceTemplateReference.__module__ = 'mcp_types'

class PromptReference(MCPModel):
    model_config = {'populate_by_name': True}
    type = _Field('ref/prompt')
    name = _Field()
    title = _Field(None)
    __annotations__ = (('type', ('literal', ('ref/prompt', ))), ('name', ('str',)), ('title', ('union', (('str',), ('none',), ))), )
PromptReference.__module__ = 'mcp_types'

class CompletionArgument(MCPModel):
    model_config = {'populate_by_name': True}
    name = _Field()
    value = _Field()
    __annotations__ = (('name', ('str',)), ('value', ('str',)), )
CompletionArgument.__module__ = 'mcp_types'

class CompletionContext(MCPModel):
    model_config = {'populate_by_name': True}
    arguments = _Field(None)
    __annotations__ = (('arguments', ('union', (('dict', ('str',), ('str',), 'dict', False), ('none',), ))), )
CompletionContext.__module__ = 'mcp_types'

class CompleteRequestParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    ref = _Field()
    argument = _Field()
    context = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('ref', ('union', (('lazy', lambda: ResourceTemplateReference, 'ResourceTemplateReference'), ('lazy', lambda: PromptReference, 'PromptReference'), ))), ('argument', ('lazy', lambda: CompletionArgument, 'CompletionArgument')), ('context', ('union', (('lazy', lambda: CompletionContext, 'CompletionContext'), ('none',), ))), )
CompleteRequestParams.__module__ = 'mcp_types'

class CompleteRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('completion/complete')
    params = _Field()
    __annotations__ = (('method', ('literal', ('completion/complete', ))), ('params', ('lazy', lambda: CompleteRequestParams, 'CompleteRequestParams')), )
CompleteRequest.__module__ = 'mcp_types'

class Completion(MCPModel):
    model_config = {'populate_by_name': True}
    values = _Field()
    total = _Field(None)
    has_more = _Field(None, alias='hasMore')
    __annotations__ = (('values', ('list', ('str',), 'list', False)), ('total', ('union', (('int',), ('none',), ))), ('has_more', ('union', (('bool',), ('none',), ))), )
Completion.__module__ = 'mcp_types'

class CompleteResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    completion = _Field()
    result_type = _Field('complete', alias='resultType')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('completion', ('lazy', lambda: Completion, 'Completion')), ('result_type', ('union', (('literal', ('complete', 'input_required', )), ('str',), ))), )
CompleteResult.__module__ = 'mcp_types'

class ListRootsRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('roots/list')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('roots/list', ))), ('params', ('union', (('lazy', lambda: RequestParams, 'RequestParams'), ('none',), ))), )
ListRootsRequest.__module__ = 'mcp_types'

class RootsListChangedNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/roots/list_changed')
    params = _Field(None)
    __annotations__ = (('method', ('literal', ('notifications/roots/list_changed', ))), ('params', ('union', (('lazy', lambda: NotificationParams, 'NotificationParams'), ('none',), ))), )
RootsListChangedNotification.__module__ = 'mcp_types'

class CancelledNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    request_id = _Field(None, alias='requestId')
    reason = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('request_id', ('union', (('int',), ('str',), ('none',), ))), ('reason', ('union', (('str',), ('none',), ))), )
CancelledNotificationParams.__module__ = 'mcp_types'

class CancelledNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/cancelled')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/cancelled', ))), ('params', ('lazy', lambda: CancelledNotificationParams, 'CancelledNotificationParams')), )
CancelledNotification.__module__ = 'mcp_types'

class ElicitCompleteNotificationParams(NotificationParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    elicitation_id = _Field(alias='elicitationId')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('elicitation_id', ('str',)), )
ElicitCompleteNotificationParams.__module__ = 'mcp_types'

class ElicitCompleteNotification(Notification):
    model_config = {'populate_by_name': True}
    method = _Field('notifications/elicitation/complete')
    params = _Field()
    __annotations__ = (('method', ('literal', ('notifications/elicitation/complete', ))), ('params', ('lazy', lambda: ElicitCompleteNotificationParams, 'ElicitCompleteNotificationParams')), )
ElicitCompleteNotification.__module__ = 'mcp_types'

class ElicitRequestFormParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    mode = _Field('form')
    message = _Field()
    requested_schema = _Field(alias='requestedSchema')
    task = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('mode', ('literal', ('form', ))), ('message', ('str',)), ('requested_schema', ('dict', ('str',), ('any', 'Any'), 'dict', False)), ('task', ('union', (('lazy', lambda: TaskMetadata, 'TaskMetadata'), ('none',), ))), )
ElicitRequestFormParams.__module__ = 'mcp_types'

class ElicitRequestURLParams(RequestParams):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    mode = _Field('url')
    message = _Field()
    url = _Field()
    elicitation_id = _Field(None, alias='elicitationId')
    task = _Field(None)
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('mode', ('literal', ('url', ))), ('message', ('str',)), ('url', ('str',)), ('elicitation_id', ('union', (('str',), ('none',), ))), ('task', ('union', (('lazy', lambda: TaskMetadata, 'TaskMetadata'), ('none',), ))), )
ElicitRequestURLParams.__module__ = 'mcp_types'

class ElicitRequest(Request):
    model_config = {'populate_by_name': True}
    method = _Field('elicitation/create')
    params = _Field()
    __annotations__ = (('method', ('literal', ('elicitation/create', ))), ('params', ('union', (('lazy', lambda: ElicitRequestURLParams, 'ElicitRequestURLParams'), ('lazy', lambda: ElicitRequestFormParams, 'ElicitRequestFormParams'), ))), )
ElicitRequest.__module__ = 'mcp_types'

class ElicitationRequiredErrorData(MCPModel):
    model_config = {'populate_by_name': True}
    elicitations = _Field()
    __annotations__ = (('elicitations', ('list', ('lazy', lambda: ElicitRequestURLParams, 'ElicitRequestURLParams'), 'list', False)), )
ElicitationRequiredErrorData.__module__ = 'mcp_types'

class InputRequiredResult(Result):
    model_config = {'populate_by_name': True}
    meta = _Field(None, alias='_meta')
    result_type = _Field('input_required', alias='resultType')
    input_requests = _Field(None, alias='inputRequests')
    request_state = _Field(None, alias='requestState')
    __annotations__ = (('meta', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), ('result_type', ('literal', ('input_required', ))), ('input_requests', ('union', (('dict', ('str',), ('union', (('lazy', lambda: CreateMessageRequest, 'CreateMessageRequest'), ('lazy', lambda: ListRootsRequest, 'ListRootsRequest'), ('lazy', lambda: ElicitRequest, 'ElicitRequest'), )), 'dict', False), ('none',), ))), ('request_state', ('union', (('str',), ('none',), ))), )
InputRequiredResult.__module__ = 'mcp_types'

class ErrorData(_MCPModel):
    model_config = {'populate_by_name': True}
    code = _Field()
    message = _Field()
    data = _Field(None)
    __annotations__ = (('code', ('int',)), ('message', ('str',)), ('data', ('any', 'Any')), )
ErrorData.__module__ = 'mcp_types'

class JSONRPCError(_MCPModel):
    model_config = {'populate_by_name': True}
    jsonrpc = _Field()
    id = _Field()
    error = _Field()
    __annotations__ = (('jsonrpc', ('literal', ('2.0', ))), ('id', ('union', (('int',), ('str',), ('none',), ))), ('error', ('lazy', lambda: ErrorData, 'ErrorData')), )
JSONRPCError.__module__ = 'mcp_types'

class JSONRPCNotification(_MCPModel):
    model_config = {'populate_by_name': True}
    jsonrpc = _Field()
    method = _Field()
    params = _Field(None)
    __annotations__ = (('jsonrpc', ('literal', ('2.0', ))), ('method', ('str',)), ('params', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
JSONRPCNotification.__module__ = 'mcp_types'

class JSONRPCRequest(_MCPModel):
    model_config = {'populate_by_name': True}
    jsonrpc = _Field()
    id = _Field()
    method = _Field()
    params = _Field(None)
    __annotations__ = (('jsonrpc', ('literal', ('2.0', ))), ('id', ('union', (('int',), ('str',), ))), ('method', ('str',)), ('params', ('union', (('dict', ('str',), ('any', 'Any'), 'dict', False), ('none',), ))), )
JSONRPCRequest.__module__ = 'mcp_types'

class JSONRPCResponse(_MCPModel):
    model_config = {'populate_by_name': True}
    jsonrpc = _Field()
    id = _Field()
    result = _Field()
    __annotations__ = (('jsonrpc', ('literal', ('2.0', ))), ('id', ('union', (('int',), ('str',), ))), ('result', ('dict', ('str',), ('any', 'Any'), 'dict', False)), )
JSONRPCResponse.__module__ = 'mcp_types'

ContentBlock = (TextContent, ImageContent, AudioContent, ResourceLink, EmbeddedResource, )
ElicitRequestParams = (ElicitRequestURLParams, ElicitRequestFormParams, )
InputRequest = (CreateMessageRequest, ListRootsRequest, ElicitRequest, )
InputResponse = (CreateMessageResult, CreateMessageResultWithTools, ListRootsResult, ElicitResult, )
SamplingContent = (TextContent, ImageContent, AudioContent, )
SamplingMessageContentBlock = (TextContent, ImageContent, AudioContent, ToolUseContent, ToolResultContent, )
ClientNotification = (CancelledNotification, ProgressNotification, InitializedNotification, RootsListChangedNotification, )
ClientRequest = (PingRequest, InitializeRequest, CompleteRequest, SetLevelRequest, GetPromptRequest, ListPromptsRequest, ListResourcesRequest, ListResourceTemplatesRequest, ReadResourceRequest, SubscribeRequest, UnsubscribeRequest, CallToolRequest, ListToolsRequest, DiscoverRequest, SubscriptionsListenRequest, )
ClientResult = (EmptyResult, CreateMessageResult, CreateMessageResultWithTools, ListRootsResult, ElicitResult, )
ServerNotification = (CancelledNotification, ProgressNotification, LoggingMessageNotification, ResourceUpdatedNotification, ResourceListChangedNotification, ToolListChangedNotification, PromptListChangedNotification, ElicitCompleteNotification, SubscriptionsAcknowledgedNotification, )
ServerRequest = (PingRequest, CreateMessageRequest, ListRootsRequest, ElicitRequest, )
ServerResult = (EmptyResult, InitializeResult, DiscoverResult, CompleteResult, GetPromptResult, ListPromptsResult, ListResourcesResult, ListResourceTemplatesResult, ReadResourceResult, CallToolResult, ListToolsResult, SubscriptionsListenResult, InputRequiredResult, )
JSONRPCMessage = (JSONRPCRequest, JSONRPCNotification, JSONRPCResponse, JSONRPCError, )
