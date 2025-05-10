pub mod process;

#[derive(Debug)]
pub enum OsEvent {
    ProcessCreation(u32),
    ProcessDeletion(u32),
}
