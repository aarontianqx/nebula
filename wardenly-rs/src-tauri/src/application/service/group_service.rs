use crate::domain::model::Group;
use crate::domain::repository::{GroupRepository, Result};

pub struct GroupService<R: GroupRepository> {
    repo: R,
}

impl<R: GroupRepository> GroupService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn get_all(&self) -> Result<Vec<Group>> {
        self.repo.find_all()
    }

    #[allow(dead_code)]
    pub fn get_by_id(&self, id: &str) -> Result<Option<Group>> {
        self.repo.find_by_id(id)
    }

    pub fn create(&self, group: Group) -> Result<Group> {
        self.repo.save(&group)?;
        Ok(group)
    }

    pub fn update(&self, group: Group) -> Result<Group> {
        self.repo.save(&group)?;
        Ok(group)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.repo.delete(id)
    }
}

