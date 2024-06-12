use candid::CandidType;

#[derive(CandidType)]
pub enum ManagerError {
    NonExistentValue,
}
