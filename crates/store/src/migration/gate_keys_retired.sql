-- The sanitiser and output-format gates no turn asks any more: the gate kinds a gate declared under their names
-- and every supervisor's switch of them go, so no screen lists them.
DELETE FROM "gate_kinds" WHERE "key" IN ('sanitizer', 'output-format');
DELETE FROM "settings" WHERE "key" LIKE 'agent\_gate:%' ESCAPE '\'
  AND split_part("key", ':', 3) IN ('sanitizer', 'output-format', 'hacking-gate');
