select * from unnest(array['123', '456']);

select * from unnest(array['123', '456']) a(val, row_num);

select * from unnest(array['123', '456']) with ordinality;

select * from unnest(array['123', '456']) with ordinality a(val, row_num);

select * from table_1
where (utc_activity_start_dttm + make_interval(mins := activity_dur_mnt)
    between '2024-01-07T00:00:00'::timestamp and '2024-01-14T23:59:59.999999'::timestamp);

select array(select unnest(list_field_1) intersect select unnest(list_field_2)) from table_1;
