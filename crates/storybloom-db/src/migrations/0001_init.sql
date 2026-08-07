-- 0001_init.sql
--
-- Baseline migration. Intentionally a no-op: it exists so the migrations
-- directory is non-empty and `sqlx::migrate!` has a valid history to build
-- on. Real schema (stories, chapters, characters, ...) lands in later
-- migration files once features are implemented.
SELECT 1;
