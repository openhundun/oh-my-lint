alter operator family alt_fam2 using btree rename to alt_fam3;

alter operator family alt_fam3 using btree owner to joe;

alter operator family alt_fam3 using btree owner to current_role;

alter operator family alt_fam3 using btree owner to current_user;

alter operator family alt_fam3 using btree owner to session_user;

alter operator family alt_fam3 using btree set schema test_schema;

alter operator family public.custom_fam using hash owner to joe;

alter operator family integer_ops using btree add
  operator 6 < (time, time),
  operator 7 <= (time, time),
  operator 8 = (time, time),
  function 1 btint42cmp(int4, int2);

alter operator family integer_ops using btree drop
  operator 6 (time, time),
  function 1 (int4, int2);
