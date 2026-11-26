create operator class complex_abs_ops
    default for type complex using btree as
        operator 1 <,
        operator 2 <=,
        operator 3 =,
        operator 4 >=,
        operator 5 >,
        function 1 complex_abs_cmp(complex, complex);

create operator class gist__int_ops
    default for type _int4 using gist as
        operator 3 &&,
        operator 6 = (anyarray, anyarray),
        operator 7 @>,
        operator 8 <@,
        operator 20 @@ (_int4, query_int),
        function 1 g_int_consistent(internal, _int4, smallint, oid, internal),
        function 2 g_int_union(internal, internal),
        function 7 g_int_same(_int4, _int4, internal),
        storage internal;

create operator class myschema.int4_custom_ops
    for type int4 using btree family integer_ops as
        operator 1 < (int4, int4) for search,
        function 1 (int4, int4) btint4cmp(int4, int4);

create operator class point_knn_ops
    for type point using gist as
        operator 15 <-> (point, point) for order by float_ops,
        storage box;
