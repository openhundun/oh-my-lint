-- issue:2094
alter view myview alter date_column set default now();
alter view myview alter int_column set default 1;
alter view myview alter text_column set default 'value';
alter view myview alter bool_column set default false;
alter view myview alter other_column set default other_value;
alter view myview alter other_column set default current_timestamp;
alter view myview alter other_column set default a_function(a_parameter);
alter view myview alter other_column set default a_function('a_parameter');
alter view myview alter other_column drop default;
alter view if exists myview alter date_column set default now();
alter view if exists myview alter int_column set default 1;
alter view if exists myview alter text_column set default 'value';
alter view if exists myview alter bool_column set default false;
alter view if exists myview alter other_column set default other_value;
alter view if exists myview alter other_column set default current_timestamp;
alter view if exists myview alter other_column set default a_function(a_parameter);
alter view if exists myview alter other_column set default a_function('a_parameter');
alter view if exists myview alter other_column drop default;

alter view myview owner to baz_role;
alter view myview owner to "baz-role";
alter view myview owner to current_role;
alter view myview owner to current_user;
alter view myview owner to session_user;
alter view if exists myview owner to baz_role;
alter view if exists myview owner to "baz-role";
alter view if exists myview owner to current_role;
alter view if exists myview owner to current_user;
alter view if exists myview owner to session_user;

alter view myview rename column_name to new_column_name;
alter view myview rename column column_name to new_column_name;
alter view if exists myview rename column_name to new_column_name;
alter view if exists myview rename column column_name to new_column_name;

alter view myview rename to new_name;
alter view if exists myview rename to new_name;

alter view myview set schema new_schema;
alter view if exists myview set schema new_schema;

alter view myview set ( view_option_name );
alter view myview set ( view_option_name=1);
alter view myview set ( view_option_name=1, view_option_name2='value', view_option_name3, view_option_name4=false);
ALTER VIEW myview SET ( timescaledb.materialized_only = false );
alter view if exists myview set ( view_option_name );
alter view if exists myview set ( view_option_name=1);
alter view if exists myview set ( view_option_name, view_option_name2=1, view_option_name3);

alter view myview reset( view_option_name );
alter view myview reset( view_option_name, view_option_name2 );
ALTER VIEW myview RESET ( timescaledb.materialized_only );
alter view if exists myview reset( view_option_name );
alter view if exists myview reset( view_option_name, view_option_name2 );
