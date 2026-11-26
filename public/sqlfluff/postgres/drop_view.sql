drop view abc;

drop view "abc";

drop view if exists abc;

drop view abc, "def", ghi;

drop view if exists abc, def, ghi;

-- Test CASCADE trailing keyword

drop view abc cascade;

drop view if exists abc cascade;

drop view abc, def, ghi cascade;

drop view if exists abc, def, ghi cascade;


-- Test RESTRICT trailing keyword

drop view abc restrict;

drop view if exists abc restrict;

drop view abc, def, ghi restrict;

drop view if exists abc, def, ghi restrict;
