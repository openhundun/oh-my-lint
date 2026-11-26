select (test_column is normalized) as is_normalized from test_table;
select (test_column is nfc normalized) as is_normalized from test_table;
select (test_column is nfd normalized) as is_normalized from test_table;
select (test_column is nfkc normalized) as is_normalized from test_table;
select (test_column is nfkd normalized) as is_normalized from test_table;
select (test_column is not normalized) as is_normalized from test_table;
select (test_column is not nfc normalized) as is_normalized from test_table;
select (test_column is not nfd normalized) as is_normalized from test_table;
select (test_column is not nfkc normalized) as is_normalized from test_table;
select (test_column is not nfkd normalized) as is_normalized from test_table;

create domain text_default as text check (value is normalized);
create domain text_nfc as text check (value is nfc normalized);
create domain text_nfd as text check (value is nfd normalized);
create domain text_nfkc as text check (value is nfkc normalized);
create domain text_nfkd as text check (value is nfkd normalized);
create domain text_default as text check (value is not normalized);
create domain text_nfc as text check (value is not nfc normalized);
create domain text_nfd as text check (value is not nfd normalized);
create domain text_nfkc as text check (value is not nfkc normalized);
create domain text_nfkd as text check (value is not nfkd normalized);


create table test_table (
    test_column text not null,
    constraint default_constraint check (test_column is normalized),
    constraint nfc_constraint check (test_column is nfc normalized),
    constraint nfd_constraint check (test_column is nfd normalized),
    constraint nfkc_constraint check (test_column is nfkc normalized),
    constraint nfkd_constraint check (test_column is nfkd normalized),
    constraint not_default_constraint check (test_column is not normalized),
    constraint not_nfc_constraint check (test_column is not nfc normalized),
    constraint not_nfd_constraint check (test_column is not nfd normalized),
    constraint not_nfkc_constraint check (test_column is not nfkc normalized),
    constraint not_nfkd_constraint check (test_column is not nfkd normalized),
    primary key (test_column)
);
