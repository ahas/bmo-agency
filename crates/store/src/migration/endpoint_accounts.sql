ALTER TABLE "claude_accounts" ADD COLUMN "endpoint_url" text;
ALTER TABLE "claude_accounts" ADD COLUMN "key_file" text;
ALTER TABLE "claude_accounts" ADD COLUMN "endpoint_models" text;
ALTER TABLE "claude_accounts" ADD COLUMN "base_prompt" text REFERENCES "base_prompts" ("name");
