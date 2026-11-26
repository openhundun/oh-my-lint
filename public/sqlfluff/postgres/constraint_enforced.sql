-- [ ENFORCED | NOT ENFORCED ] constraint attribute (PostgreSQL 18)
-- https://www.postgresql.org/docs/18/sql-createtable.html
-- https://www.postgresql.org/docs/18/sql-altertable.html

-- Column constraints
create table orders (
    id int primary key,
    customer_id int references customers (id) not enforced,
    quantity int check (quantity > 0) enforced,
    price decimal constraint price_positive check (price > 0) not enforced
);

create table line_items (order_id int references orders (id)
        deferrable initially deferred not enforced);

-- Table constraints
create table shipments (
    id int,
    order_id int,
    weight decimal,
    constraint shipments_order_fk foreign key (order_id)
        references orders (id) not enforced,
    constraint shipments_weight_check check (weight >= 0) enforced
);

alter table shipments add constraint shipments_id_check check (id > 0) not enforced;

alter table shipments add constraint shipments_order_fk2 foreign key (order_id) references orders (id) on delete cascade not enforced;

-- ALTER CONSTRAINT
ALTER TABLE shipments ALTER CONSTRAINT shipments_order_fk ENFORCED;

ALTER TABLE shipments ALTER CONSTRAINT shipments_order_fk NOT ENFORCED;

ALTER TABLE shipments
    ALTER CONSTRAINT shipments_order_fk DEFERRABLE INITIALLY IMMEDIATE ENFORCED;
