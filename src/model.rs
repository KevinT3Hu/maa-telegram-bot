use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct TaskStatus {
    pub user: String,
    pub device: String,
    pub task: String,
    pub status: String,
    pub payload: String,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub users: HashMap<String, User>,
}

impl Device {
    pub fn new(id: String) -> Self {
        Self {
            id: id.clone(),
            name: id.clone(),
            users: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: String,
    pub tasks: Vec<Task>,
}

#[derive(Serialize, Clone, Debug)]
pub enum TaskType {
    CaptureImage,
    #[serde(rename = "LinkStart-Base")]
    LinkStartBase,
    #[serde(rename = "LinkStart-WakeUp")]
    LinkStartWakeUp,
    #[serde(rename = "LinkStart-Combat")]
    LinkStartCombat,
    #[serde(rename = "LinkStart-Recruiting")]
    LinkStartRecruiting,
    #[serde(rename = "LinkStart-Mall")]
    LinkStartMall,
    #[serde(rename = "LinkStart-Mission")]
    LinkStartMission,
    #[serde(rename = "LinkStart-AutoRoguelike")]
    LinkStartAutoRoguelike,
    #[serde(rename = "LinkStart-ReclamationAlgorithm")]
    LinkStartReclamationAlgorithm,
}

impl Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::CaptureImage => write!(f, "CaptureImage"),
            TaskType::LinkStartBase => write!(f, "LinkStart-Base"),
            TaskType::LinkStartWakeUp => write!(f, "LinkStart-WakeUp"),
            TaskType::LinkStartCombat => write!(f, "LinkStart-Combat"),
            TaskType::LinkStartRecruiting => write!(f, "LinkStart-Recruiting"),
            TaskType::LinkStartMall => write!(f, "LinkStart-Mall"),
            TaskType::LinkStartMission => write!(f, "LinkStart-Mission"),
            TaskType::LinkStartAutoRoguelike => write!(f, "LinkStart-AutoRoguelike"),
            TaskType::LinkStartReclamationAlgorithm => write!(f, "LinkStart-ReclamationAlgorithm"),
        }
    }
}

impl TaskType {
    pub fn get_all() -> Vec<String> {
        vec![
            "CaptureImage".to_owned(),
            "LinkStart-Base".to_owned(),
            "LinkStart-WakeUp".to_owned(),
            "LinkStart-Combat".to_owned(),
            "LinkStart-Recruiting".to_owned(),
            "LinkStart-Mall".to_owned(),
            "LinkStart-Mission".to_owned(),
            "LinkStart-AutoRoguelike".to_owned(),
            "LinkStart-ReclamationAlgorithm".to_owned(),
        ]
    }
}

impl From<&str> for TaskType {
    fn from(s: &str) -> Self {
        match s {
            "CaptureImage" => Self::CaptureImage,
            "LinkStart-Base" => Self::LinkStartBase,
            "LinkStart-WakeUp" => Self::LinkStartWakeUp,
            "LinkStart-Combat" => Self::LinkStartCombat,
            "LinkStart-Recruiting" => Self::LinkStartRecruiting,
            "LinkStart-Mall" => Self::LinkStartMall,
            "LinkStart-Mission" => Self::LinkStartMission,
            "LinkStart-AutoRoguelike" => Self::LinkStartAutoRoguelike,
            "LinkStart-ReclamationAlgorithm" => Self::LinkStartReclamationAlgorithm,
            _ => panic!("Invalid task type"),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Task {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
}

impl Task {
    pub fn new(task_type: TaskType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_type,
        }
    }

    pub fn from_str(task_type: &str) -> Self {
        Self::new(task_type.into())
    }

    pub fn new_capture_image_task() -> Self {
        Self::new(TaskType::CaptureImage)
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct GetTaskResponse {
    pub tasks: Vec<Task>,
}

#[derive(Deserialize, Debug)]
pub struct GetTaskReq {
    pub user: String,
    pub device: String,
}
