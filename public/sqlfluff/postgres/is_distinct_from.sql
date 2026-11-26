-- IS [NOT] DISTINCT FROM in the SELECT clause
select null is distinct from true;
select null is not distinct from true;

-- IS DISTINCT FROM with column references and FROM clause
select a is distinct from b from t;
select a is not distinct from b from t;

-- IS DISTINCT FROM with SELECT DISTINCT modifier
select distinct a is distinct from b from t;

-- IS DISTINCT FROM with multiple select elements
select a, b is distinct from c, d from t;

-- IS DISTINCT FROM followed by INTO
select a is distinct from b into new_t from t;

-- IS [NOT] DISTINCT FROM with inline and block comments
select a is -- inline comment
distinct from b from t;
select a is /* block comment */ not distinct from b from t;

-- IS DISTINCT FROM in WHERE clause
select a from t where a is distinct from b;
select a from t where a is not distinct from b;
