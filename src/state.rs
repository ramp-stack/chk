use crate::{AvatarIconStyle, AvatarContent};

#[derive(Clone, Debug, Default)]
pub struct CurrentProject {
    pub inner: Project
}

#[derive(Clone, Debug)]
pub struct AllProjects {
    pub inner: Vec<Project>
}

impl Default for AllProjects {
    fn default() -> Self {
        AllProjects {
            inner: vec![Project::default()]
        }
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub avatar: AvatarContent,
    pub name: String,
    pub date: String, 
    pub id: String,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            avatar: AvatarContent::icon("home", AvatarIconStyle::Brand),
            name: "Orange".to_string(),
            date: "11/23/2025".to_string(), 
            id: "projectid200".to_string(),
        }
    }
}