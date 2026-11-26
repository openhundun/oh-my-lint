-- SQL from issue #2033
select coalesce(doc#>>'{fields}', '') as field
from mytable
where doc ->> 'some_field' = 'some_value';

-- Get JSON array element (indexed from zero, negative integers count from the end)
select '[{"a":"foo"},{"b":"bar"},{"c":"baz"}]'::json->2;
-- Get JSON object field by key
select '{"a": {"b":"foo"}}'::json->'a';
-- Get JSON array element as text
select '[1,2,3]'::json->>2;
-- Get JSON object field as text
select '{"a":1,"b":2}'::json->>'b';
-- Get JSON object at the specified path
select '{"a": {"b":{"c": "foo"}}}'::json#>'{a,b}';
-- Get JSON object at the specified path as text
select '{"a":[1,2,3],"b":[4,5,6]}'::json#>>'{a,2}';

select
    '{"a":1, "b":2}'::jsonb @> '{"b":2}'::jsonb,
    '{"b":2}'::jsonb <@ '{"a":1, "b":2}'::jsonb,
    '{"a":1, "b":2}'::jsonb ? 'b',
    '{"a":1, "b":2, "c":3}'::jsonb ?| array['b', 'd'],
    '["a", "b", "c"]'::jsonb ?& array['a', 'b'],
    '["a", {"b":1}]'::jsonb #- '{1,b}',
    '{"a":[1,2,3,4,5]}'::jsonb @? '$.a[*] ? (@ > 2)',
    '{"a":[1,2,3,4,5]}'::jsonb @@ '$.a[*] > 2';

select json_object('code' value 'P123', 'title': 'Jaws');

select json_object(
        'word': case
            when str = '1' then 'One'
            when str = '2' then 'Two'
            else 'Unknown'
        end
    ) as json_column
from first_table;
