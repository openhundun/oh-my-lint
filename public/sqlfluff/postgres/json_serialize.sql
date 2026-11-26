select json_serialize('"test"'::json returning bytea);
select json_serialize('"test"'::json returning text);
select json_serialize('"test"'::json);
select json_serialize('{"key": "value"}'::json returning bytea);
select json_serialize('{"key": "value"}'::json returning text);
select json_serialize('{"key": "value"}'::json);
