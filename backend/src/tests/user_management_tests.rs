use uuid::Uuid;

use crate::repositories::{user_companies, users};
use crate::services::user_service;
use crate::tests::support::{seed_company, seed_employee, seed_user, skip_if_no_db};

#[tokio::test]
async fn admin_user_list_includes_employee_only_accounts_in_shared_company() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let admin_id = seed_user(&pool, company_id, "admin").await;
    user_companies::insert(&pool, admin_id, company_id)
        .await
        .expect("link admin company");

    let employee_id = seed_employee(&pool, company_id, None, 500_000).await;
    let employee_user_id = Uuid::now_v7();
    let email = format!("employee-list-{}@example.invalid", Uuid::new_v4());
    users::insert_employee_user(
        &pool,
        employee_user_id,
        &email,
        "unused-test-hash",
        "Employee Account",
        company_id,
        employee_id,
    )
    .await
    .expect("insert employee user");
    user_companies::insert(&pool, employee_user_id, company_id)
        .await
        .expect("link employee company");

    let users = user_service::list_users(&pool, false, admin_id, None)
        .await
        .expect("list users for admin");
    let employee = users
        .iter()
        .find(|user| user.id == employee_user_id)
        .expect("employee account should be visible to company admin");

    assert_eq!(employee.roles, ["employee"]);
    assert_eq!(employee.employee_id, Some(employee_id));
    assert!(
        employee
            .companies
            .iter()
            .any(|company| company.id == company_id)
    );
}

#[tokio::test]
async fn deleted_user_is_hidden_and_cannot_be_restored_by_a_matching_email() {
    let Some(pool) = skip_if_no_db().await else {
        return;
    };
    let company_id = seed_company(&pool).await;
    let super_admin_id = seed_user(&pool, company_id, "super_admin").await;
    let user_id = seed_user(&pool, company_id, "employee").await;
    user_companies::insert(&pool, user_id, company_id)
        .await
        .expect("link employee company");

    user_service::delete_user(&pool, user_id, super_admin_id)
        .await
        .expect("soft delete user");

    assert!(
        users::get_active_by_id(&pool, user_id)
            .await
            .expect("load deleted user")
            .is_none(),
        "a deleted user must not authenticate"
    );
    assert!(
        user_service::list_users(&pool, true, super_admin_id, Some(company_id))
            .await
            .expect("list filtered users")
            .iter()
            .all(|user| user.id != user_id),
        "a deleted user must not reappear in the company user list"
    );

    let deleted = users::find_by_email(
        &pool,
        &users::get_by_id(&pool, user_id)
            .await
            .expect("load deleted tombstone")
            .expect("tombstone exists")
            .email,
    )
    .await
    .expect("find deleted tombstone")
    .expect("deleted tombstone exists");
    assert!(deleted.is_deleted);
}
