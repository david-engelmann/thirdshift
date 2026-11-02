"""One reading ladder for the three file surfaces of the tool.

The backlog file, the brief file, and the resolutions file are all
read the same way, as UTF-8 text decoded as JSON and handed to a
tail parse, and they differ only in the error type a refusal
raises, the noun the refusal messages name, and the tail parse
itself. TS-34 keeps that ladder in this one place, so the reading
discipline of the three surfaces cannot drift apart.
"""

import json
from pathlib import Path


def read_json_file(path, error_type, noun, parse):
    """Read one file surface and return what its tail parse returns.

    A file that cannot be read, a file that is not valid UTF-8, and
    a file that is not valid JSON are refused under error_type with
    a message that names the surface through noun and names the
    defect and the path.
    """
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise error_type(noun + " cannot be read, " + str(path)) from exc
    except UnicodeDecodeError as exc:
        raise error_type(noun + " is not valid UTF-8, " + str(path)) from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise error_type(noun + " is not valid JSON, " + str(path)) from exc
    return parse(data)
