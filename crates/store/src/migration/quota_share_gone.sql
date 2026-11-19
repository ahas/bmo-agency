-- A road holds no share of its provider's quota back (docs/provider-order.md, 「Quota: what a provider has left」), so a
-- role's row keeps none.
ALTER TABLE "roles" DROP COLUMN "quota_share";
