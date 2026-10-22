-- A tier mapping names whether its model thinks (docs/model-tiers.md): null leaves the choice to the
-- agent's own reasoning or the registration, and true or false fixes it for every key on the tier.
ALTER TABLE "model_tier_mappings" ADD COLUMN "thinking" BOOLEAN;
