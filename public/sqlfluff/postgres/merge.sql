-- Merge with WHEN MATCHED THEN DO NOTHING
merge into t using s on t.id = s.id
when matched then do nothing;

-- Merge with WHEN NOT MATCHED THEN DO NOTHING
merge into t using s on t.id = s.id
when not matched then do nothing;

-- Merge with a matched DO NOTHING guarded by an AND condition
merge into t using s on t.id = s.id
when matched and s.flag then do nothing;

-- Merge with a not-matched DO NOTHING guarded by an AND condition
merge into t using s on t.id = s.id
when not matched and s.flag then do nothing;

-- Merge mixing DO NOTHING with the other actions
merge into t using s on t.id = s.id
when matched and s.deleted then delete
when matched then update set val = s.val
when not matched then do nothing;
