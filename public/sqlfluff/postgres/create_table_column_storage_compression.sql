create table storage_test (
    a text storage plain,
    b text storage external,
    c text storage extended,
    d text storage main,
    e text
);

create table compression_test (
    a text compression pglz,
    b text compression lz4,
    c text
);

create table combined_test (a text storage main compression lz4 collate "C" not null, b jsonb storage external default '{}');

create table kw_names (storage text, compression int);
