create type my_type as (
    int_ int4,
    bool_ boolean,
    comment_ text
);

select ((1, true, null)::my_type).int_;
