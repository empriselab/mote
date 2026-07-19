#!/usr/bin/env python3
"""Generates mote_link/_generated.py from mote-ffi's JSON schemas.

Mirrors mote-configuration/scripts/generate-types.mjs, which generates
TypeScript types from the same schemas — this is the Python equivalent, plus
a generic wire (de)serializer driven by the same class-name-is-the-wire-tag
convention used to build the message unions below. That's what actually
prevents drift: there's no hand-written per-variant branch to forget to
update when a Rust message variant is added, removed, or renamed.

Every JSON Schema `oneOf` in mote-api's messages is one of two shapes:

  - Pure enum: every branch is `{"type": "string", "const": "Foo"}`. Becomes
    a Python `Enum`.
  - Tagged union: a mix of those unit (const-string) branches and data
    branches shaped `{"type": "object", "properties": {"Foo": <schema>}}`
    (serde's adjacently-tagged representation for enum variants). Each unit
    branch becomes an empty dataclass; each data branch becomes either the
    referenced struct's own dataclass directly (when the branch's payload is
    a struct — Rust's convention here is that the variant and its payload
    struct share a name, e.g. `SetUid(SetUid)`) or, when the payload isn't a
    struct (e.g. `Scan(Vec<Point>)`), a single-field "newtype" dataclass
    whose one field is always named `value` and whose wire form is the bare
    payload, not `{"value": ...}`.

Both message roots (`HostToMoteMessage`, `MoteToHostMessage`) are tagged
unions in this same sense, so the same code generates them.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
SCHEMAS_DIR = SCRIPT_DIR.parent / "schemas"
OUT_FILE = SCRIPT_DIR.parent / "mote_link" / "_generated.py"

PRIMITIVE_TYPES = {
    "string": "str",
    "integer": "int",
    "number": "float",
    "boolean": "bool",
}

BANNER = '''\
"""Generated from mote-ffi's JSON schemas — do not edit by hand.

Regenerate with `task ffi:generate-python-types` (see
mote-ffi/scripts/generate_python_types.py) after changing mote-api's message
types.
"""

import dataclasses
import json
from dataclasses import dataclass
from enum import Enum
from typing import Any, List, Optional, Union, get_args, get_origin

'''

RUNTIME = '''

def _encode(value: Any, typ: Any = None) -> Any:
    """Encodes `value` per its declared type `typ`.

    `typ` is what disambiguates a plain struct field (encodes as its own flat
    dict of fields) from a union-typed field (encodes tag-wrapped, since more
    than one variant shape is possible). `typ=None` means "top-level value",
    e.g. what `to_wire_json` receives -- always tag-wrapped.
    """
    if value is None:
        return None
    if isinstance(value, Enum):
        return value.value
    if typ is not None and get_origin(typ) is Union:
        args = [a for a in get_args(typ) if a is not type(None)]
        if len(args) == 1:
            return _encode(value, args[0])
        return _encode_tagged(value)
    if isinstance(value, list):
        item_t = get_args(typ)[0] if typ is not None and get_origin(typ) is list else None
        return [_encode(v, item_t) for v in value]
    if dataclasses.is_dataclass(value):
        if typ is None:
            return _encode_tagged(value)
        return _encode_struct(value)
    return value


def _encode_struct(value: Any) -> dict:
    return {f.name: _encode(getattr(value, f.name), f.type) for f in dataclasses.fields(value)}


def _encode_tagged(msg: Any) -> Any:
    tag = type(msg).__name__
    fields = dataclasses.fields(msg)
    if not fields:
        return tag
    if type(msg) in _NEWTYPE_CLASSES:
        (f,) = fields
        return {tag: _encode(getattr(msg, f.name), f.type)}
    return {tag: _encode_struct(msg)}


def to_wire_json(msg: Any) -> str:
    """Serializes a message (or any generated value) to mote-api's wire JSON."""
    return json.dumps(_encode(msg))


def _decode(value: Any, typ: Any) -> Any:
    if typ is type(None) or value is None:
        return None
    origin = get_origin(typ)
    if origin is Union:
        args = [a for a in get_args(typ) if a is not type(None)]
        if len(args) == 1:
            return _decode(value, args[0])
        return _decode_tagged(value, tuple(args))
    if origin is list:
        (item_t,) = get_args(typ)
        return [_decode(v, item_t) for v in value]
    if isinstance(typ, type) and issubclass(typ, Enum):
        return typ(value)
    if dataclasses.is_dataclass(typ):
        # A struct referenced directly by a field (not as a union member)
        # decodes its properties straight from the object -- no tag wrapper.
        fields = dataclasses.fields(typ)
        return typ(**{f.name: _decode(value.get(f.name), f.type) for f in fields})
    return value


def _decode_tagged(value: Any, classes: tuple) -> Any:
    tags = {c.__name__: c for c in classes}
    if isinstance(value, str):
        cls = tags.get(value)
        if cls is None or dataclasses.fields(cls):
            raise ValueError(f"unexpected tag {value!r}, expected one of {sorted(tags)}")
        return cls()
    if isinstance(value, dict) and len(value) == 1:
        ((tag, inner),) = value.items()
        cls = tags.get(tag)
        if cls is None:
            raise ValueError(f"unknown tag {tag!r}, expected one of {sorted(tags)}")
        fields = dataclasses.fields(cls)
        if not fields:
            raise ValueError(f"tag {tag!r} takes no data but got {inner!r}")
        if cls in _NEWTYPE_CLASSES:
            (f,) = fields
            return cls(**{f.name: _decode(inner, f.type)})
        return _decode(inner, cls)
    raise ValueError(f"unexpected wire value {value!r} for tags {sorted(tags)}")


def from_wire_json(json_str: str, union: Any) -> Any:
    """Deserializes mote-api wire JSON into one of `union`'s member types."""
    return _decode(json.loads(json_str), union)
'''


class GeneratorError(Exception):
    pass


def _enum_member_name(const: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", const).upper()


class Generator:
    def __init__(self, defs: dict[str, dict]):
        self.defs = defs
        self.emitted: dict[str, str] = {}
        self.newtype_classes: list[str] = []
        self._pending: set[str] = set()

    def resolve_ref(self, ref: str) -> str:
        return self.resolve_def(ref.rsplit("/", 1)[-1])

    def resolve_def(self, name: str) -> str:
        if name in self.emitted:
            return name
        if name in self._pending:
            raise GeneratorError(
                f"cyclic schema reference involving {name!r} isn't supported"
            )
        schema = self.defs.get(name)
        if schema is None:
            raise GeneratorError(f"unknown $ref target {name!r}")
        self._pending.add(name)
        self._emit_node(name, schema)
        self._pending.discard(name)
        return name

    def resolve_type(self, schema: dict) -> str:
        """Returns a Python type expression for an inline (unnamed) schema node."""
        if "$ref" in schema:
            return self.resolve_ref(schema["$ref"])
        if "anyOf" in schema:
            branches = schema["anyOf"]
            non_null = [b for b in branches if b.get("type") != "null"]
            if len(non_null) != 1:
                raise GeneratorError(f"unsupported anyOf shape: {schema!r}")
            inner = self.resolve_type(non_null[0])
            nullable = len(non_null) != len(branches)
            return f"Optional[{inner}]" if nullable else inner
        if "oneOf" in schema:
            raise GeneratorError(
                f"anonymous (non-$def) oneOf schemas aren't supported: {schema!r}"
            )
        t = schema.get("type")
        if isinstance(t, list):
            non_null = [x for x in t if x != "null"]
            if len(non_null) != 1:
                raise GeneratorError(f"unsupported multi-type schema: {schema!r}")
            inner = PRIMITIVE_TYPES[non_null[0]]
            return f"Optional[{inner}]" if "null" in t else inner
        if t == "array":
            return f"List[{self.resolve_type(schema['items'])}]"
        if t in PRIMITIVE_TYPES:
            return PRIMITIVE_TYPES[t]
        raise GeneratorError(f"unsupported schema node: {schema!r}")

    def is_struct_like(self, schema: dict) -> bool:
        """After resolving any $ref, is this a plain object-with-properties?"""
        node = schema
        if "$ref" in node:
            node = self.defs[node["$ref"].rsplit("/", 1)[-1]]
        return (
            node.get("type") == "object"
            and "properties" in node
            and "oneOf" not in node
        )

    def _emit_node(self, name: str, schema: dict) -> None:
        if "oneOf" in schema:
            self._emit_union(name, schema)
        elif schema.get("type") == "object":
            self._emit_struct(name, schema)
        else:
            raise GeneratorError(f"don't know how to generate {name!r}: {schema!r}")

    def _emit_struct(self, name: str, schema: dict) -> None:
        props: dict = schema.get("properties", {})
        required = set(schema.get("required", []))
        ordered = [f for f in schema.get("required", []) if f in props]
        ordered += [f for f in props if f not in required]

        lines = ["@dataclass", f"class {name}:"]
        doc = schema.get("description")
        if doc:
            lines.append(f'    """{doc}"""')
        if not ordered:
            lines.append("    pass")
        for fname in ordered:
            ftype = self.resolve_type(props[fname])
            if fname not in required and not ftype.startswith("Optional["):
                ftype = f"Optional[{ftype}]"
            lines.append(f"    {fname}: {ftype}")
        self.emitted[name] = "\n".join(lines)

    def _emit_union(self, name: str, schema: dict) -> str:
        branches = schema["oneOf"]
        is_pure_enum = all(b.get("type") == "string" and "const" in b for b in branches)

        if is_pure_enum:
            lines = [f"class {name}(str, Enum):"]
            doc = schema.get("description")
            if doc:
                lines.append(f'    """{doc}"""')
            for b in branches:
                const = b["const"]
                lines.append(f"    {_enum_member_name(const)} = {const!r}")
            self.emitted[name] = "\n".join(lines)
            return name

        member_names = []
        for b in branches:
            if b.get("type") == "string" and "const" in b:
                tag = b["const"]
                self._emit_unit_variant(tag, b.get("description"))
            else:
                properties = b.get("properties")
                if not properties or len(properties) != 1:
                    raise GeneratorError(f"unsupported oneOf branch shape: {b!r}")
                ((tag, inner_schema),) = properties.items()
                self._emit_data_variant(tag, inner_schema, b.get("description"))
            member_names.append(tag)

        self.emitted[name] = f"{name} = Union[{', '.join(member_names)}]"
        return name

    def _emit_unit_variant(self, tag: str, doc: str | None) -> None:
        if tag in self.emitted:
            return  # Shared across both message directions (e.g. Ping/Pong).
        lines = ["@dataclass", f"class {tag}:"]
        if doc:
            lines.append(f'    """{doc}"""')
        lines.append("    pass")
        self.emitted[tag] = "\n".join(lines)

    def _emit_data_variant(self, tag: str, inner_schema: dict, doc: str | None) -> None:
        if tag in self.emitted:
            return
        if self.is_struct_like(inner_schema):
            if "$ref" in inner_schema:
                ref_name = inner_schema["$ref"].rsplit("/", 1)[-1]
                if ref_name != tag:
                    raise GeneratorError(
                        f"variant tag {tag!r} doesn't match its referenced type {ref_name!r} "
                        "-- the generator only supports Rust's convention of a tuple variant "
                        "sharing its payload struct's name"
                    )
                self.resolve_def(ref_name)
                return
            self._emit_struct(tag, inner_schema)
            return
        # Non-struct payload (array, scalar, or a nested union): the wire form is
        # `{"Tag": <payload>}` with the payload appearing directly, not nested
        # under a field name, so this needs a single generic `value` field.
        inner_type = self.resolve_type(inner_schema)
        lines = ["@dataclass", f"class {tag}:"]
        if doc:
            lines.append(f'    """{doc}"""')
        lines.append(f"    value: {inner_type}")
        self.emitted[tag] = "\n".join(lines)
        self.newtype_classes.append(tag)


def load_schema(name: str) -> dict:
    return json.loads((SCHEMAS_DIR / name).read_text())


def main() -> None:
    host = load_schema("host_to_mote.json")
    mote = load_schema("mote_to_host.json")

    defs: dict[str, dict] = {}
    for schema in (host, mote):
        for def_name, node in schema.get("$defs", {}).items():
            if def_name in defs and defs[def_name] != node:
                raise GeneratorError(f"conflicting definitions for {def_name!r}")
            defs[def_name] = node

    gen = Generator(defs)
    gen._emit_union("HostToMoteMessage", host)
    gen._emit_union("MoteToHostMessage", mote)

    body = "\n\n\n".join(gen.emitted.values())
    newtype_set = (
        "_NEWTYPE_CLASSES = {" + ", ".join(gen.newtype_classes) + "}"
        if gen.newtype_classes
        else "_NEWTYPE_CLASSES: set = set()"
    )
    public_names = list(gen.emitted.keys()) + ["to_wire_json", "from_wire_json"]
    all_decl = "__all__ = [\n" + "".join(f"    {n!r},\n" for n in public_names) + "]\n"

    OUT_FILE.write_text(
        BANNER + all_decl + "\n\n" + body + "\n\n\n" + newtype_set + "\n" + RUNTIME
    )
    print(f"Generated {OUT_FILE}")


if __name__ == "__main__":
    main()
