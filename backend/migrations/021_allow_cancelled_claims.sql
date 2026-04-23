-- Allow employees/admins to cancel expense claims before permanent deletion.

ALTER TABLE claims
    DROP CONSTRAINT IF EXISTS claims_status_check;

ALTER TABLE claims
    ADD CONSTRAINT claims_status_check
    CHECK (status IN ('draft', 'pending', 'approved', 'rejected', 'processed', 'cancelled'));
