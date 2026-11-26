alter aggregate range_agg_preserve_gaps(tstzrange) rename to my_agg;

alter aggregate my_agg(tstzrange) owner to me;
alter aggregate my_agg(tstzrange) owner to current_role;
alter aggregate my_agg(tstzrange) owner to current_user;
alter aggregate my_agg(tstzrange) owner to session_user;

alter aggregate my_agg(*) set schema api;

alter aggregate complex_agg_function(integer, text, numeric)
rename to renamed_agg_function;
