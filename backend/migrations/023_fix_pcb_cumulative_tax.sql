-- Correct the `pcb_brackets.cumulative_tax` column for tax year 2024.
--
-- Migration 007 seeded cumulative_tax values that were exactly 100× larger
-- than they should be (e.g. RM15,000 stored where RM150 was correct), so
-- `calculate_tax_from_brackets` was inflating annual tax by roughly two
-- orders of magnitude for every employee whose chargeable income crossed
-- into the 3%+ brackets.
--
-- Code path: services/pcb_calculator.rs::calculate_tax_from_brackets adds
-- `cumulative` directly to a `bracket_tax` computed in sen, so `cumulative`
-- must also be in sen. All other columns (income_from, income_to) are
-- already in sen; only cumulative_tax was mis-scaled.
--
-- Correct cumulative tax at the start of each 2024 bracket (sen):
--   RM5,001     →       0
--   RM20,001    →  15,000
--   RM35,001    →  60,000
--   RM50,001    → 150,000
--   RM70,001    → 370,000
--   RM100,001   → 940,000
--   RM400,001   → 8,440,000
--   RM600,001   → 13,640,000
--   RM2,000,001 → 52,840,000

UPDATE pcb_brackets SET cumulative_tax = 15000       WHERE effective_year = 2024 AND chargeable_income_from = 2000001;
UPDATE pcb_brackets SET cumulative_tax = 60000       WHERE effective_year = 2024 AND chargeable_income_from = 3500001;
UPDATE pcb_brackets SET cumulative_tax = 150000      WHERE effective_year = 2024 AND chargeable_income_from = 5000001;
UPDATE pcb_brackets SET cumulative_tax = 370000      WHERE effective_year = 2024 AND chargeable_income_from = 7000001;
UPDATE pcb_brackets SET cumulative_tax = 940000      WHERE effective_year = 2024 AND chargeable_income_from = 10000001;
UPDATE pcb_brackets SET cumulative_tax = 8440000     WHERE effective_year = 2024 AND chargeable_income_from = 40000001;
UPDATE pcb_brackets SET cumulative_tax = 13640000    WHERE effective_year = 2024 AND chargeable_income_from = 60000001;
UPDATE pcb_brackets SET cumulative_tax = 52840000    WHERE effective_year = 2024 AND chargeable_income_from = 200000001;
