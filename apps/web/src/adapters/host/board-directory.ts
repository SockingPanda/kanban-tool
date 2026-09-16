import { parseCanonicalBoardSlug } from '../../domain/board-slug';
import type { BoardOption } from '../../domain/board-directory';
import type { RpcTransport } from '../../application/data/rpc-transport';
import { parseApiListBoardsResponse } from '../../lib/api/generated/contracts/api-list-boards-response';

export async function readBoardDirectory(transport: RpcTransport, signal?: AbortSignal): Promise<readonly BoardOption[]> {
  const response = await transport.call({ method: 'ListBoards', query: { include_archived: true }, signal });
  return parseApiListBoardsResponse(response.payload).data.map(board => {
    const slug = parseCanonicalBoardSlug(board.slug);
    if (!slug) throw new Error('服务返回的项目 slug 无效。');
    return { id: board.id, slug, name: board.name, archived: board.archived_at !== null };
  });
}
