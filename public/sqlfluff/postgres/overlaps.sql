-- with DATE
select
    start_date,
    end_date
from test_overlaps
where (start_date, end_date) overlaps ('2023-02-15'::date, '2023-03-15'::date);

select start_date, end_date from test_overlaps
where (start_date, end_date) overlaps ('2023-02-15', '2023-03-15');

select t1.start_date, t1.end_date
from test_overlaps1 t1
left join test_overlaps2 t2
where (t1.start_date, t1.end_date) overlaps (t2.start_date, t2.end_date);

select start_date, end_date from test_overlaps
where (start_date, end_date) overlaps ('2023-12-30T00:00:00'::timestamp, '2024-01-14T13:01:39.884877'::timestamp);

select start_date, end_date from test_overlaps
where ('2023-12-30T00:00:00'::timestamp, '2024-01-14T13:01:39.884877'::timestamp) overlaps (start_date, end_date);

select start_date, end_date from test_overlaps
where (start_date, end_date) overlaps ('2023-12-30'::date, '2 hour'::interval);

select start_date, end_date from test_overlaps
where ('2023-12-30'::date, '2024-01-14'::date) overlaps (start_date, end_date);

select start_date_1, start_date_2, end_date from test_overlaps
where ('2023-12-30'::date, '2024-01-14'::date) overlaps (greatest(start_date_1, start_date_2), end_date);
