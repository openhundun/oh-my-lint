create function mleast(variadic arr numeric[]) returns numeric as $$
    SELECT min($1[i]) FROM generate_subscripts($1, 1) g(i);
$$ language sql;

SELECT mleast(VARIADIC ARRAY[10, -1, 5, 4.4]);

SELECT mleast(VARIADIC ARRAY[]::numeric[]);

SELECT mleast(VARIADIC arr := ARRAY[10, -1, 5, 4.4]);

create function anyleast(variadic anyarray) returns anyelement as $$
    SELECT min($1[i]) FROM generate_subscripts($1, 1) g(i);
$$ language sql;

create function concat_values(text, variadic anyarray) returns text as $$
    SELECT array_to_string($2, $1);
$$ language sql;

SELECT my_function(other_function(
    VARIADIC ARRAY_REMOVE(ARRAY[
        a.value1,
        b.value2,
        c.value3
    ], NULL)
))
FROM a
FULL OUTER JOIN b USING (id)
FULL OUTER JOIN c USING (id);

SELECT json_extract_path_text(t.col::json, VARIADIC ARRAY['foo'::text])
FROM t;

SELECT my_function(VARIADIC ARRAY[
    CASE WHEN x > 0 THEN x ELSE 0 END,
    y + z,
    'literal'
]);

SELECT my_function(VARIADIC ARRAY(SELECT value FROM table1));

SELECT my_function(VARIADIC param_name = ARRAY[1, 2, 3]);

SELECT my_function(
    regular_param = 'value',
    VARIADIC variadic_param = ARRAY[1, 2, 3]
);

SELECT my_function(VARIADIC param_name => ARRAY[1, 2, 3]);

SELECT my_function(
    regular_param => 'value',
    VARIADIC variadic_param => ARRAY[1, 2, 3]
);
