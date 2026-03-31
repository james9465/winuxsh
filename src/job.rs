// Job control for WinSH
use crate::error::{Result, ShellError};
use std::fmt;

/// Job status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobStatus {
    Running,
    Stopped,
    Done,
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobStatus::Running => write!(f, "Running"),
            JobStatus::Stopped => write!(f, "Stopped"),
            JobStatus::Done => write!(f, "Done"),
        }
    }
}

/// Job structure
#[derive(Debug, Clone)]
pub struct Job {
    pub id: u32,
    pub command: String,
    pub status: JobStatus,
    pub pid: u32,
}

impl Job {
    /// Create a new job
    pub fn new(id: u32, command: String, pid: u32) -> Self {
        Job {
            id,
            command,
            status: JobStatus::Running,
            pid,
        }
    }

    /// Set job status
    pub fn set_status(&mut self, status: JobStatus) {
        self.status = status;
    }

    /// Get job status
    pub fn status(&self) -> JobStatus {
        self.status
    }
}

/// Job manager
#[derive(Debug)]
pub struct JobManager {
    jobs: Vec<Job>,
    next_job_id: u32,
}

impl JobManager {
    /// Create a new job manager
    pub fn new() -> Self {
        JobManager {
            jobs: Vec::new(),
            next_job_id: 1,
        }
    }

    /// Add a background job
    pub fn add_job(&mut self, command: String, pid: u32) -> u32 {
        let job_id = self.next_job_id;
        let job = Job::new(job_id, command, pid);
        self.jobs.push(job);
        self.next_job_id += 1;
        job_id
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: u32) -> Option<&Job> {
        self.jobs.iter().find(|j| j.id == job_id)
    }

    /// Get a mutable job by ID
    pub fn get_job_mut(&mut self, job_id: u32) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == job_id)
    }

    /// Get a job by index
    pub fn get_job_by_index(&self, index: usize) -> Option<&Job> {
        self.jobs.get(index)
    }

    /// Find job index by ID
    pub fn find_job_index(&self, job_id: u32) -> Option<usize> {
        self.jobs.iter().position(|j| j.id == job_id)
    }

    /// List all jobs
    pub fn list_jobs(&self) -> &[Job] {
        &self.jobs
    }

    /// Remove a job by index
    pub fn remove_job(&mut self, index: usize) -> Result<Job> {
        if index < self.jobs.len() {
            Ok(self.jobs.remove(index))
        } else {
            Err(ShellError::Job(format!("Invalid job index: {}", index)))
        }
    }

    /// Cleanup completed jobs
    pub fn cleanup(&mut self) {
        self.jobs.retain(|job| job.status != JobStatus::Done);
    }

    /// Check if there are any jobs
    pub fn has_jobs(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Get job count
    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_creation() {
        let job = Job::new(1, "sleep 10".to_string(), 1234);
        assert_eq!(job.id, 1);
        assert_eq!(job.command, "sleep 10");
        assert_eq!(job.pid, 1234);
        assert_eq!(job.status(), JobStatus::Running);
    }

    #[test]
    fn test_job_status() {
        let mut job = Job::new(1, "sleep 10".to_string(), 1234);
        assert_eq!(job.status(), JobStatus::Running);

        job.set_status(JobStatus::Stopped);
        assert_eq!(job.status(), JobStatus::Stopped);

        job.set_status(JobStatus::Done);
        assert_eq!(job.status(), JobStatus::Done);
    }

    #[test]
    fn test_job_manager() {
        let mut manager = JobManager::new();
        assert_eq!(manager.job_count(), 0);
        assert!(!manager.has_jobs());

        let job_id = manager.add_job("sleep 10".to_string(), 1234);
        assert_eq!(job_id, 1);
        assert_eq!(manager.job_count(), 1);
        assert!(manager.has_jobs());

        let job = manager.get_job(job_id);
        assert!(job.is_some());
        assert_eq!(job.unwrap().command, "sleep 10");

        let index = manager.find_job_index(job_id);
        assert!(index.is_some());
        assert_eq!(index.unwrap(), 0);
    }

    #[test]
    fn test_job_manager_cleanup() {
        let mut manager = JobManager::new();
        manager.add_job("sleep 10".to_string(), 1234);
        manager.add_job("sleep 20".to_string(), 5678);

        // Mark first job as done
        if let Some(job) = manager.get_job_mut(1) {
            job.set_status(JobStatus::Done);
        }

        manager.cleanup();
        assert_eq!(manager.job_count(), 1);
        assert_eq!(manager.list_jobs()[0].id, 2);
    }
}
