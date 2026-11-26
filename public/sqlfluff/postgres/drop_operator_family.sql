drop operator family alt_fam2 using btree;

drop operator family if exists alt_fam2 using btree;

drop operator family alt_fam2 using btree cascade;

drop operator family alt_fam2 using btree restrict;

drop operator family fam1, fam2 using btree;

drop operator family if exists public.fam1, public.fam2 using btree cascade;
