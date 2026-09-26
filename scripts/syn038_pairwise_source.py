from __future__ import annotations

from copy import deepcopy
import hashlib
import json
from pathlib import Path


STORAGE_FORMAT = "lapligence.syn038.pairwise-source/v1"
EVIDENCE_DEFAULT_FIELDS = {
    "stderr", "pipeline_mode", "snapshot_drop", "cli_edition_args", "oracle_mode",
}
SOURCE_FIELDS = {
    "schema", "storage_format", "scope", "factors", "excluded_form_lanes",
    "qualifier_catalog", "type_qualifier_scope", "evidence_defaults", "evidence",
    "supplemental_witnesses", "level_defaults", "qualifier_sets", "observations",
    "targeted_chains", "required_selected_pairs", "baseline_counts",
    "catalog_sha256", "rule_basis_sha256", "required_zero_legal_gaps",
}
COMPACT_FIELDS = {
    "storage_format", "evidence_defaults", "level_defaults", "qualifier_sets",
}


def canonical_sha256(value: object) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=True, allow_nan=False).encode("ascii")
    return hashlib.sha256(payload).hexdigest()


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def reject_constant(value: str) -> None:
    raise ValueError(f"non-JSON numeric constant: {value}")


def read_source(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"),
                      object_pairs_hook=unique_object, parse_constant=reject_constant)


def require_object(value: object, label: str) -> dict:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def require_rows(value: object, label: str) -> list[dict]:
    if not isinstance(value, list) or any(not isinstance(row, dict) for row in value):
        raise ValueError(f"{label} must be an array of objects")
    return value


def check_levels(levels: dict, factors: dict, label: str) -> None:
    for key, value in levels.items():
        if key not in factors or (value is not None and
                                  (not isinstance(value, str) or value not in factors[key]["levels"])):
            raise ValueError(f"{label}: unknown {key}={value!r}")


def expand_source(source: dict) -> dict:
    require_object(source, "pairwise source")
    if source.get("storage_format") != STORAGE_FORMAT:
        raise ValueError(f"storage_format must be {STORAGE_FORMAT}")
    missing, extra = SOURCE_FIELDS - source.keys(), source.keys() - SOURCE_FIELDS
    if missing or extra:
        raise ValueError(f"pairwise source fields: missing={sorted(missing)}, unexpected={sorted(extra)}")
    factors = require_object(source["factors"], "factors")
    for key, definition in factors.items():
        definition = require_object(definition, f"factor {key}")
        values = definition.get("levels")
        if not isinstance(values, list) or not values or any(not isinstance(v, str) for v in values):
            raise ValueError(f"factor {key} must have named levels")
    defaults = require_object(source["level_defaults"], "level_defaults")
    if defaults.keys() != factors.keys():
        raise ValueError("level_defaults must define every factor exactly once")
    check_levels(defaults, factors, "level_defaults")
    evidence_defaults = require_object(source["evidence_defaults"], "evidence_defaults")
    if evidence_defaults.keys() != EVIDENCE_DEFAULT_FIELDS:
        raise ValueError("evidence_defaults must define the five shared execution/oracle fields")
    qualifiers = require_object(source["qualifier_sets"], "qualifier_sets")
    for name, values in qualifiers.items():
        require_object(values, f"qualifier set {name}")
    for key in ("catalog_sha256", "rule_basis_sha256"):
        value = source[key]
        if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
            raise ValueError(f"{key} must be a lowercase SHA-256 digest")

    expanded = deepcopy({key: value for key, value in source.items() if key not in COMPACT_FIELDS})
    expanded["evidence"] = [deepcopy({**evidence_defaults, **row})
                            for row in require_rows(source["evidence"], "evidence")]
    observations = []
    for row in require_rows(source["observations"], "observations"):
        obs_id = row.get("id", "observation")
        if "missing_factors" in row:
            raise ValueError(f"{obs_id}: missing_factors is derived from null levels; do not store it")
        overrides = require_object(row.get("levels"), f"{obs_id} levels")
        check_levels(overrides, factors, str(obs_id))
        profile = row.get("qualifiers")
        if not isinstance(profile, str) or profile not in qualifiers:
            raise ValueError(f"{obs_id}: unknown qualifier set {profile!r}")
        levels = {**defaults, **overrides}
        observation = deepcopy(row)
        observation["levels"] = {key: levels[key] for key in factors if levels[key] is not None}
        observation["qualifiers"] = deepcopy(qualifiers[profile])
        missing = [key for key in factors if levels[key] is None]
        if missing:
            if not isinstance(row.get("sparse_basis"), str) or not row["sparse_basis"].strip():
                raise ValueError(f"{obs_id}: sparse factor vector lacks its exact missing-axis basis")
            observation["missing_factors"] = missing
        elif "sparse_basis" in row:
            raise ValueError(f"{obs_id}: full factor vector has stale sparse-axis metadata")
        observations.append(observation)
    expanded["observations"] = observations
    return expanded


def format_source(source: dict) -> str:
    def render(value: object, depth: int, key: str = "") -> str:
        compact = json.dumps(value, ensure_ascii=True, allow_nan=False)
        if isinstance(value, list) and value and any(isinstance(item, dict) for item in value):
            items = ["  " * (depth + 1) + render(item, depth + 1) for item in value]
            return "[\n" + ",\n".join(items) + "\n" + "  " * depth + "]"
        if isinstance(value, dict) and value and key not in ("levels", "level_defaults"):
            items = ["  " * (depth + 1) + json.dumps(name) + ": " + render(item, depth + 1, name)
                     for name, item in value.items()]
            return "{\n" + ",\n".join(items) + "\n" + "  " * depth + "}"
        return compact

    return render(source, 0) + "\n"
