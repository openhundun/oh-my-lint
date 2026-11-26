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

select t1.field, '2001-09-28'::date + 7 + t1.data as a from t1;

select t1.field, now() + '1 hour'::interval + t1.data as a from t1;

select t1.field, t1.data + '03:00'::time as a from t1;

select t1.field, '1 day'::interval + '1 hour 2 minute'::interval + t1.data as a from t1;

select t1.field, '2001-09-28 01:00'::timestamp + '23 hour'::interval + t1.data as a from t1;

select t1.field, '01:00'::time + '3 second'::interval + t1.data as a from t1;

select t1.field, -'23 hour'::interval + t1.data as a from t1;

select t1.field, '1 hour'::interval / 1.5 + t1.data as a from t1;

select t1.field, '1 hour'::interval * 3.5 + t1.data as a from t1;


select t1.field, '2005-04-02 12:00:00-07'::timestamptz + '24 hour'::interval + t1.data as a from t1;
