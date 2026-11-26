select t1.field, extract(century from t1.sometime) as a from t1;

select t1.field, extract(decade from t1.sometime) as a from t1;

select t1.field, extract(dow from t1.sometime) as a from t1;

select t1.field, extract(doy from t1.sometime) as a from t1;

select t1.field, extract(epoch from t1.sometime) as a from t1;

select t1.field, extract(isodow from t1.sometime) as a from t1;

select t1.field, extract(isoyear from t1.sometime) as a from t1;

select t1.field, extract(microseconds from t1.sometime) as a from t1;

select t1.field, extract(millennium from t1.sometime) as a from t1;

select t1.field, extract(milliseconds from t1.sometime) as a from t1;

select t1.field, extract(timezone from t1.sometime) as a from t1;

select t1.field, extract(timezone_hour from t1.sometime) as a from t1;

select t1.field, extract(timezone_minute from t1.sometime) as a from t1;

select t1.field, date '2001-09-28' + 7 + t1.data as a from t1;

select t1.field, now() + interval '1 HOUR' + t1.data as a from t1;

select t1.field, t1.data + time '03:00' as a from t1;

select t1.field, interval '1 DAY' + interval '1 HOUR 2 MINUTES' + t1.data as a from t1;

select t1.field, timestamp '2001-09-28 01:00' + interval '23 HOURS' + t1.data as a from t1;

select t1.field, time '01:00' + interval '3S' + t1.data as a from t1;

select t1.field, -interval '23 HOURS' + t1.data as a from t1;

select t1.field, interval '1 HOUR' / 1.5 + t1.data as a from t1;

select t1.field, interval '1' hour * 3.5 + t1.data as a from t1;


select t1.field, timestamp with time zone '2005-04-02 12:00:00-07' + interval '24 HOURS' + t1.data as a from t1
