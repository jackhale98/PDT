//! Team roster and role management for workflow authorization

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::core::identity::EntityPrefix;
use crate::core::Project;

/// Signing format options for commit/tag signing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SigningFormat {
    /// GPG signing (traditional, widely supported)
    #[default]
    Gpg,
    /// SSH signing (simpler, uses existing SSH keys, git 2.34+)
    Ssh,
    /// Sigstore gitsign (keyless, OIDC-based, modern)
    Gitsign,
}

impl std::fmt::Display for SigningFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SigningFormat::Gpg => write!(f, "gpg"),
            SigningFormat::Ssh => write!(f, "ssh"),
            SigningFormat::Gitsign => write!(f, "gitsign"),
        }
    }
}

impl std::str::FromStr for SigningFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gpg" => Ok(SigningFormat::Gpg),
            "ssh" => Ok(SigningFormat::Ssh),
            "gitsign" => Ok(SigningFormat::Gitsign),
            _ => Err(format!(
                "Unknown signing format: {}. Valid options: gpg, ssh, gitsign",
                s
            )),
        }
    }
}

/// Team roles for authorization
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ValueEnum, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Engineering,
    Quality,
    Management,
    Admin,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Engineering => write!(f, "engineering"),
            Role::Quality => write!(f, "quality"),
            Role::Management => write!(f, "management"),
            Role::Admin => write!(f, "admin"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "engineering" => Ok(Role::Engineering),
            "quality" => Ok(Role::Quality),
            "management" => Ok(Role::Management),
            "admin" => Ok(Role::Admin),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// A team member with their roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub name: String,
    pub email: String,
    /// Username for matching git user (git config user.name or user.email)
    pub username: String,
    #[serde(default)]
    pub roles: Vec<Role>,
    #[serde(default = "default_active")]
    pub active: bool,
    /// Signing format used by this member (gpg, ssh, or gitsign)
    /// Public keys are stored in .tdt/keys/{format}/{username}.pub or .asc
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_format: Option<SigningFormat>,
}

fn default_active() -> bool {
    true
}

impl TeamMember {
    /// Check if member has a specific role
    pub fn has_role(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }

    /// Check if member has any of the specified roles
    pub fn has_any_role(&self, roles: &[Role]) -> bool {
        roles.iter().any(|r| self.roles.contains(r))
    }

    /// Check if member is an admin (can bypass authorization)
    pub fn is_admin(&self) -> bool {
        self.roles.contains(&Role::Admin)
    }
}

/// Team roster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRoster {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub members: Vec<TeamMember>,
    /// Approval matrix: entity prefix -> required roles
    /// Special key "_release" for release authorization
    #[serde(default)]
    pub approval_matrix: HashMap<String, Vec<Role>>,
}

fn default_version() -> u32 {
    1
}

impl Default for TeamRoster {
    fn default() -> Self {
        Self {
            version: 1,
            members: Vec::new(),
            approval_matrix: HashMap::new(),
        }
    }
}

impl TeamRoster {
    /// Load team roster from project's .tdt/team.yaml
    pub fn load(project: &Project) -> Option<Self> {
        let path = project.tdt_dir().join("team.yaml");
        Self::load_from_path(&path)
    }

    /// Load team roster from a specific path
    pub fn load_from_path(path: &Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }

        let contents = std::fs::read_to_string(path).ok()?;
        serde_yml::from_str(&contents).ok()
    }

    /// Save team roster to project's .tdt/team.yaml
    pub fn save(&self, project: &Project) -> std::io::Result<()> {
        let path = project.tdt_dir().join("team.yaml");
        self.save_to_path(&path)
    }

    /// Save team roster to a specific path
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        let contents = serde_yml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, contents)
    }

    /// Find member by username (matches git user.name)
    pub fn find_member(&self, username: &str) -> Option<&TeamMember> {
        self.members
            .iter()
            .find(|m| m.active && m.username.eq_ignore_ascii_case(username))
    }

    /// Find member by email
    pub fn find_member_by_email(&self, email: &str) -> Option<&TeamMember> {
        self.members
            .iter()
            .find(|m| m.active && m.email.eq_ignore_ascii_case(email))
    }

    /// Get current user as team member (from git config)
    pub fn current_user(&self) -> Option<&TeamMember> {
        // Try git user.name first
        if let Ok(output) = std::process::Command::new("git")
            .args(["config", "user.name"])
            .output()
        {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !name.is_empty() {
                    if let Some(member) = self.find_member(&name) {
                        return Some(member);
                    }
                }
            }
        }

        // Try git user.email as fallback
        if let Ok(output) = std::process::Command::new("git")
            .args(["config", "user.email"])
            .output()
        {
            if output.status.success() {
                let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !email.is_empty() {
                    if let Some(member) = self.find_member_by_email(&email) {
                        return Some(member);
                    }
                }
            }
        }

        None
    }

    /// Get roles required to approve an entity type
    pub fn required_roles(&self, entity_prefix: EntityPrefix) -> Option<&Vec<Role>> {
        let key = entity_prefix.to_string();
        self.approval_matrix.get(&key)
    }

    /// Get roles required to release entities
    pub fn release_roles(&self) -> Option<&Vec<Role>> {
        self.approval_matrix.get("_release")
    }

    /// Check if a member can approve an entity type
    pub fn can_approve(&self, member: &TeamMember, entity_prefix: EntityPrefix) -> bool {
        // Admins can approve anything
        if member.is_admin() {
            return true;
        }

        // If no approval matrix entry, anyone can approve
        let Some(required_roles) = self.required_roles(entity_prefix) else {
            return true;
        };

        // Check if member has any of the required roles
        member.has_any_role(required_roles)
    }

    /// Check if a member can release entities
    pub fn can_release(&self, member: &TeamMember) -> bool {
        // Admins can release anything
        if member.is_admin() {
            return true;
        }

        // If no release roles defined, check for management role
        let Some(required_roles) = self.release_roles() else {
            return member.has_role(Role::Management);
        };

        // Check if member has any of the required roles
        member.has_any_role(required_roles)
    }

    /// Add a member to the roster
    pub fn add_member(&mut self, member: TeamMember) {
        self.members.push(member);
    }

    /// Remove a member by username
    pub fn remove_member(&mut self, username: &str) -> bool {
        let len_before = self.members.len();
        self.members
            .retain(|m| !m.username.eq_ignore_ascii_case(username));
        self.members.len() < len_before
    }

    /// Get all active members
    pub fn active_members(&self) -> impl Iterator<Item = &TeamMember> {
        self.members.iter().filter(|m| m.active)
    }

    /// Get members with a specific role
    pub fn members_with_role(&self, role: Role) -> impl Iterator<Item = &TeamMember> {
        self.members
            .iter()
            .filter(move |m| m.active && m.has_role(role))
    }

    /// Generate default team.yaml template content
    pub fn default_template() -> &'static str {
        r#"# TDT Team Roster
# Defines team members and their roles for workflow authorization

version: 1

members:
  # Example member entry:
  # - name: "Jane Smith"
  #   email: "jane@example.com"
  #   username: "jsmith"        # Matches git config user.name
  #   roles: [engineering, quality]
  #   signing_format: gpg       # Optional: gpg, ssh, or gitsign
  #   active: true
  []

# Approval matrix: which roles can approve which entity types
# If an entity type is not listed, any team member can approve it
# Role options: engineering, quality, management, admin
approval_matrix:
  # REQ: [engineering, quality]
  # RISK: [quality, management]
  # DEV: [quality, management]
  # NCR: [quality]
  # CAPA: [quality, management]
  # _release: [management]  # Special key for release authorization
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_roster() -> TeamRoster {
        let mut roster = TeamRoster::default();
        roster.members.push(TeamMember {
            name: "Jane Smith".to_string(),
            email: "jane@example.com".to_string(),
            username: "jsmith".to_string(),
            roles: vec![Role::Engineering, Role::Quality],
            active: true,
            signing_format: Some(SigningFormat::Gpg),
        });
        roster.members.push(TeamMember {
            name: "Bob Wilson".to_string(),
            email: "bob@example.com".to_string(),
            username: "bwilson".to_string(),
            roles: vec![Role::Management],
            active: true,
            signing_format: Some(SigningFormat::Ssh),
        });
        roster.members.push(TeamMember {
            name: "Admin User".to_string(),
            email: "admin@example.com".to_string(),
            username: "admin".to_string(),
            roles: vec![Role::Admin],
            active: true,
            signing_format: None,
        });
        roster
            .approval_matrix
            .insert("REQ".to_string(), vec![Role::Engineering, Role::Quality]);
        roster
            .approval_matrix
            .insert("RISK".to_string(), vec![Role::Quality, Role::Management]);
        roster
            .approval_matrix
            .insert("_release".to_string(), vec![Role::Management]);
        roster
    }

    #[test]
    fn test_find_member() {
        let roster = create_test_roster();
        let member = roster.find_member("jsmith").unwrap();
        assert_eq!(member.name, "Jane Smith");
    }

    #[test]
    fn test_find_member_case_insensitive() {
        let roster = create_test_roster();
        assert!(roster.find_member("JSMITH").is_some());
        assert!(roster.find_member("JSmith").is_some());
    }

    #[test]
    fn test_find_member_by_email() {
        let roster = create_test_roster();
        let member = roster.find_member_by_email("jane@example.com").unwrap();
        assert_eq!(member.username, "jsmith");
    }

    #[test]
    fn test_can_approve_with_required_roles() {
        let roster = create_test_roster();
        let jane = roster.find_member("jsmith").unwrap();
        let bob = roster.find_member("bwilson").unwrap();

        // Jane (Engineering, Quality) can approve REQ
        assert!(roster.can_approve(jane, EntityPrefix::Req));

        // Bob (Management) cannot approve REQ
        assert!(!roster.can_approve(bob, EntityPrefix::Req));

        // Bob can approve RISK (needs Quality or Management)
        assert!(roster.can_approve(bob, EntityPrefix::Risk));
    }

    #[test]
    fn test_admin_can_approve_anything() {
        let roster = create_test_roster();
        let admin = roster.find_member("admin").unwrap();

        assert!(roster.can_approve(admin, EntityPrefix::Req));
        assert!(roster.can_approve(admin, EntityPrefix::Risk));
        assert!(roster.can_release(admin));
    }

    #[test]
    fn test_can_release() {
        let roster = create_test_roster();
        let jane = roster.find_member("jsmith").unwrap();
        let bob = roster.find_member("bwilson").unwrap();

        // Jane (Engineering, Quality) cannot release
        assert!(!roster.can_release(jane));

        // Bob (Management) can release
        assert!(roster.can_release(bob));
    }

    #[test]
    fn test_no_approval_matrix_allows_anyone() {
        let roster = create_test_roster();
        let jane = roster.find_member("jsmith").unwrap();
        let bob = roster.find_member("bwilson").unwrap();

        // CMP is not in approval matrix, so anyone can approve
        assert!(roster.can_approve(jane, EntityPrefix::Cmp));
        assert!(roster.can_approve(bob, EntityPrefix::Cmp));
    }

    #[test]
    fn test_save_and_load() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("team.yaml");

        let roster = create_test_roster();
        roster.save_to_path(&path).unwrap();

        let loaded = TeamRoster::load_from_path(&path).unwrap();
        assert_eq!(loaded.members.len(), 3);
        assert_eq!(loaded.members[0].name, "Jane Smith");
    }

    #[test]
    fn test_add_remove_member() {
        let mut roster = TeamRoster::default();

        roster.add_member(TeamMember {
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
            roles: vec![Role::Engineering],
            active: true,
            signing_format: None,
        });

        assert_eq!(roster.members.len(), 1);
        assert!(roster.find_member("testuser").is_some());

        roster.remove_member("testuser");
        assert_eq!(roster.members.len(), 0);
        assert!(roster.find_member("testuser").is_none());
    }

    #[test]
    fn test_signing_format_serialization() {
        let mut roster = TeamRoster::default();
        roster.add_member(TeamMember {
            name: "SSH User".to_string(),
            email: "ssh@example.com".to_string(),
            username: "sshuser".to_string(),
            roles: vec![Role::Engineering],
            active: true,
            signing_format: Some(SigningFormat::Ssh),
        });

        let yaml = serde_yml::to_string(&roster).unwrap();
        assert!(yaml.contains("signing_format: ssh"));

        let loaded: TeamRoster = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(loaded.members[0].signing_format, Some(SigningFormat::Ssh));
    }
}
