create operator family alt_fam2 using btree;

create operator family custom_fam using hash;

create operator family public.my_op_fam using gist;
