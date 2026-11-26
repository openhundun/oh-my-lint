-- https://www.postgresql.org/docs/current/sql-altertype.html
alter type foo rename to bar;
ALTER TYPE foo OWNER TO CURRENT_USER;
ALTER TYPE foo OWNER TO new_owner;
ALTER TYPE foo SET SCHEMA new_schema;
ALTER TYPE compfoo ADD ATTRIBUTE f3 int, DROP ATTRIBUTE IF EXISTS f4, ALTER ATTRIBUTE f5 TYPE int;
ALTER TYPE compfoo RENAME ATTRIBUTE f6 TO f7;
alter type colors add value 'orange' after 'red';
alter type foo add value 'baz';
alter type foo add value 'qux' before 'baz';
alter type foo add value 'quux' after 'baz';
alter type financial.reporting_statuses rename value 'partially' to 'partially-reported';
