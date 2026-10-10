"""Callbacks encode inputs and decode selected output scalars without changing presence."""


def scalar_walk(value, ty, inputs, codecs, direction, depth=0):
    if value is None or ty is None or not codecs:
        return value
    if depth > 256:
        raise ValueError("Scalar codec nesting exceeds 256 levels")
    if "scalar" in ty:
        callback = codecs.get(ty["scalar"], {}).get(direction)
        return callback(value) if callback else value
    if "named" in ty:
        return scalar_walk(value, inputs[ty["named"]], inputs, codecs, direction, depth + 1)
    if "list" in ty:
        return [
            scalar_walk(item, ty["list"], inputs, codecs, direction, depth + 1) for item in value
        ]
    if "fields" in ty:
        result = dict(value)
        for key, field in ty["fields"]:
            if key in value:
                result[key] = scalar_walk(value[key], field, inputs, codecs, direction, depth + 1)
        return result
    if "union" in ty:
        candidates = [
            member
            for member in ty["union"]
            if member is not None
            and "fields" in member
            and all(
                field is None or "literal" not in field or value.get(key) == field["literal"]
                for key, field in member["fields"]
            )
        ]
        if len(candidates) != 1:
            raise ValueError("Scalar decoder needs selected __typename for abstract results")
        return scalar_walk(value, candidates[0], inputs, codecs, direction, depth + 1)
    return value
