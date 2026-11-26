create aggregate agg_twocols(numeric, numeric) (
   sfunc = mysfunc_accum,
   stype = numeric,
   combinefunc = mycombine_accum,
   initcond = 0
);

create aggregate agg_array_stype(numeric[]) (
   sfunc = mysfunc_accum,
   stype = numeric[]
);
