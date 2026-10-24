-- The gate kinds the gate declares (docs/api.md, 「Gate kinds」): its order, kept as the gate publishes it.
CREATE TABLE "gate_kinds" (
  "key" text NOT NULL,
  "part" text NOT NULL,
  "step" bigint NOT NULL,
  "position" bigint NOT NULL,
  "condition" text,
  "switchable" bigint NOT NULL DEFAULT 0,
  "laya_probe" bigint NOT NULL DEFAULT 0,
  "default_laya_mode" text,
  "answer_tokens" bigint,
  "declared_at" bigint NOT NULL,
  PRIMARY KEY ("key", "part", "step")
);
