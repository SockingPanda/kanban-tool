//! 当前可调用的 RPC 清单；DTO parts 的历史 HTTP 名称不注册任何业务路由。

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

pub const BUSINESS_SERVICE: &str = "kanban.v1.KanbanService";
pub const QUERY_SERVICE: &str = "kanban.v1.QueryService";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcMethodDescriptor {
    pub operation_id: String,
    pub service: String,
    pub method: String,
    pub request: String,
    pub response: String,
    pub server_streaming: bool,
}

impl RpcMethodDescriptor {
    pub fn path(&self) -> String {
        format!("/{}/{}", self.service, self.method)
    }
}

#[derive(Deserialize)]
struct BusinessMethod {
    operation_id: String,
    method: String,
    request: String,
    response: String,
}

/// 具名业务方法来自同一份生成 manifest；完整查询流由其正式 Protobuf 定义。
/// descriptor 对照测试保证这里没有缺失、额外方法或错误 streaming 标记。
pub fn methods() -> &'static [RpcMethodDescriptor] {
    static METHODS: OnceLock<Vec<RpcMethodDescriptor>> = OnceLock::new();
    METHODS.get_or_init(|| {
        let business: Vec<BusinessMethod> =
            serde_json::from_str(super::METHOD_MANIFEST).expect("生成 RPC manifest 必须有效");
        let mut methods = business
            .into_iter()
            .map(|item| RpcMethodDescriptor {
                operation_id: item.operation_id,
                service: BUSINESS_SERVICE.into(),
                method: item.method,
                request: item.request,
                response: item.response,
                server_streaming: false,
            })
            .collect::<Vec<_>>();
        methods.push(RpcMethodDescriptor {
            operation_id: "rpc.watch-queries".into(),
            service: QUERY_SERVICE.into(),
            method: "WatchQueries".into(),
            request: "WatchQueriesRequest".into(),
            response: "QueryFrame".into(),
            server_streaming: true,
        });
        methods
    })
}

pub fn method_for_operation(id: &str) -> Option<&'static RpcMethodDescriptor> {
    methods().iter().find(|method| method.operation_id == id)
}
