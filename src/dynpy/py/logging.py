# logging for native builds (src/dynpy/mod.rs), written for Brevi after CPython 3.13's Lib/logging/__init__.py:
# the same loggers, levels, handlers, formatters, filters, LoggerAdapter, basicConfig and default formats, and
# the same text on stderr. What it has not got a program using it runs in CPython (caps.rs LOGGING_LACKS):
# logging.handlers, logging.config, FileHandler and basicConfig(filename=), the caller's file, line and
# function in records (%(lineno)d, %(funcName)s and the rest), process and thread ids, stack_info and
# captureWarnings. Tracebacks in records are written as Brevi writes an uncaught exception's.
import sys
import time
import re
import datetime as _dt
import tzlocal as _tzl

__all__ = ['BASIC_FORMAT', 'BufferingFormatter', 'CRITICAL', 'DEBUG', 'ERROR', 'FATAL', 'Filter', 'Formatter',
           'Handler', 'INFO', 'LogRecord', 'Logger', 'LoggerAdapter', 'NOTSET', 'NullHandler', 'StreamHandler',
           'WARN', 'WARNING', 'addLevelName', 'basicConfig', 'critical', 'debug', 'disable', 'error',
           'exception', 'fatal', 'getLevelName', 'getLogger', 'getLoggerClass', 'info', 'log', 'makeLogRecord',
           'setLoggerClass', 'shutdown', 'warn', 'warning', 'getLogRecordFactory', 'setLogRecordFactory',
           'lastResort', 'raiseExceptions', 'getLevelNamesMapping', 'getHandlerByName', 'getHandlerNames']

raiseExceptions = True
logThreads = True
logMultiprocessing = True
logProcesses = True
logAsyncioTasks = True

_startTime = time.time_ns()

CRITICAL = 50
FATAL = CRITICAL
ERROR = 40
WARNING = 30
WARN = WARNING
INFO = 20
DEBUG = 10
NOTSET = 0

_levelToName = {CRITICAL: 'CRITICAL', ERROR: 'ERROR', WARNING: 'WARNING', INFO: 'INFO', DEBUG: 'DEBUG', NOTSET: 'NOTSET'}
_nameToLevel = {'CRITICAL': CRITICAL, 'FATAL': FATAL, 'ERROR': ERROR, 'WARN': WARNING, 'WARNING': WARNING,
                'INFO': INFO, 'DEBUG': DEBUG, 'NOTSET': NOTSET}


def getLevelNamesMapping():
    return _nameToLevel.copy()


def getLevelName(level):
    result = _levelToName.get(level)
    if result is not None:
        return result
    result = _nameToLevel.get(level)
    if result is not None:
        return result
    return "Level %s" % level


def addLevelName(level, levelName):
    _levelToName[level] = levelName
    _nameToLevel[levelName] = level


def _checkLevel(level):
    if isinstance(level, bool) or not isinstance(level, (int, str)):
        raise TypeError("Level not an integer or a valid string: %r" % (level,))
    if isinstance(level, int):
        return level
    if level not in _nameToLevel:
        raise ValueError("Unknown level: %r" % level)
    return _nameToLevel[level]


_localzone = None


def _local(t):
    global _localzone
    if _localzone is None:
        _localzone = _tzl.get_localzone()
    return _dt.datetime.fromtimestamp(t, tz=_localzone)


def _exc_info_of(exc_info):
    if isinstance(exc_info, BaseException):
        return (type(exc_info), exc_info, None)
    if isinstance(exc_info, tuple):
        return exc_info
    return sys.exc_info()


class LogRecord(object):
    def __init__(self, name, level, pathname, lineno, msg, args, exc_info, func=None, sinfo=None, **kwargs):
        ct = time.time_ns()
        self.name = name
        self.msg = msg
        if args and len(args) == 1 and isinstance(args[0], dict) and args[0]:
            args = args[0]
        self.args = args
        self.levelname = getLevelName(level)
        self.levelno = level
        self.pathname = pathname
        self.filename = pathname
        self.module = "Unknown module"
        self.exc_info = exc_info
        self.exc_text = None
        self.stack_info = sinfo
        self.lineno = lineno
        self.funcName = func
        self.created = ct / 1e9
        self.msecs = (ct % 1000000000) // 1000000 + 0.0
        self.relativeCreated = (ct - _startTime) / 1e6
        self.thread = None
        self.threadName = None
        self.processName = None
        self.process = None
        self.taskName = None

    def __repr__(self):
        return '<LogRecord: %s, %s, %s, %s, "%s">' % (self.name, self.levelno, self.pathname, self.lineno, self.msg)

    def getMessage(self):
        msg = str(self.msg)
        if self.args:
            msg = msg % self.args
        return msg


_logRecordFactory = LogRecord


def setLogRecordFactory(factory):
    global _logRecordFactory
    _logRecordFactory = factory


def getLogRecordFactory():
    return _logRecordFactory


def makeLogRecord(dict):
    rv = _logRecordFactory(None, None, "", 0, "", (), None, None)
    for k in dict:
        setattr(rv, k, dict[k])
    return rv


_PCT = re.compile(r'%\(([^)]+)\)([#0+ -]*\*?(?:\d+|\*)?(?:\.(?:\d+|\*))?[diouxefgcrsaEFGX%])', re.I)
_PCT_CHECK = re.compile(r'%\(\w+\)[#0+ -]*(\*|\d+)?(\.(\*|\d+))?[diouxefgcrsa%]', re.I)
_STR_FIELD = re.compile(r'\{(\w*)')
_TMPL = re.compile(r'\$(?:(\$)|(\w+)|\{(\w+)\})')


class PercentStyle(object):
    default_format = '%(message)s'
    asctime_format = '%(asctime)s'
    asctime_search = '%(asctime)'

    def __init__(self, fmt, *, defaults=None):
        self._fmt = fmt or self.default_format
        self._defaults = defaults

    def usesTime(self):
        return self._fmt.find(self.asctime_search) >= 0

    def validate(self):
        if not _PCT_CHECK.search(self._fmt):
            raise ValueError("Invalid format '%s' for '%s' style" % (self._fmt, self.default_format[0]))

    def _values(self, record):
        values = {}
        if self._defaults:
            for k in self._defaults:
                values[k] = self._defaults[k]
        d = record.__dict__
        for k in d:
            values[k] = d[k]
        return values

    def _format(self, record):
        values = self._values(record)
        out = []
        pos = 0
        for m in _PCT.finditer(self._fmt):
            out.append(self._fmt[pos:m.start()].replace('%%', '%'))
            out.append(('%' + m.group(2)) % (values[m.group(1)],))
            pos = m.end()
        out.append(self._fmt[pos:].replace('%%', '%'))
        return ''.join(out)

    def format(self, record):
        try:
            return self._format(record)
        except KeyError as e:
            raise ValueError('Formatting field not found in record: %s' % e)


class StrFormatStyle(PercentStyle):
    default_format = '{message}'
    asctime_format = '{asctime}'
    asctime_search = '{asctime'

    def _format(self, record):
        return self._fmt.format(**self._values(record))

    def validate(self):
        if not _STR_FIELD.search(self._fmt):
            raise ValueError('invalid format: no fields')


class StringTemplateStyle(PercentStyle):
    default_format = '${message}'
    asctime_format = '${asctime}'
    asctime_search = '${asctime}'

    def usesTime(self):
        fmt = self._fmt
        return fmt.find('$asctime') >= 0 or fmt.find(self.asctime_search) >= 0

    def validate(self):
        if not _TMPL.search(self._fmt):
            raise ValueError('invalid format: no fields')

    def _format(self, record):
        values = self._values(record)

        def sub(m):
            if m.group(1):
                return '$'
            return str(values[m.group(2) or m.group(3)])
        return _TMPL.sub(sub, self._fmt)


BASIC_FORMAT = "%(levelname)s:%(name)s:%(message)s"

_STYLES = {
    '%': (PercentStyle, BASIC_FORMAT),
    '{': (StrFormatStyle, '{levelname}:{name}:{message}'),
    '$': (StringTemplateStyle, '${levelname}:${name}:${message}'),
}


class Formatter(object):
    converter = None
    default_time_format = '%Y-%m-%d %H:%M:%S'
    default_msec_format = '%s,%03d'

    def __init__(self, fmt=None, datefmt=None, style='%', validate=True, *, defaults=None):
        if style not in _STYLES:
            raise ValueError('Style must be one of: %s' % ','.join(_STYLES.keys()))
        self._style = _STYLES[style][0](fmt, defaults=defaults)
        if validate:
            self._style.validate()
        self._fmt = self._style._fmt
        self.datefmt = datefmt

    def formatTime(self, record, datefmt=None):
        if self.converter is not None:
            ct = self.converter(record.created)
            if datefmt:
                s = time.strftime(datefmt, ct)
            else:
                s = time.strftime(self.default_time_format, ct)
                if self.default_msec_format:
                    s = self.default_msec_format % (s, record.msecs)
            return s
        ct = _local(record.created)
        if datefmt:
            return ct.strftime(datefmt)
        s = ct.strftime(self.default_time_format)
        if self.default_msec_format:
            s = self.default_msec_format % (s, record.msecs)
        return s

    def formatException(self, ei):
        # Each exception in the chain (__cause__, __context__) as Brevi writes an uncaught one: the header
        # and the last line, without CPython's frames.
        chain = []
        x = ei[1]
        while x is not None and len(chain) < 100:
            chain.append(x)
            if x.__cause__ is not None:
                x = x.__cause__
            elif getattr(x, "__suppress_context__", False):
                x = None
            else:
                x = getattr(x, "__context__", None)
        chain.reverse()
        lines = []
        for i, x in enumerate(chain):
            lines.append("Traceback (most recent call last):")
            name = type(x).__name__
            mod = getattr(type(x), "__module__", "builtins")
            if mod not in ("builtins", "__main__", None):
                name = mod + "." + name
            text = str(x)
            lines.append(name + ": " + text if text else name)
            if i < len(chain) - 1:
                lines.append("")
                if chain[i + 1].__cause__ is x:
                    lines.append("The above exception was the direct cause of the following exception:")
                else:
                    lines.append("During handling of the above exception, another exception occurred:")
                lines.append("")
        return "\n".join(lines)

    def usesTime(self):
        return self._style.usesTime()

    def formatMessage(self, record):
        return self._style.format(record)

    def formatStack(self, stack_info):
        return stack_info

    def format(self, record):
        record.message = record.getMessage()
        if self.usesTime():
            record.asctime = self.formatTime(record, self.datefmt)
        s = self.formatMessage(record)
        if record.exc_info:
            if not record.exc_text:
                record.exc_text = self.formatException(record.exc_info)
        if record.exc_text:
            if s[-1:] != "\n":
                s = s + "\n"
            s = s + record.exc_text
        if record.stack_info:
            if s[-1:] != "\n":
                s = s + "\n"
            s = s + self.formatStack(record.stack_info)
        return s


_defaultFormatter = Formatter()


class BufferingFormatter(object):
    def __init__(self, linefmt=None):
        if linefmt:
            self.linefmt = linefmt
        else:
            self.linefmt = _defaultFormatter

    def formatHeader(self, records):
        return ""

    def formatFooter(self, records):
        return ""

    def format(self, records):
        rv = ""
        if len(records) > 0:
            rv = rv + self.formatHeader(records)
            for record in records:
                rv = rv + self.linefmt.format(record)
            rv = rv + self.formatFooter(records)
        return rv


class Filter(object):
    def __init__(self, name=''):
        self.name = name
        self.nlen = len(name)

    def filter(self, record):
        if self.nlen == 0:
            return True
        elif self.name == record.name:
            return True
        elif record.name.find(self.name, 0, self.nlen) != 0:
            return False
        return (record.name[self.nlen] == ".")


class Filterer(object):
    def __init__(self):
        self.filters = []

    def addFilter(self, filter):
        if not (filter in self.filters):
            self.filters.append(filter)

    def removeFilter(self, filter):
        if filter in self.filters:
            self.filters.remove(filter)

    def filter(self, record):
        for f in self.filters:
            if hasattr(f, 'filter'):
                result = f.filter(record)
            else:
                result = f(record)
            if not result:
                return False
            if isinstance(result, LogRecord):
                record = result
        return record


_handlers = {}
_handlerList = []


def getHandlerByName(name):
    return _handlers.get(name)


def getHandlerNames():
    return frozenset(_handlers)


class Handler(Filterer):
    def __init__(self, level=NOTSET):
        Filterer.__init__(self)
        self._name = None
        self.level = _checkLevel(level)
        self.formatter = None
        self._closed = False
        _handlerList.append(self)

    def get_name(self):
        return self._name

    def set_name(self, name):
        if self._name in _handlers:
            del _handlers[self._name]
        self._name = name
        if name:
            _handlers[name] = self

    @property
    def name(self):
        return self._name

    @name.setter
    def name(self, name):
        self.set_name(name)

    def createLock(self):
        pass

    def acquire(self):
        pass

    def release(self):
        pass

    def setLevel(self, level):
        self.level = _checkLevel(level)

    def format(self, record):
        if self.formatter:
            fmt = self.formatter
        else:
            fmt = _defaultFormatter
        return fmt.format(record)

    def emit(self, record):
        raise NotImplementedError('emit must be implemented by Handler subclasses')

    def handle(self, record):
        rv = self.filter(record)
        if isinstance(rv, LogRecord):
            record = rv
        if rv:
            self.emit(record)
        return rv

    def setFormatter(self, fmt):
        self.formatter = fmt

    def flush(self):
        pass

    def close(self):
        self._closed = True
        if self._name and self._name in _handlers:
            del _handlers[self._name]

    def handleError(self, record):
        if raiseExceptions and sys.stderr:
            e = sys.exception()
            sys.stderr.write('--- Logging error ---\n')
            if e is not None:
                sys.stderr.write(_defaultFormatter.formatException((type(e), e, None)) + '\n')
            try:
                sys.stderr.write('Message: %r\nArguments: %s\n' % (record.msg, record.args))
            except Exception:
                sys.stderr.write('Unable to print the message and arguments - possible formatting error.\nUse the traceback above to help find the error.\n')

    def __repr__(self):
        level = getLevelName(self.level)
        return '<%s (%s)>' % (self.__class__.__name__, level)


class StreamHandler(Handler):
    terminator = '\n'

    def __init__(self, stream=None):
        Handler.__init__(self)
        if stream is None:
            stream = sys.stderr
        self.stream = stream

    def flush(self):
        if self.stream and hasattr(self.stream, "flush"):
            self.stream.flush()

    def emit(self, record):
        try:
            msg = self.format(record)
            stream = self.stream
            stream.write(msg + self.terminator)
            self.flush()
        except RecursionError:
            raise
        except Exception:
            self.handleError(record)

    def setStream(self, stream):
        if stream is self.stream:
            result = None
        else:
            result = self.stream
            self.flush()
            self.stream = stream
        return result

    def __repr__(self):
        level = getLevelName(self.level)
        name = ''
        if self.stream is sys.stderr:
            name = '<stderr>'
        elif self.stream is sys.stdout:
            name = '<stdout>'
        if name:
            name += ' '
        return '<%s %s(%s)>' % (self.__class__.__name__, name, level)


class _StderrHandler(StreamHandler):
    def __init__(self, level=NOTSET):
        Handler.__init__(self, level)
        self.stream = None

    def emit(self, record):
        self.stream = sys.stderr
        StreamHandler.emit(self, record)


_defaultLastResort = _StderrHandler(WARNING)
lastResort = _defaultLastResort


class NullHandler(Handler):
    def handle(self, record):
        pass

    def emit(self, record):
        pass


class Manager(object):
    def __init__(self, rootnode):
        self.root = rootnode
        self.disable = 0
        self.emittedNoHandlerWarning = False
        self.loggerDict = {}
        self.loggerClass = None
        self.logRecordFactory = None

    def getLogger(self, name):
        if not isinstance(name, str):
            raise TypeError('A logger name must be a string')
        rv = self.loggerDict.get(name)
        if rv is None:
            rv = (self.loggerClass or _loggerClass)(name)
            rv.manager = self
            self.loggerDict[name] = rv
            self._fixup(rv)
        return rv

    def setLoggerClass(self, klass):
        self.loggerClass = klass

    def setLogRecordFactory(self, factory):
        self.logRecordFactory = factory

    def _fixup(self, alogger):
        name = alogger.name
        i = name.rfind(".")
        rv = None
        while (i > 0) and not rv:
            rv = self.loggerDict.get(name[:i])
            i = name.rfind(".", 0, i - 1)
        if not rv:
            rv = self.root
        alogger.parent = rv
        # loggers made before this one whose parent is now this one (CPython keeps placeholders for them)
        pre = name + "."
        for k in self.loggerDict:
            c = self.loggerDict[k]
            if k.startswith(pre) and (c.parent is self.root or len(c.parent.name) < len(name)):
                c.parent = alogger

    def _clear_cache(self):
        pass


class Logger(Filterer):
    def __init__(self, name, level=NOTSET):
        Filterer.__init__(self)
        self.name = name
        self.level = _checkLevel(level)
        self.parent = None
        self.propagate = True
        self.handlers = []
        self.disabled = False

    def setLevel(self, level):
        self.level = _checkLevel(level)

    def debug(self, msg, *args, **kwargs):
        if self.isEnabledFor(DEBUG):
            self._log(DEBUG, msg, args, **kwargs)

    def info(self, msg, *args, **kwargs):
        if self.isEnabledFor(INFO):
            self._log(INFO, msg, args, **kwargs)

    def warning(self, msg, *args, **kwargs):
        if self.isEnabledFor(WARNING):
            self._log(WARNING, msg, args, **kwargs)

    def warn(self, msg, *args, **kwargs):
        self.warning(msg, *args, **kwargs)

    def error(self, msg, *args, **kwargs):
        if self.isEnabledFor(ERROR):
            self._log(ERROR, msg, args, **kwargs)

    def exception(self, msg, *args, exc_info=True, **kwargs):
        self.error(msg, *args, exc_info=exc_info, **kwargs)

    def critical(self, msg, *args, **kwargs):
        if self.isEnabledFor(CRITICAL):
            self._log(CRITICAL, msg, args, **kwargs)

    def fatal(self, msg, *args, **kwargs):
        self.critical(msg, *args, **kwargs)

    def log(self, level, msg, *args, **kwargs):
        if not isinstance(level, int):
            if raiseExceptions:
                raise TypeError("level must be an integer")
            else:
                return
        if self.isEnabledFor(level):
            self._log(level, msg, args, **kwargs)

    def findCaller(self, stack_info=False, stacklevel=1):
        return "(unknown file)", 0, "(unknown function)", None

    def makeRecord(self, name, level, fn, lno, msg, args, exc_info, func=None, extra=None, sinfo=None):
        rv = _logRecordFactory(name, level, fn, lno, msg, args, exc_info, func, sinfo)
        if extra is not None:
            for key in extra:
                if (key in ["message", "asctime"]) or (key in rv.__dict__):
                    raise KeyError("Attempt to overwrite %r in LogRecord" % key)
                setattr(rv, key, extra[key])
        return rv

    def _log(self, level, msg, args, exc_info=None, extra=None, stack_info=False, stacklevel=1):
        fn, lno, func, sinfo = self.findCaller(stack_info, stacklevel)
        if exc_info:
            exc_info = _exc_info_of(exc_info)
        record = self.makeRecord(self.name, level, fn, lno, msg, args, exc_info, func, extra, sinfo)
        self.handle(record)

    def handle(self, record):
        if self.disabled:
            return
        maybe_record = self.filter(record)
        if not maybe_record:
            return
        if isinstance(maybe_record, LogRecord):
            record = maybe_record
        self.callHandlers(record)

    def addHandler(self, hdlr):
        if not (hdlr in self.handlers):
            self.handlers.append(hdlr)

    def removeHandler(self, hdlr):
        if hdlr in self.handlers:
            self.handlers.remove(hdlr)

    def hasHandlers(self):
        c = self
        rv = False
        while c:
            if c.handlers:
                rv = True
                break
            if not c.propagate:
                break
            else:
                c = c.parent
        return rv

    def callHandlers(self, record):
        c = self
        found = 0
        while c:
            for hdlr in c.handlers:
                found = found + 1
                if record.levelno >= hdlr.level:
                    hdlr.handle(record)
            if not c.propagate:
                c = None
            else:
                c = c.parent
        if found == 0:
            if lastResort:
                if record.levelno >= lastResort.level:
                    lastResort.handle(record)
            elif raiseExceptions and not self.manager.emittedNoHandlerWarning:
                sys.stderr.write("No handlers could be found for logger \"%s\"\n" % self.name)
                self.manager.emittedNoHandlerWarning = True

    def getEffectiveLevel(self):
        logger = self
        while logger:
            if logger.level:
                return logger.level
            logger = logger.parent
        return NOTSET

    def isEnabledFor(self, level):
        if self.disabled:
            return False
        if self.manager.disable >= level:
            return False
        return level >= self.getEffectiveLevel()

    def getChild(self, suffix):
        if self.root is not self:
            suffix = '.'.join((self.name, suffix))
        return self.manager.getLogger(suffix)

    def getChildren(self):
        if self.root is self:
            return set(self.manager.loggerDict[k] for k in self.manager.loggerDict if '.' not in k)
        pre = self.name + '.'
        n = len(pre)
        return set(self.manager.loggerDict[k] for k in self.manager.loggerDict
                   if k.startswith(pre) and '.' not in k[n:])

    def __repr__(self):
        level = getLevelName(self.getEffectiveLevel())
        return '<%s %s (%s)>' % (self.__class__.__name__, self.name, level)


class RootLogger(Logger):
    def __init__(self, level):
        Logger.__init__(self, "root", level)

    def __repr__(self):
        return '<RootLogger root (%s)>' % getLevelName(self.getEffectiveLevel())


_loggerClass = Logger


class LoggerAdapter(object):
    def __init__(self, logger, extra=None, merge_extra=False):
        self.logger = logger
        self.extra = extra
        self.merge_extra = merge_extra

    def process(self, msg, kwargs):
        if self.merge_extra and "extra" in kwargs:
            e = {}
            if self.extra:
                for k in self.extra:
                    e[k] = self.extra[k]
            for k in kwargs["extra"]:
                e[k] = kwargs["extra"][k]
            kwargs["extra"] = e
        else:
            kwargs["extra"] = self.extra
        return msg, kwargs

    def debug(self, msg, *args, **kwargs):
        self.log(DEBUG, msg, *args, **kwargs)

    def info(self, msg, *args, **kwargs):
        self.log(INFO, msg, *args, **kwargs)

    def warning(self, msg, *args, **kwargs):
        self.log(WARNING, msg, *args, **kwargs)

    def warn(self, msg, *args, **kwargs):
        self.warning(msg, *args, **kwargs)

    def error(self, msg, *args, **kwargs):
        self.log(ERROR, msg, *args, **kwargs)

    def exception(self, msg, *args, exc_info=True, **kwargs):
        self.log(ERROR, msg, *args, exc_info=exc_info, **kwargs)

    def critical(self, msg, *args, **kwargs):
        self.log(CRITICAL, msg, *args, **kwargs)

    def log(self, level, msg, *args, **kwargs):
        if self.isEnabledFor(level):
            msg, kwargs = self.process(msg, kwargs)
            self.logger.log(level, msg, *args, **kwargs)

    def isEnabledFor(self, level):
        return self.logger.isEnabledFor(level)

    def setLevel(self, level):
        self.logger.setLevel(level)

    def getEffectiveLevel(self):
        return self.logger.getEffectiveLevel()

    def hasHandlers(self):
        return self.logger.hasHandlers()

    @property
    def manager(self):
        return self.logger.manager

    @property
    def name(self):
        return self.logger.name

    def __repr__(self):
        logger = self.logger
        level = getLevelName(logger.getEffectiveLevel())
        return '<%s %s (%s)>' % (self.__class__.__name__, logger.name, level)


root = RootLogger(WARNING)
Logger.root = root
Logger.manager = Manager(Logger.root)
root.manager = Logger.manager


def basicConfig(**kwargs):
    force = kwargs.pop('force', False)
    if force:
        for h in root.handlers[:]:
            root.removeHandler(h)
            h.close()
    if len(root.handlers) == 0:
        handlers = kwargs.pop("handlers", None)
        if handlers is None:
            if "stream" in kwargs and "filename" in kwargs:
                raise ValueError("'stream' and 'filename' should not be specified together")
        else:
            if "stream" in kwargs or "filename" in kwargs:
                raise ValueError("'stream' or 'filename' should not be specified together with 'handlers'")
        if handlers is None:
            stream = kwargs.pop("stream", None)
            h = StreamHandler(stream)
            handlers = [h]
        dfs = kwargs.pop("datefmt", None)
        style = kwargs.pop("style", '%')
        if style not in _STYLES:
            raise ValueError('Style must be one of: %s' % ','.join(_STYLES.keys()))
        fs = kwargs.pop("format", _STYLES[style][1])
        fmt = Formatter(fs, dfs, style)
        for h in handlers:
            if h.formatter is None:
                h.setFormatter(fmt)
            root.addHandler(h)
        level = kwargs.pop("level", None)
        if level is not None:
            root.setLevel(level)
        if kwargs:
            keys = ', '.join(kwargs.keys())
            raise ValueError('Unrecognised argument(s): %s' % keys)


def getLogger(name=None):
    if not name or isinstance(name, str) and name == root.name:
        return root
    return Logger.manager.getLogger(name)


def setLoggerClass(klass):
    global _loggerClass
    if klass != Logger:
        if not issubclass(klass, Logger):
            raise TypeError("logger not derived from logging.Logger: " + klass.__name__)
    _loggerClass = klass


def getLoggerClass():
    return _loggerClass


def critical(msg, *args, **kwargs):
    if len(root.handlers) == 0:
        basicConfig()
    root.critical(msg, *args, **kwargs)


def fatal(msg, *args, **kwargs):
    critical(msg, *args, **kwargs)


def error(msg, *args, **kwargs):
    if len(root.handlers) == 0:
        basicConfig()
    root.error(msg, *args, **kwargs)


def exception(msg, *args, exc_info=True, **kwargs):
    error(msg, *args, exc_info=exc_info, **kwargs)


def warning(msg, *args, **kwargs):
    if len(root.handlers) == 0:
        basicConfig()
    root.warning(msg, *args, **kwargs)


def warn(msg, *args, **kwargs):
    warning(msg, *args, **kwargs)


def info(msg, *args, **kwargs):
    if len(root.handlers) == 0:
        basicConfig()
    root.info(msg, *args, **kwargs)


def debug(msg, *args, **kwargs):
    if len(root.handlers) == 0:
        basicConfig()
    root.debug(msg, *args, **kwargs)


def log(level, msg, *args, **kwargs):
    if len(root.handlers) == 0:
        basicConfig()
    root.log(level, msg, *args, **kwargs)


def disable(level=CRITICAL):
    root.manager.disable = level


def shutdown(handlerList=None):
    for h in reversed(_handlerList if handlerList is None else handlerList):
        try:
            h.flush()
            h.close()
        except (OSError, ValueError):
            pass
