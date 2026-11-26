create sequence foo;

create sequence foo as integer;

create sequence foo increment by 3;

create sequence foo minvalue 5 no maxvalue;

create sequence foo no minvalue maxvalue 12;

create sequence foo increment 5 start with 8 cache 4;

create sequence foo no cycle;

create sequence foo owned by none;

create sequence foo owned by my_table.my_column;

create temp sequence if not exists foo;

create temporary sequence foo;

create sequence derp increment by -5 minvalue + 50 maxvalue - 5;;
