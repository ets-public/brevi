# Writes the module files of Brevi's mcp copy (src/dynpy/py/mcp*.py): each of the SDK's modules a server
# imports, as the names Brevi's copy (_brevi_mcp.py, _brevi_mcp_models.py) has of it. A name not here is not
# in Brevi's mcp, and a program importing it is run by CPython's (src/caps.rs reads these files).
#
#   python3 tools/gen_mcp_shims.py
import re

PY = "src/dynpy/py/"
models = re.findall(r"^class (\w+)\(", open(PY + "_brevi_mcp_models.py").read(), re.M)
unions = re.findall(r"^(\w+) = \(", open(PY + "_brevi_mcp_models.py").read(), re.M)
models = [m for m in models if not m.startswith("_") and m != "MCPModel"]
CONSTS = ["INVALID_PARAMS", "METHOD_NOT_FOUND", "INTERNAL_ERROR", "INVALID_REQUEST", "PARSE_ERROR", "CONNECTION_CLOSED", "REQUEST_TIMEOUT",
          "HEADER_MISMATCH", "MISSING_REQUIRED_CLIENT_CAPABILITY", "UNSUPPORTED_PROTOCOL_VERSION", "URL_ELICITATION_REQUIRED",
          "LATEST_PROTOCOL_VERSION", "PROTOCOL_VERSION_META_KEY", "CLIENT_INFO_META_KEY", "CLIENT_CAPABILITIES_META_KEY",
          "SERVER_INFO_META_KEY", "LOG_LEVEL_META_KEY"]

# the SDK's mcp/__init__.py, as far as the copy has it
MCP_TOP_TYPES = """CallToolRequest ClientCapabilities ClientNotification ClientRequest ClientResult CompleteRequest CreateMessageRequest
CreateMessageResult CreateMessageResultWithTools ErrorData GetPromptRequest GetPromptResult Implementation InitializedNotification
InitializeRequest InitializeResult JSONRPCError JSONRPCRequest JSONRPCResponse ListPromptsRequest ListPromptsResult ListResourcesRequest
ListResourcesResult ListToolsResult LoggingMessageNotification Notification PingRequest ProgressNotification PromptsCapability
ReadResourceRequest ReadResourceResult Resource ResourcesCapability ResourceUpdatedNotification RootsCapability SamplingCapability
SamplingContent SamplingContextCapability SamplingMessage SamplingMessageContentBlock SamplingToolsCapability ServerCapabilities
ServerNotification ServerRequest ServerResult SetLevelRequest SubscribeRequest Tool ToolChoice ToolResultContent ToolsCapability
ToolUseContent UnsubscribeRequest""".split()

FILES = {
    "mcp.py": [("_brevi_mcp_models", [n for n in MCP_TOP_TYPES if n in models or n in unions]),
               ("_brevi_mcp", ["ServerSession", "stdio_server", "MCPDeprecationWarning", "MCPError", "InvalidUriTemplate", "UriTemplate"]),
               ("mcp.types", None)],
    "mcp_types.py": [("_brevi_mcp_models", models + unions), ("_brevi_mcp", CONSTS)],
    "mcp_server.py": [("_brevi_mcp", ["Server", "ServerRequestContext", "MCPServer", "NotificationOptions", "InitializationOptions"])],
    "mcp_server_mcpserver.py": [("_brevi_mcp", ["MCPServer", "Context", "Image", "Audio", "Message", "UserMessage", "AssistantMessage",
                                                "ResourceSecurity", "DEFAULT_RESOURCE_SECURITY"]),
                                ("_brevi_mcp_models", ["Icon"])],
    "mcp_server_context.py": [("_brevi_mcp", ["ServerRequestContext", "CallNext", "HandlerResult"])],
    "mcp_server_lowlevel.py": [("_brevi_mcp", ["Server", "NotificationOptions"])],
    "mcp_server_stdio.py": [("_brevi_mcp", ["stdio_server"])],
    "mcp_server_transport_security.py": [("_brevi_mcp", ["TransportSecuritySettings", "DEFAULT_MAX_REQUEST_BODY_SIZE"])],
    "mcp_shared_exceptions.py": [("_brevi_mcp", ["MCPError", "MCPDeprecationWarning"])],
    "mcp_shared_subscriptions.py": [("_brevi_mcp", ["LISTEN_STREAM_METHODS", "SUBSCRIPTION_ID_META_KEY", "PromptsListChanged", "ResourceUpdated",
                                                    "ResourcesListChanged", "ServerEvent", "ToolsListChanged", "event_from_wire",
                                                    "event_matches", "event_to_notification"])],
    "mcp_server_subscriptions.py": [("_brevi_mcp", ["SUBSCRIPTION_ID_META_KEY", "InMemorySubscriptionBus", "ListenHandler", "PromptsListChanged",
                                                    "ResourceUpdated", "ResourcesListChanged", "ServerEvent", "SubscriptionBus",
                                                    "ToolsListChanged"])],
    "mcp_server_mcpserver_exceptions.py": [("_brevi_mcp", ["MCPServerError", "ResourceError", "UnexpectedResourceError", "ResourceNotFoundError",
                                                           "ToolError", "UnexpectedToolError", "InvalidSignature"])],
    "mcp_server_mcpserver_prompts.py": [("_brevi_mcp", ["Prompt", "PromptManager"])],
    "mcp_server_mcpserver_prompts_base.py": [("_brevi_mcp", ["Message", "UserMessage", "AssistantMessage", "Prompt", "PromptArgument"])],
    "mcp_server_mcpserver_resources.py": [("_brevi_mcp", ["Resource", "TextResource", "BinaryResource", "FunctionResource", "ResourceTemplate",
                                                          "ResourceManager", "ResourceSecurity", "DEFAULT_RESOURCE_SECURITY"])],
    "mcp_server_mcpserver_tools.py": [("_brevi_mcp", ["Tool", "ToolManager"])],
    "mcp_server_mcpserver_utilities_types.py": [("_brevi_mcp", ["Image", "Audio"])],
}

for name, parts in FILES.items():
    mod = name[:-3].replace("_", ".").replace("mcp.types", "mcp_types").replace("mcp.server.mcpserver.utilities.types", "mcp.server.mcpserver.utilities.types").replace("transport.security", "transport_security")
    if name == "mcp_types.py":
        mod = "mcp.types (and mcp_types)"
    out = ["# %s, from Brevi's mcp copy: written by tools/gen_mcp_shims.py, do not edit." % mod]
    for src, names in parts:
        if names is None:
            out.append("import %s as types" % src)
            continue
        for i in range(0, len(names), 8):
            out.append("from %s import %s" % (src, ", ".join(names[i:i + 8])))
    out.append("")
    open(PY + name, "w").write("\n".join(out))
    print(PY + name)
