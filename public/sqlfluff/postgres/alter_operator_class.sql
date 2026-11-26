alter operator class int4_custom_ops using btree rename to int4_new_ops;

alter operator class int4_new_ops using btree owner to joe;

alter operator class int4_new_ops using btree owner to current_role;

alter operator class int4_new_ops using btree owner to current_user;

alter operator class int4_new_ops using btree owner to session_user;

alter operator class int4_new_ops using btree set schema test_schema;

alter operator class public.gist__int_ops using gist owner to joe;
