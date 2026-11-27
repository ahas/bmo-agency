-- A supervisor's name tag: the words its answers carry before them where they go out in its owner's own account,
-- an emoji among them as its owner writes it (docs/supervisors.md, 「Its name tag」). None keeps the default.
ALTER TABLE "supervisors" ADD COLUMN "name_tag" text;
