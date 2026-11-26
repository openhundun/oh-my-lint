ALTER DEFAULT PRIVILEGES FOR USER my_user GRANT SELECT ON TABLES TO my_group;
alter default privileges for user my_user in schema my_schema grant insert on tables to my_group;
alter default privileges for role my_user grant update on tables to my_group;
alter default privileges for role my_user in schema my_schema, your_schema grant delete on tables to my_group with grant option;
alter default privileges for role my_user, your_user grant truncate on tables to group my_group;
alter default privileges for role my_user grant references, trigger on tables to public;
alter default privileges for role my_user grant all on sequences to group my_group, group your_group;
alter default privileges for role my_user grant execute on routines to my_group;
alter default privileges grant all privileges on functions to my_group;
alter default privileges in schema my_schema, your_schema grant usage on types to my_group, group your_group;
alter default privileges for role my_user grant usage on schemas to my_group;
alter default privileges for role my_user grant create on schemas to my_group;

ALTER DEFAULT PRIVILEGES FOR USER my_user, your_user IN SCHEMA my_schema, your_schema
GRANT SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER
ON TABLES TO GROUP my_group, PUBLIC
WITH GRANT OPTION;

alter default privileges for user my_user revoke all on tables from my_group;
ALTER DEFAULT PRIVILEGES FOR USER my_user IN SCHEMA my_schema REVOKE SELECT ON TABLES FROM GROUP my_group;
alter default privileges for role my_user in schema my_schema, your_schema revoke insert on tables from public cascade;
alter default privileges for role my_user in schema my_schema revoke update, delete, truncate, references, trigger on tables from public restrict;
alter default privileges for role my_user, your_user revoke all privileges on sequences from my_group;
alter default privileges for role my_user revoke execute on functions from my_group;
alter default privileges revoke execute on routines from my_group;
alter default privileges in schema my_schema, your_schema revoke usage on types from my_group, group your_group;
alter default privileges for role my_user revoke usage, create on types from my_group;

ALTER DEFAULT PRIVILEGES FOR ROLE my_user, your_user IN SCHEMA my_schema, your_schema
REVOKE SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER
ON TABLES FROM GROUP my_group, PUBLIC
CASCADE;
