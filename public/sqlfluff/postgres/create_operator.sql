create operator >=(
    leftarg = semantic_version,
    rightarg = semantic_version,
    procedure = semantic_version_ge,
    commutator = <=
);

CREATE OPERATOR === (
    LEFTARG = box,
    RIGHTARG = box,
    FUNCTION = area_equal_function,
    COMMUTATOR = ===,
    NEGATOR = !==,
    RESTRICT = area_restriction_function,
    JOIN = area_join_function,
    HASHES, MERGES
);

-- pg_dump writes the schema of the operator, and `OPERATOR(schema.name)`
CREATE OPERATOR app.=== (
    FUNCTION = app.int_eq,
    LEFTARG = integer,
    RIGHTARG = integer,
    COMMUTATOR = OPERATOR(app.===),
    NEGATOR = OPERATOR(app.!==)
);

create operator <<-> (
    leftarg = double precision,
    rightarg = double precision,
    function = float8_distance,
    commutator = <<->
);

create operator ~~~ (
    leftarg = character varying,
    rightarg = text[],
    function = varchar_in_array
);

create operator public.@@ (
    rightarg = timestamp with time zone,
    function = age_of
);

create operator public.## (
    leftarg = point,
    rightarg = lseg,
    function = close_ps
);
