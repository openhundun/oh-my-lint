create text search configuration my_config(
    parser=my_parser
);
create text search configuration public.my_config(
    parser=pg_catalog."default"
);
create text search configuration copy_config(
    copy=pg_catalog.english
);
