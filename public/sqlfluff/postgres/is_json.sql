-- PostgreSQL 16 IS JSON syntax tests
-- Based on: https://www.postgresql.org/docs/16/functions-json.html#FUNCTIONS-SQLJSON-MISC

-- Basic IS JSON syntax
select '{}' is json;
select '[]' is json;
select '"test"' is json;
select 'invalid' is json;

-- IS NOT JSON syntax
select 'invalid' is not json;
select '{}' is not json;

-- IS JSON with type specification
select '{}' is json object;
select '[]' is json array;
select '"test"' is json scalar;
select '{"a": 1}' is json value;

-- IS NOT JSON with type specification
select '"test"' is not json object;
select '{}' is not json array;
select '[]' is not json scalar;
select 'invalid' is not json value;

-- IS JSON with UNIQUE KEYS
select '{"a": 1, "b": 2}' is json with unique keys;
select '{"a": 1, "a": 2}' is json with unique keys;
select '{"a": 1, "b": 2}' is json without unique keys;

-- IS JSON with type and unique keys
select '{"a": 1, "b": 2}' is json object with unique keys;
select '{"a": 1, "a": 2}' is json object with unique keys;
select '{"a": 1, "b": 2}' is json object without unique keys;

-- IS NOT JSON with type and unique keys
select '[]' is not json object with unique keys;
select '[1, 2]' is not json object without unique keys;

-- Complex expressions with IS JSON
select col1 is json, col2 is not json array from table1;
select (case when data is json object then 'valid' else 'invalid' end) from table1;

-- IS JSON in WHERE clauses
select * from table1 where data is json;
select * from table1 where config is not json object;
select * from table1 where metadata is json with unique keys;

-- IS JSON with column expressions
select (column_name::text) is json from table1;
select coalesce(data, '{}') is json object from table1;
