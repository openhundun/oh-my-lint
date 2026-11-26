alter table distributors add column address varchar(30);

alter table measurements add column mtime timestamptz default now();

alter table transactions add column status varchar(30) default 'old', alter column status set default 'current';

alter table distributors drop column address restrict;

alter table distributors alter column address type varchar(80), alter column name type varchar(100);

alter table foo alter column foo_timestamp set data type timestamptz using 'epoch'::timestamptz + foo_timestamp * '1 second'::interval;

alter table foo
    alter column foo_timestamp drop default,
    alter column foo_timestamp type timestamptz using 'epoch'::timestamp + foo_timestamp * '1 second'::interval,
    alter column foo_timestamp set default now();

alter table mytable alter date_column set default now();
alter table mytable alter int_column set default 1;
alter table mytable alter text_column set default 'value';
alter table mytable alter bool_column set default false;
alter table mytable alter other_column set default other_value;
alter table mytable alter other_column set default current_timestamp;
alter table mytable alter other_column set default a_function(a_parameter);
alter table mytable alter other_column set default a_function('a_parameter');
alter table mytable alter other_column set default 1 + 2 + 3;
alter table mytable alter other_column set default 1 + 2 + 3;
alter table mytable alter other_column drop default;
alter table if exists mytable alter date_column set default now();
alter table if exists mytable alter int_column set default 1;
alter table if exists mytable alter text_column set default 'value';
alter table if exists mytable alter bool_column set default false;
alter table if exists mytable alter other_column set default other_value;
alter table if exists mytable alter other_column set default current_timestamp;
alter table if exists mytable alter other_column set default a_function(a_parameter);
alter table if exists mytable alter other_column set default a_function('a_parameter');
alter table if exists mytable alter other_column set default 1 + 2 + 3;
alter table if exists mytable alter other_column set default 1 + 2 + 3;
alter table if exists mytable alter other_column drop default;

alter table distributors rename column address to city;

alter table distributors rename to suppliers;

alter table distributors rename constraint zipchk to zip_check;

alter table distributors alter column street set not null;

alter table distributors alter column street drop not null;

alter table distributors add constraint zipchk check (char_length(zipcode) = 5);

alter table distributors add constraint zipchk check (char_length(zipcode) = 5) no inherit;

alter table distributors drop constraint zipchk;

alter table distributors drop constraint "zipchk";

alter table only distributors drop constraint zipchk;

alter table distributors add constraint distfk foreign key (address) references addresses (address);

alter table distributors add constraint distfk foreign key (address) references addresses (address) match full;

alter table distributors add constraint distfk foreign key (address) references addresses (address) on delete restrict on update cascade;

alter table distributors add constraint distfk foreign key (address) references addresses (address) not valid;

alter table distributors validate constraint distfk;

alter table distributors add constraint dist_id_zipcode_key unique (dist_id, zipcode);

alter table distributors add primary key (dist_id);

alter table distributors set tablespace fasttablespace;

alter table distributors set (parameter_1='value');

alter table distributors set (parameter_1=1);

alter table distributors set (parameter_1=1, parameter_2='value');

alter table myschema.distributors set schema yourschema;

alter table distributors drop constraint distributors_pkey, add constraint distributors_pkey primary key using index dist_id_temp_idx;

alter table measurement attach partition measurement_y2016m07 for values from ('2016-07-01') to ('2016-08-01');

alter table cities attach partition cities_ab for values in ('a', 'b');

alter table orders attach partition orders_p4 for values with (modulus 4, remainder 3);

alter table only orders attach partition public.orders_p4 for values with (modulus 4, remainder 3);

alter table cities attach partition cities_partdef default;

alter table measurement detach partition measurement_y2015m12;

alter table measurement detach partition measurement_y2021m10 concurrently finalize;

alter table landing.workorderhistory add constraint workorder_id_foreign_key foreign key (workorderid) references landing.workorder (id);

alter table my_table add column if not exists foo text;

alter table public.obm_buildings owner to postgres;

alter table distributors alter column street add generated always as identity (increment 4 no maxvalue);

alter table distributors alter column street set restart with 3;

alter table distributors add my_column int generated by default as identity (cache 3 maxvalue 63 owned by none);

alter table public.test owner to "ID";

alter table public.test owner to id;

alter table if exists only public.test owner to current_role;

alter table public.history alter column id add generated always as identity (sequence name public.history_id_seq);

alter table tbl
    add column nulls_distinct text unique,
    add column nulls_not_distinct text unique nulls not distinct,
    add everything text unique with (arg1=3, arg5='str') using index tablespace spc;

alter table tbl add pk text default 'hello' primary key with (arg1=3, arg5='str') using index tablespace tblspace not null;

alter table tbl
    add constraint foo1 unique (fld, col),
    add constraint foo2 unique (fld),
    add constraint foo3 unique nulls not distinct (fld),
    add constraint everything unique (fld, col) include (two, three) with (arg1=3, arg5='str') using index tablespace tblspc,
    add constraint pk primary key (fld, col) include (four) with (ff=auto, gg=stuff) using index tablespace tblspc;

alter table foo set (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc);
alter table foo reset (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc);
alter table foo alter column baz set (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc);
alter table foo alter column baz reset (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc);

alter table reservation add exclude using gist (during with &&);
alter table room_reservation add constraint cons exclude using gist (room with =, during with &&);

alter table no_using add exclude (field with =) no inherit;
alter table many_options add exclude
    using gist (
        one with =,
        nulls_opclass nulls with =,
        nulls_last with =,
        two collate "en-US" opclass (opt1, opt2=5, opt3='str', ns.opt4, ns.opt5=6, ns.opt6='str', opt7=asc) nulls first with =,
        (two + 5) with =,
        myfunc(a, b) with =,
        myfunc_opclass(a, b) fop (opt=1, foo=2) with =,
        only_opclass opclass with =,
        desc_order desc with =
    )
    include (a, b)
    with (idx_num=5, idx_str='idx_value', idx_kw=desc)
    using index tablespace tblspc
    where (field != 'def') deferrable not valid initially deferred;

alter table mytable alter column mycolumn set compression lz4;
alter table mytable alter mycolumn set compression lz4;

alter table mytable alter column mycolumn set storage plain;
alter table mytable alter column mycolumn set storage external;
alter table mytable alter column mycolumn set storage extended;
alter table mytable alter column mycolumn set storage main;
alter table mytable alter mycolumn set storage default;
