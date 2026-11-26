select
    ca_state,
    cd_gender,
    cd_marital_status,
    cd_dep_count,
    count(*) as cnt1,
    min(cd_dep_count) as min1,
    max(cd_dep_count) as max1,
    avg(cd_dep_count) as avg1,
    cd_dep_employed_count,
    count(*) as cnt2,
    min(cd_dep_employed_count) as min2,
    max(cd_dep_employed_count) as max2,
    avg(cd_dep_employed_count) as avg2,
    cd_dep_college_count,
    count(*) as cnt3,
    min(cd_dep_college_count),
    max(cd_dep_college_count),
    avg(cd_dep_college_count)
from customer c, customer_address ca, customer_demographics
where 1 = 1
    and c.c_current_addr_sk = ca.ca_address_sk
    and cd_demo_sk = c.c_current_cdemo_sk
    and exists (
        select *
        from store_sales, date_dim
        where 1 = 1
            and c.c_customer_sk = ss_customer_sk
            and ss_sold_date_sk = d_date_sk
            and d_year = 2002
            and d_qoy < 4
    )
    and (
        exists (
            select *
            from web_sales, date_dim
            where (
                1 = 1
                and c.c_customer_sk = ws_bill_customer_sk
                and ws_sold_date_sk = d_date_sk
                and d_year = 2002
                and d_qoy < 4
            )
        )
        or exists (
            select *
            from catalog_sales, date_dim
            where 1 = 1
                and c.c_customer_sk = cs_ship_customer_sk
                and cs_sold_date_sk = d_date_sk
                and d_year = 2002
                and d_qoy < 4
        )
    )
group by
    ca_state,
    cd_gender,
    cd_marital_status,
    cd_dep_count,
    cd_dep_employed_count,
    cd_dep_college_count
order by
    ca_state nulls first,
    cd_gender nulls first,
    cd_marital_status nulls first,
    cd_dep_count nulls first,
    cd_dep_employed_count nulls first,
    cd_dep_college_count nulls first
limit 100
