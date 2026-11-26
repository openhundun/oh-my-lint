select word.*, paragraph.id as paragraph_id
from word
inner join paragraph on paragraph.page_id = word.page_id
where
  word.character_range @> paragraph.character_range
  and word.character_range <@ paragraph.character_range
  and word.character_range && paragraph.character_range
  and word.character_range << paragraph.character_range
  and word.character_range >> paragraph.character_range
  and word.character_range &> paragraph.character_range
  and word.character_range &< paragraph.character_range
  and word.character_range -|- paragraph.character_range
  and word.character_range + paragraph.character_range
  and word.character_range * paragraph.character_range
  and word.character_range - paragraph.character_range
