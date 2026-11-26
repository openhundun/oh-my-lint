select json_value(raw_data, '$."action_type"' returning smallint);
select json_value(raw_data, '$."name"' returning text);
select json_value(raw_data, '$."price"' returning numeric);
select json_value(raw_data, '$."active"' returning boolean);
select json_value(raw_data, '$."timestamp"' returning timestamp);
select json_value(raw_data, '$."id"');
create table audit_log_entries (
    id          bigint   not null,
    action_type smallint not null generated always as (json_value(raw_data, '$."action_type"' returning smallint)) stored,
    target_id   bigint            generated always as (json_value(raw_data, '$."target_id"' returning bigint)) stored
);
