use super::super::{ObjectError, ObjectResult, rollup, store::*};
use super::*;
use crate::KanbanService;
use kanban_core::Clock;
use turso::Connection;

pub(super) fn page(limit: usize, offset: usize) -> ObjectResult<()> {
    if limit == 0 || limit > 1_000 || i64::try_from(offset).is_err() {
        return Err(ObjectError::invalid(
            "分页需要 1..=1000 的 limit 和非负 offset",
        ));
    }
    Ok(())
}

pub(super) async fn record(
    c: &Connection,
    board_id: &str,
    kind: PlanningKind,
    id: &str,
    body: bool,
) -> ObjectResult<PlanningRecord> {
    if !id.starts_with("obj_") || id.len() <= 4 {
        return Err(ObjectError::invalid("模块和迭代必须使用 obj_... ID"));
    }
    let columns = if body { "o.body" } else { "NULL" };
    let query = format!(
        "SELECT o.id,o.board_id,o.title,{columns},o.version,o.created_at,o.updated_at,o.archived_at,(SELECT source_id FROM object_relation_edges WHERE board_id=o.board_id AND relation_key='module_hierarchy' AND target_id=o.id), (SELECT option_key FROM object_property_values WHERE object_id=o.id AND property_key='cycle.status'),(SELECT integer_value FROM object_property_values WHERE object_id=o.id AND property_key='cycle.starts_at'),(SELECT integer_value FROM object_property_values WHERE object_id=o.id AND property_key='cycle.ends_at'),(SELECT integer_value FROM object_property_values WHERE object_id=o.id AND property_key='cycle.started_at'),(SELECT integer_value FROM object_property_values WHERE object_id=o.id AND property_key='cycle.closed_at') FROM objects o WHERE o.board_id=?1 AND o.id=?2 AND o.type_key=?3"
    );
    let data = rows(c, &query, vec![s(board_id), s(id), s(kind.key())]).await?;
    let r = data
        .first()
        .ok_or_else(|| ObjectError::missing("当前项目中不存在对应类型的对象"))?;
    Ok(PlanningRecord {
        id: text(&r[0])?,
        board_id: text(&r[1])?,
        title: text(&r[2])?,
        body: opt_text(&r[3])?,
        version: ObjectVersion {
            object: int(&r[4])?,
            source: None,
        },
        created_at: int(&r[5])?,
        updated_at: int(&r[6])?,
        archived_at: opt_int(&r[7])?,
        parent_id: opt_text(&r[8])?,
        status: opt_text(&r[9])?,
        starts_at: opt_int(&r[10])?,
        ends_at: opt_int(&r[11])?,
        started_at: opt_int(&r[12])?,
        closed_at: opt_int(&r[13])?,
    })
}

impl<C: Clock> KanbanService<C> {
    pub async fn planning_get(
        &self,
        board_id: &str,
        kind: PlanningKind,
        id: &str,
    ) -> ObjectResult<PlanningRecord> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let result = record(&tx, board_id, kind, id, true).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn planning_list(
        &self,
        board_id: &str,
        kind: PlanningKind,
        options: PlanningListOptions,
    ) -> ObjectResult<PlanningPage> {
        page(options.limit, options.offset)?;
        if options.q.as_ref().is_some_and(|q| q.chars().count() > 1024) {
            return Err(ObjectError::invalid("标题查询超过 1024 字符"));
        }
        if kind == PlanningKind::Module && options.status.is_some()
            || kind == PlanningKind::Cycle && options.parent_id.is_some()
        {
            return Err(ObjectError::invalid("筛选不适用于当前类型"));
        }
        if options
            .status
            .as_deref()
            .is_some_and(|s| !matches!(s, "planned" | "active" | "completed" | "cancelled"))
        {
            return Err(ObjectError::invalid("无效迭代状态"));
        }
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        board(&tx, board_id, false).await?;
        if let Some(parent) = &options.parent_id {
            record(&tx, board_id, PlanningKind::Module, parent, false).await?;
        }
        let predicate = "FROM objects o WHERE o.board_id=?1 AND o.type_key=?2 AND (?3 OR o.archived_at IS NULL) AND (?4 IS NULL OR instr(lower(o.title),lower(?4))>0) AND (?5 IS NULL OR EXISTS (SELECT 1 FROM object_relation_edges e WHERE e.board_id=o.board_id AND e.relation_key='module_hierarchy' AND e.target_id=o.id AND e.source_id=?5)) AND (?6 IS NULL OR EXISTS (SELECT 1 FROM object_property_values v WHERE v.object_id=o.id AND v.property_key='cycle.status' AND v.option_key=?6))";
        let mut args = vec![
            s(board_id),
            s(kind.key()),
            n(i64::from(options.include_archived)),
            os(options.q.as_deref()),
            os(options.parent_id.as_deref()),
            os(options.status.as_deref()),
        ];
        let total =
            count(&tx, &format!("SELECT COUNT(*) {predicate}"), args.clone()).await? as usize;
        args.extend([n(options.limit as i64), n(options.offset as i64)]);
        let ids = rows(
            &tx,
            &format!("SELECT o.id {predicate} ORDER BY o.id LIMIT ?7 OFFSET ?8"),
            args,
        )
        .await?;
        let mut items = Vec::new();
        for row in ids {
            items.push(record(&tx, board_id, kind, &text(&row[0])?, false).await?);
        }
        tx.commit().await?;
        Ok(PlanningPage { items, total })
    }

    pub async fn planning_members(
        &self,
        board_id: &str,
        kind: PlanningKind,
        id: &str,
        limit: usize,
        offset: usize,
    ) -> ObjectResult<PlanningMemberPage> {
        page(limit, offset)?;
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let object = record(&tx, board_id, kind, id, false).await?;
        let frozen = object.closed_at.is_some();
        let base = vec![s(board_id), s(id)];
        let mut args = base.clone();
        args.extend([n(limit as i64), n(offset as i64)]);
        let (total, data, captured_at) = if frozen {
            // 由数据库对快照数组分页，避免把大型正文与完整快照送到 adapter。
            let source = "FROM object_snapshots snap, json_each(snap.body_json,'$.members') member WHERE snap.board_id=?1 AND snap.object_id=?2 AND snap.kind='workflow.closed'";
            let total = count(&tx, &format!("SELECT COUNT(*) {source}"), base.clone()).await?;
            let data = rows(&tx, &format!("SELECT json_extract(member.value,'$.id'),json_extract(member.value,'$.title'),json_extract(member.value,'$.status'),json_extract(member.value,'$.version.object'),json_extract(member.value,'$.version.source'),json_extract(member.value,'$.carried_to') {source} ORDER BY CAST(member.key AS INTEGER) LIMIT ?3 OFFSET ?4"), args).await?;
            let captured = count(&tx,"SELECT json_extract(body_json,'$.captured_at') FROM object_snapshots WHERE board_id=?1 AND object_id=?2 AND kind='workflow.closed'",base).await?;
            (total, data, Some(captured))
        } else {
            let source = format!(
                "FROM object_relation_edges e JOIN tasks t ON t.id=e.target_id AND t.board_id=e.board_id JOIN objects o ON o.id=t.id WHERE e.board_id=?1 AND e.source_id=?2 AND e.relation_key='{}'",
                kind.members()
            );
            let total = count(&tx, &format!("SELECT COUNT(*) {source}"), base).await?;
            let data = rows(&tx, &format!("SELECT t.id,t.title,t.status,o.version,t.lock_version,NULL {source} ORDER BY t.id LIMIT ?3 OFFSET ?4"), args).await?;
            (total, data, None)
        };
        let items = data
            .iter()
            .map(|r| {
                Ok(PlanningMember {
                    id: text(&r[0])?,
                    title: text(&r[1])?,
                    status: text(&r[2])?,
                    version: ObjectVersion {
                        object: int(&r[3])?,
                        source: opt_int(&r[4])?,
                    },
                    carried_to: opt_text(&r[5])?,
                })
            })
            .collect::<ObjectResult<Vec<_>>>()?;
        tx.commit().await?;
        Ok(PlanningMemberPage {
            items,
            total: total as usize,
            frozen,
            captured_at,
            version: object.version,
        })
    }

    pub async fn planning_overview(
        &self,
        board_id: &str,
        kind: PlanningKind,
        id: &str,
    ) -> ObjectResult<PlanningProgress> {
        let mut c = self
            .store
            .connection()
            .await
            .map_err(ObjectError::storage)?;
        let tx = c.transaction().await?;
        let object = record(&tx, board_id, kind, id, true).await?;
        let overview = rollup::overview(&tx, board_id, id).await?;
        let captured_at = object.closed_at;
        tx.commit().await?;
        Ok(PlanningProgress {
            object,
            progress: overview.progress,
            frozen: overview.frozen,
            captured_at,
        })
    }
}
