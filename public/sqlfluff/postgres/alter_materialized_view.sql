ALTER MATERIALIZED VIEW bar ALTER column_name SET STATISTICS 1;
ALTER MATERIALIZED VIEW bar ALTER COLUMN column_name SET STATISTICS 1;
ALTER MATERIALIZED VIEW bar ALTER column_name SET ( attribute_option = 1);
ALTER MATERIALIZED VIEW bar ALTER COLUMN column_name SET (
    attribute_option1 = 'avalue', attribute_option2 = 'avalue'
);
ALTER MATERIALIZED VIEW bar ALTER column_name RESET ( attribute_option );
ALTER MATERIALIZED VIEW bar ALTER column_name RESET (
    attribute_option, attribute_option2
);
ALTER MATERIALIZED VIEW bar ALTER COLUMN column_name RESET ( attribute_option );
ALTER MATERIALIZED VIEW bar ALTER COLUMN column_name RESET (
    attribute_option, attribute_option2
);
ALTER MATERIALIZED VIEW bar ALTER column_name SET STORAGE PLAIN;
ALTER MATERIALIZED VIEW bar ALTER COLUMN column_name SET STORAGE EXTENDED;
ALTER MATERIALIZED VIEW bar ALTER COLUMN column_name SET STORAGE DEFAULT;

ALTER MATERIALIZED VIEW bar CLUSTER ON index_name;
ALTER MATERIALIZED VIEW bar SET WITHOUT CLUSTER;

alter materialized view bar set ( storage_parameter);
alter materialized view bar set ( storage_parameter, storage_parameter);
alter materialized view bar set (
    storage_parameter='some_value', storage_parameter
);
ALTER MATERIALIZED VIEW bar SET ( timescaledb.materialized_only = false );

alter materialized view bar reset( storage_parameter);
alter materialized view bar reset( storage_parameter, storage_parameter);
ALTER MATERIALIZED VIEW bar RESET ( timescaledb.materialized_only );

alter materialized view bar owner to baz_role;
alter materialized view bar owner to "baz-role";

ALTER MATERIALIZED VIEW bar DEPENDS ON EXTENSION baz;
ALTER MATERIALIZED VIEW bar NO DEPENDS ON EXTENSION baz;

alter materialized view bar rename column_name to new_column_name;
alter materialized view if exists bar rename column column_name to new_column_name;

alter materialized view bar rename to baz;
alter materialized view if exists bar rename to baz;

alter materialized view bar set schema new_schema;
alter materialized view if exists bar set schema new_schema;

ALTER MATERIALIZED VIEW ALL IN TABLESPACE old_tablespace SET TABLESPACE new_tablespace;
ALTER MATERIALIZED VIEW ALL IN TABLESPACE old_tablespace SET TABLESPACE new_tablespace NOWAIT;

ALTER MATERIALIZED VIEW ALL IN TABLESPACE old_tablespace OWNED BY role_name SET TABLESPACE new_tablespace;
ALTER MATERIALIZED VIEW ALL IN TABLESPACE old_tablespace OWNED BY role_name SET TABLESPACE new_tablespace NOWAIT;

ALTER MATERIALIZED VIEW ALL IN TABLESPACE old_tablespace OWNED BY role_name,
role_name_2 SET TABLESPACE new_tablespace;
ALTER MATERIALIZED VIEW ALL IN TABLESPACE old_tablespace OWNED BY role_name,
role_name_2 SET TABLESPACE new_tablespace NOWAIT;
