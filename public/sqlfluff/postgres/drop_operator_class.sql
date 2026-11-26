drop operator class widget_ops using btree;

drop operator class if exists widget_ops using btree;

drop operator class widget_ops using btree cascade;

drop operator class widget_ops using btree restrict;

drop operator class if exists public.widget_ops using gist cascade;
