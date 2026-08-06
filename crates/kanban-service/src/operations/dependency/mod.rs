mod create;
mod list;
mod remove;

pub use create::{AddDependencyCommand, AddDependencyResult};
pub use remove::{RemoveDependencyCommand, RemoveDependencyResult};

pub(crate) fn application_dependency_snapshot(
    snapshot: crate::domain::DependencySnapshotRecord,
) -> crate::Result<crate::DependencySnapshotRecord> {
    Ok(crate::DependencySnapshotRecord {
        task: super::application_task(snapshot.task)?,
        parents: snapshot
            .parents
            .into_iter()
            .map(super::application_task)
            .collect::<crate::Result<Vec<_>>>()?,
        children: snapshot
            .children
            .into_iter()
            .map(super::application_task)
            .collect::<crate::Result<Vec<_>>>()?,
        edges: snapshot
            .edges
            .into_iter()
            .map(|edge| {
                Ok(crate::DependencyEdgeRecord {
                    parent: super::application_task(edge.parent)?,
                    child: super::application_task(edge.child)?,
                })
            })
            .collect::<crate::Result<Vec<_>>>()?,
    })
}
